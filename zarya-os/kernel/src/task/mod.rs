//! # Планировщик задач операционной системы Zarya
//!
//! Реализует:
//! - Вытесняющую многозадачность
//! - Приоритеты процессов (Real-time, Normal, Idle)
//! - Round-robin планирование внутри классов приоритетов
//! - Переключение контекста между потоками
//! - Системные вызовы для управления процессами

#![no_std]

use x86_64::{
    VirtAddr,
    structures::paging::Page,
    instructions::interrupts,
};
use spin::Mutex;
use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::vec::Vec;
use alloc::boxed::Box;

/// Глобальный планировщик
static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

/// Счетчик тиков таймера для планировщика
static TICK_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Инициализация планировщика
pub fn init() {
    *SCHEDULER.lock() = Scheduler::new();
}

/// Классы приоритетов процессов
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Реального времени (наивысший приоритет)
    RealTime = 99,
    /// Высокий приоритет
    High = 75,
    /// Нормальный приоритет (по умолчанию)
    Normal = 50,
    /// Низкий приоритет
    Low = 25,
    /// Фоновый (наинизший приоритет)
    Idle = 0,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

/// Состояние потока
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    /// Поток готов к выполнению
    Ready,
    /// Поток выполняется в данный момент
    Running,
    /// Поток заблокирован (ожидает ресурс)
    Blocked,
    /// Поток завершен
    Terminated,
    /// Поток спит
    Sleeping,
}

/// Тип потока
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadType {
    /// Ядерный поток
    Kernel,
    /// Пользовательский поток
    User,
}

/// Контекст процесса для переключения
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Context {
    /// Регистры общего назначения
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    /// Указатель инструкции
    pub rip: u64,
    /// Кодовый сегмент
    pub cs: u64,
    /// Флаги
    pub rflags: u64,
    /// Указатель стека
    pub rsp: u64,
    /// Сегмент стека
    pub ss: u64,
}

impl Context {
    /// Создание нового контекста с нулевыми значениями
    pub const fn new() -> Self {
        Self {
            rax: 0, rbx: 0, rcx: 0, rdx: 0,
            rsi: 0, rdi: 0, rbp: 0,
            r8: 0, r9: 0, r10: 0, r11: 0,
            r12: 0, r13: 0, r14: 0, r15: 0,
            rip: 0, cs: 0, rflags: 0, rsp: 0, ss: 0,
        }
    }
    
    /// Создание контекста для нового потока
    pub fn new_thread(entry_point: VirtAddr, stack_top: VirtAddr, is_user: bool) -> Self {
        let mut ctx = Self::new();
        ctx.rip = entry_point.as_u64();
        ctx.rsp = stack_top.as_u64();
        
        // Стандартные флаги: прерывания включены
        ctx.rflags = 0x202; // IF flag + reserved bit
        
        // Сегменты для kernel/user режима
        if is_user {
            ctx.cs = 0x23; // User code segment
            ctx.ss = 0x2B; // User data segment
        } else {
            ctx.cs = 0x08; // Kernel code segment
            ctx.ss = 0x10; // Kernel data segment
        }
        
        ctx
    }
}

/// Структура потока (Thread Control Block)
pub struct Thread {
    /// Уникальный идентификатор потока
    pub id: ThreadId,
    /// Идентификатор процесса-владельца
    pub process_id: ProcessId,
    /// Состояние потока
    pub state: ThreadState,
    /// Тип потока
    pub thread_type: ThreadType,
    /// Приоритет потока
    pub priority: Priority,
    /// Контекст выполнения
    pub context: Context,
    /// Вершина стека потока
    pub stack_top: VirtAddr,
    /// Базовый адрес стека
    pub stack_base: VirtAddr,
    /// Размер стека
    pub stack_size: usize,
    /// Время выполнения (тикы)
    pub run_time: usize,
    /// Время ожидания
    pub wait_time: usize,
    /// Родительский поток
    pub parent: Option<ThreadId>,
}

impl Thread {
    /// Создание нового потока
    pub fn new(
        id: ThreadId,
        process_id: ProcessId,
        entry_point: VirtAddr,
        stack_top: VirtAddr,
        stack_base: VirtAddr,
        stack_size: usize,
        priority: Priority,
        thread_type: ThreadType,
    ) -> Self {
        let context = Context::new_thread(
            entry_point,
            stack_top,
            thread_type == ThreadType::User,
        );
        
        Self {
            id,
            process_id,
            state: ThreadState::Ready,
            thread_type,
            priority,
            context,
            stack_top,
            stack_base,
            stack_size,
            run_time: 0,
            wait_time: 0,
            parent: None,
        }
    }
    
    /// Создание главного потока процесса
    pub fn main_thread(
        id: ThreadId,
        process_id: ProcessId,
        entry_point: VirtAddr,
        stack_top: VirtAddr,
        stack_base: VirtAddr,
        stack_size: usize,
    ) -> Self {
        Self::new(
            id,
            process_id,
            entry_point,
            stack_top,
            stack_base,
            stack_size,
            Priority::Normal,
            ThreadType::User,
        )
    }
}

/// Идентификатор потока
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThreadId(pub usize);

impl ThreadId {
    /// Генерация нового уникального ID
    pub fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Идентификатор процесса
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessId(pub usize);

impl ProcessId {
    /// Генерация нового уникального ID
    pub fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
    
    /// PID 0 - idle процесс
    pub const IDLE: Self = ProcessId(0);
    /// PID 1 - init процесс
    pub const INIT: Self = ProcessId(1);
}

/// Структура процесса
pub struct Process {
    /// Уникальный идентификатор процесса
    pub id: ProcessId,
    /// Идентификатор родительского процесса
    pub parent_id: Option<ProcessId>,
    /// Состояние процесса
    pub state: ThreadState,
    /// Приоритет процесса
    pub priority: Priority,
    /// Потоки процесса
    pub threads: Vec<Thread>,
    /// Главный поток (первый поток)
    pub main_thread_id: ThreadId,
    /// Виртуальное адресное пространство (таблица страниц)
    pub page_table_root: Option<u64>,
    /// Рабочая директория
    pub working_dir: [u8; 256],
    /// Аргументы командной строки
    pub args: Vec<[u8; 256]>,
    /// Открытые файловые дескрипторы
    pub file_descriptors: Vec<usize>,
    /// Статистика
    pub total_run_time: usize,
}

impl Process {
    /// Создание нового процесса
    pub fn new(id: ProcessId, parent_id: Option<ProcessId>) -> Self {
        Self {
            id,
            parent_id,
            state: ThreadState::Ready,
            priority: Priority::Normal,
            threads: Vec::new(),
            main_thread_id: ThreadId(0),
            page_table_root: None,
            working_dir: [0u8; 256],
            args: Vec::new(),
            file_descriptors: Vec::new(),
            total_run_time: 0,
        }
    }
    
    /// Создание ядерного потока
    pub fn new_kernel_thread<F>(entry_point: F) -> Box<Process>
    where
        F: FnOnce() + 'static,
    {
        let mut process = Box::new(Process::new(ProcessId::new(), None));
        process.priority = Priority::RealTime;
        
        // В реальной реализации здесь было бы создание стека и контекста
        // Для демонстрации создаем упрощенную версию
        
        let thread_id = ThreadId::new();
        let stack_size = 8 * 1024; // 8KB
        
        // Заглушка для адреса входа
        let entry_addr = VirtAddr::new(entry_point as *const () as u64);
        let stack_top = VirtAddr::new(0xFFFF_FFFF_FFFF_F000u64);
        let stack_base = stack_top - stack_size;
        
        let thread = Thread::new(
            thread_id,
            process.id,
            entry_addr,
            stack_top,
            stack_base,
            stack_size,
            Priority::RealTime,
            ThreadType::Kernel,
        );
        
        process.main_thread_id = thread_id;
        process.threads.push(thread);
        
        process
    }
    
    /// Запуск пользовательского процесса (системный вызов exec)
    pub fn spawn_user_process(path: &str) -> ProcessId {
        let id = ProcessId::new();
        
        println!("[PROCESS] Создание пользовательского процесса: {} (PID: {})", path, id.0);
        
        // В реальной реализации:
        // 1. Загрузка исполняемого файла из ФС
        // 2. Создание адресного пространства
        // 3. Загрузка кода и данных в память
        // 4. Настройка стека с аргументами
        // 5. Создание главного потока
        // 6. Добавление в планировщик
        
        id
    }
    
    /// Добавление потока в процесс
    pub fn add_thread(&mut self, thread: Thread) {
        if self.threads.is_empty() {
            self.main_thread_id = thread.id;
        }
        self.threads.push(thread);
    }
    
    /// Получение текущего потока
    pub fn current_thread(&self) -> Option<&Thread> {
        self.threads.iter().find(|t| t.state == ThreadState::Running)
    }
}

/// Планировщик задач
pub struct Scheduler {
    /// Очередь готовых потоков (приоритетные очереди)
    ready_queues: [Vec<ThreadId>; 5],
    /// Все процессы в системе
    processes: Vec<Box<Process>>,
    /// Текущий выполняемый поток
    current_thread: Option<ThreadId>,
    /// Текущий выполняемый процесс
    current_process: Option<ProcessId>,
    /// Счетчик кванта времени
    time_quantum: usize,
    /// Максимальный квант времени (тиков)
    max_quantum: usize,
}

impl Scheduler {
    /// Создание нового планировщика
    pub const fn new() -> Self {
        Self {
            ready_queues: [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            processes: Vec::new(),
            current_thread: None,
            current_process: None,
            time_quantum: 0,
            max_quantum: 10, // 10 тиков на квант
        }
    }
    
    /// Добавление процесса в планировщик
    pub fn add_process(&mut self, process: Box<Process>) {
        let pid = process.id;
        
        // Добавляем главный поток в очередь готовых
        if let Some(thread) = process.threads.first() {
            if thread.state == ThreadState::Ready {
                self.add_to_ready_queue(thread.id, thread.priority);
            }
        }
        
        self.processes.push(process);
        println!("[SCHEDULER] Процесс добавлен: PID {}", pid.0);
    }
    
    /// Добавление потока в очередь готовых
    fn add_to_ready_queue(&mut self, thread_id: ThreadId, priority: Priority) {
        let queue_index = match priority {
            Priority::RealTime => 0,
            Priority::High => 1,
            Priority::Normal => 2,
            Priority::Low => 3,
            Priority::Idle => 4,
        };
        
        self.ready_queues[queue_index].push(thread_id);
    }
    
    /// Выбор следующего потока для выполнения
    pub fn schedule(&mut self) -> Option<ThreadId> {
        // Проверяем очереди по приоритету (от высшего к низшему)
        for (priority_idx, queue) in self.ready_queues.iter_mut().enumerate() {
            if let Some(thread_id) = queue.pop() {
                // Нашли готовый поток
                return Some(thread_id);
            }
        }
        
        // Нет готовых потоков - возвращаем idle
        None
    }
    
    /// Обработка тика таймера (вызывается из обработчика прерываний)
    pub fn tick(&mut self) {
        TICK_COUNT.fetch_add(1, Ordering::Relaxed);
        self.time_quantum += 1;
        
        // Проверка исчерпания кванта времени
        if self.time_quantum >= self.max_quantum {
            self.time_quantum = 0;
            
            // Вытеснение текущего потока (round-robin)
            if let Some(current_id) = self.current_thread {
                // Возвращаем текущий поток в конец очереди его приоритета
                if let Some(process) = self.get_process_by_thread(current_id) {
                    if let Some(thread) = process.threads.iter().find(|t| t.id == current_id) {
                        if thread.state == ThreadState::Running {
                            thread.state = ThreadState::Ready;
                            self.add_to_ready_queue(thread.id, thread.priority);
                        }
                    }
                }
                
                // Выбираем следующий поток
                if let Some(next_id) = self.schedule() {
                    self.switch_to_thread(next_id);
                }
            }
        }
        
        // Обновление статистики
        if let Some(pid) = self.current_process {
            if let Some(process) = self.get_process_mut(pid) {
                process.total_run_time += 1;
                
                if let Some(thread) = process.current_thread_mut() {
                    thread.run_time += 1;
                }
            }
        }
    }
    
    /// Переключение на указанный поток
    fn switch_to_thread(&mut self, thread_id: ThreadId) {
        // Помечаем старый поток как готовый
        if let Some(old_id) = self.current_thread {
            if let Some(process) = self.get_process_by_thread_mut(old_id) {
                if let Some(thread) = process.threads.iter_mut().find(|t| t.id == old_id) {
                    if thread.state == ThreadState::Running {
                        thread.state = ThreadState::Ready;
                    }
                }
            }
        }
        
        // Находим новый поток и помечаем как_running
        if let Some(process) = self.get_process_by_thread_mut(thread_id) {
            if let Some(thread) = process.threads.iter_mut().find(|t| t.id == thread_id) {
                thread.state = ThreadState::Running;
                self.current_thread = Some(thread_id);
                self.current_process = Some(process.id);
            }
        }
    }
    
    /// Запуск первого потока (вызывается после инициализации)
    pub fn run_first_task() -> ! {
        loop {
            interrupts::enable();
            interrupts::disable();
            x86_64::instructions::hlt();
        }
    }
    
    /// Получение процесса по ID
    fn get_process(&self, pid: ProcessId) -> Option<&Process> {
        self.processes.iter().find(|p| p.id == pid).map(|b| b.as_ref())
    }
    
    /// Получение процесса по ID (mutable)
    fn get_process_mut(&mut self, pid: ProcessId) -> Option<&mut Process> {
        self.processes.iter_mut().find(|p| p.id == pid).map(|b| b.as_mut())
    }
    
    /// Получение процесса по потоку
    fn get_process_by_thread(&self, thread_id: ThreadId) -> Option<&Process> {
        self.processes.iter().find(|p| p.threads.iter().any(|t| t.id == thread_id)).map(|b| b.as_ref())
    }
    
    /// Получение процесса по потоку (mutable)
    fn get_process_by_thread_mut(&mut self, thread_id: ThreadId) -> Option<&mut Process> {
        self.processes.iter_mut().find(|p| p.threads.iter().any(|t| t.id == thread_id)).map(|b| b.as_mut())
    }
}

// Extension methods для Process
impl Process {
    fn current_thread_mut(&mut self) -> Option<&mut Thread> {
        self.threads.iter_mut().find(|t| t.state == ThreadState::Running)
    }
}

/// Системные вызовы для управления процессами
pub mod syscalls {
    use super::*;
    
    /// Создать новый процесс (fork/exec)
    pub fn sys_spawn(path: &[u8]) -> Result<ProcessId, &'static str> {
        if path.is_empty() || path.len() > 256 {
            return Err("Invalid path");
        }
        
        let path_str = core::str::from_utf8(path).map_err(|_| "Invalid UTF-8")?;
        Ok(Process::spawn_user_process(path_str))
    }
    
    /// Завершить текущий процесс
    pub fn sys_exit(status: i32) -> ! {
        println!("[SYSCALL] Exit with status: {}", status);
        
        // В реальной реализации:
        // 1. Освобождение ресурсов процесса
        // 2. Уведомление родительского процесса
        // 3. Переключение на другой поток
        
        loop {
            interrupts::disable();
            x86_64::instructions::hlt();
        }
    }
    
    /// Получить ID текущего процесса
    pub fn sys_getpid() -> ProcessId {
        SCHEDULER.lock().current_process.unwrap_or(ProcessId::IDLE)
    }
    
    /// Получить ID родительского процесса
    pub fn sys_getppid() -> Option<ProcessId> {
        let scheduler = SCHEDULER.lock();
        if let Some(pid) = scheduler.current_process {
            if let Some(process) = scheduler.get_process(pid) {
                return process.parent_id;
            }
        }
        None
    }
    
    /// Установить приоритет потока
    pub fn sys_set_priority(thread_id: ThreadId, priority: Priority) -> Result<(), &'static str> {
        let mut scheduler = SCHEDULER.lock();
        
        if let Some(process) = scheduler.get_process_by_thread_mut(thread_id) {
            if let Some(thread) = process.threads.iter_mut().find(|t| t.id == thread_id) {
                thread.priority = priority;
                return Ok(());
            }
        }
        
        Err("Thread not found")
    }
    
    /// Усыпить текущий поток на указанное время (в миллисекундах)
    pub fn sys_sleep(ms: usize) {
        // В реальной реализации:
        // 1. Установка таймера
        // 2. Перевод потока в состояние Sleeping
        // 3. Переключение на другой поток
        
        let ticks = ms / 10; // Примерно 10ms на тик
        
        for _ in 0..ticks {
            interrupts::enable();
            interrupts::disable();
            x86_64::instructions::hlt();
        }
    }
    
    /// Получить информацию о процессе
    pub fn sys_get_process_info(pid: ProcessId) -> Option<ProcessInfo> {
        let scheduler = SCHEDULER.lock();
        
        scheduler.get_process(pid).map(|p| ProcessInfo {
            pid: p.id,
            parent_pid: p.parent_id,
            state: p.state,
            priority: p.priority,
            thread_count: p.threads.len(),
            total_run_time: p.total_run_time,
        })
    }
}

/// Информация о процессе (для системных вызовов)
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: ProcessId,
    pub parent_pid: Option<ProcessId>,
    pub state: ThreadState,
    pub priority: Priority,
    pub thread_count: usize,
    pub total_run_time: usize,
}

/// Менеджер потоков (помощник для работы с потоками)
pub struct ThreadManager;

impl ThreadManager {
    /// Создание нового потока в текущем процессе
    pub fn create_thread(
        entry_point: VirtAddr,
        stack_size: usize,
        priority: Priority,
    ) -> Result<ThreadId, &'static str> {
        let mut scheduler = SCHEDULER.lock();
        
        let current_pid = scheduler.current_process.ok_or("No current process")?;
        
        if let Some(process) = scheduler.get_process_mut(current_pid) {
            let thread_id = ThreadId::new();
            
            // Выделение стека (упрощенно)
            let stack_top = VirtAddr::new(0xFFFF_FFFF_FFFE_F000u64 - (process.threads.len() as u64 * stack_size as u64));
            let stack_base = stack_top - stack_size;
            
            let thread = Thread::new(
                thread_id,
                process.id,
                entry_point,
                stack_top,
                stack_base,
                stack_size,
                priority,
                ThreadType::User,
            );
            
            process.add_thread(thread);
            scheduler.add_to_ready_queue(thread_id, priority);
            
            Ok(thread_id)
        } else {
            Err("Process not found")
        }
    }
    
    /// Завершение потока
    pub fn terminate_thread(thread_id: ThreadId) -> Result<(), &'static str> {
        let mut scheduler = SCHEDULER.lock();
        
        if let Some(process) = scheduler.get_process_by_thread_mut(thread_id) {
            if let Some(thread) = process.threads.iter_mut().find(|t| t.id == thread_id) {
                thread.state = ThreadState::Terminated;
                return Ok(());
            }
        }
        
        Err("Thread not found")
    }
    
    /// Блокировка потока (ожидание ресурса)
    pub fn block_thread(thread_id: ThreadId) -> Result<(), &'static str> {
        let mut scheduler = SCHEDULER.lock();
        
        if let Some(process) = scheduler.get_process_by_thread_mut(thread_id) {
            if let Some(thread) = process.threads.iter_mut().find(|t| t.id == thread_id) {
                if thread.state == ThreadState::Running {
                    thread.state = ThreadState::Blocked;
                    // Принудительное переключение
                    scheduler.time_quantum = scheduler.max_quantum;
                }
                return Ok(());
            }
        }
        
        Err("Thread not found")
    }
    
    /// Разблокировка потока
    pub fn unblock_thread(thread_id: ThreadId) -> Result<(), &'static str> {
        let mut scheduler = SCHEDULER.lock();
        
        if let Some(process) = scheduler.get_process_by_thread_mut(thread_id) {
            if let Some(thread) = process.threads.iter_mut().find(|t| t.id == thread_id) {
                if thread.state == ThreadState::Blocked {
                    thread.state = ThreadState::Ready;
                    scheduler.add_to_ready_queue(thread_id, thread.priority);
                }
                return Ok(());
            }
        }
        
        Err("Thread not found")
    }
}
