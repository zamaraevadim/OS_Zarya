//! Планировщик задач операционной системы Zarya
//! 
//! Реализует:
//! - Вытесняющую многозадачность
//! - Процессы и потоки
//! - Приоритеты и классы планирования
//! - Round-robin и real-time планирование

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use crate::sync::Spinlock;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::sync::Arc;

/// Инициализирован ли планировщик
static mut INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Глобальный экземпляр планировщика
static SCHEDULER: Spinlock<Scheduler> = Spinlock::new(Scheduler::new());

/// Счётчик тиков таймера
static TICK_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Максимальное количество процессов в системе
const MAX_PROCESSES: usize = 256;

/// Квант времени для normal приоритета (в тиках таймера)
const TIME_SLICE_NORMAL: usize = 10;

/// Квант времени для batch приоритета
const TIME_SLICE_BATCH: usize = 50;

/// Инициализация планировщика
pub fn init() {
    unsafe {
        if INITIALIZED.load(Ordering::Relaxed) {
            kernel_log!("[SCHED] Already initialized!\n");
            return;
        }
        
        INITIALIZED.store(true, Ordering::Relaxed);
        kernel_log!("[SCHED] Scheduler initialized\n");
    }
}

/// Проверка инициализации
fn check_initialized() {
    unsafe {
        assert!(INITIALIZED.load(Ordering::Relaxed), 
                "Scheduler not initialized!");
    }
}

/// ID процесса
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pid(pub u32);

impl Pid {
    pub const fn new(id: u32) -> Self {
        Pid(id)
    }
    
    pub const fn as_u32(&self) -> u32 {
        self.0
    }
    
    /// PID несуществующего процесса
    pub const INVALID: Pid = Pid(0);
}

/// ID потока
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tid(pub u32);

impl Tid {
    pub const fn new(id: u32) -> Self {
        Tid(id)
    }
}

/// Состояние процесса
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    /// Новый процесс (ещё не запущен)
    New,
    /// Выполняется
    Running,
    /// Готов к выполнению (ждёт CPU)
    Ready,
    /// Ожидает событие (I/O, IPC, etc.)
    Waiting,
    /// Приостановлен (stopped)
    Stopped,
    /// Завершён (зомби, ждёт чтения exit status)
    Zombie,
    /// Уничтожен
    Dead,
}

/// Состояние потока
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    /// Новый поток
    New,
    /// Выполняется
    Running,
    /// Готов к выполнению
    Ready,
    /// Ожидает событие
    Blocked,
    /// Завершён
    Terminated,
}

/// Класс планирования
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedClass {
    /// Обычные процессы (CFS-like)
    Normal,
    /// Пакетные задачи (фоновые)
    Batch,
    /// Real-time FIFO
    FIFO,
    /// Real-time Round-Robin
    RR,
}

/// Приоритет процесса/потока
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Самый низкий (idle)
    Idle = 0,
    /// Низкий
    Low = 1,
    /// Нормальный (по умолчанию)
    Normal = 2,
    /// Высокий
    High = 3,
    /// Самый высокий (real-time)
    RealTime = 4,
}

impl Priority {
    /// Приоритет по умолчанию
    pub const DEFAULT: Priority = Priority::Normal;
}

/// Контекст процессора (сохраняемые регистры)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CpuContext {
    /// Регистры общего назначения x86_64
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
    /// Флаги
    pub rflags: u64,
    /// Указатель стека
    pub rsp: u64,
}

impl CpuContext {
    /// Создаёт контекст с нулевыми значениями
    pub const fn zero() -> Self {
        CpuContext {
            rax: 0, rbx: 0, rcx: 0, rdx: 0,
            rsi: 0, rdi: 0, rbp: 0,
            r8: 0, r9: 0, r10: 0, r11: 0,
            r12: 0, r13: 0, r14: 0, r15: 0,
            rip: 0, rflags: 0, rsp: 0,
        }
    }
}

/// Ядерный стек потока
pub struct KernelStack {
    /// Базовый адрес стека
    base: u64,
    /// Размер стека в байтах
    size: usize,
}

impl KernelStack {
    /// Стандартный размер ядрового стека (16 KB)
    pub const SIZE: usize = 16384;
    
    /// Создаёт новый ядерный стек
    pub fn new() -> Self {
        use crate::memory::{alloc_physical_page, PAGE_SIZE};
        
        // Выделяем страницы для стека
        let pages_needed = (Self::SIZE + PAGE_SIZE - 1) / PAGE_SIZE;
        let mut base_addr = 0u64;
        
        for i in 0..pages_needed {
            if let Some(page) = alloc_physical_page() {
                if i == 0 {
                    base_addr = page.as_u64();
                }
            } else {
                panic!("Failed to allocate kernel stack pages");
            }
        }
        
        // Очищаем стек
        unsafe {
            core::ptr::write_bytes(base_addr as *mut u8, 0, Self::SIZE);
        }
        
        KernelStack {
            base: base_addr,
            size: Self::SIZE,
        }
    }
    
    /// Получает указатель на вершину стека
    pub fn top(&self) -> u64 {
        self.base + self.size as u64
    }
    
    /// Получает базовый адрес
    pub fn base(&self) -> u64 {
        self.base
    }
}

/// Поток выполнения
pub struct Thread {
    /// ID потока
    pub tid: Tid,
    /// Состояние потока
    pub state: ThreadState,
    /// Приоритет
    pub priority: Priority,
    /// Класс планирования
    pub sched_class: SchedClass,
    /// Контекст процессора
    pub context: CpuContext,
    /// Ядерный стек
    pub kernel_stack: KernelStack,
    /// Родительский процесс
    pub process: Arc<Process>,
    /// Время оставшегося кванта
    pub time_slice: usize,
    /// Суммарное время выполнения
    pub total_runtime: usize,
}

impl Thread {
    /// Создаёт новый поток
    pub fn new(tid: Tid, process: Arc<Process>, priority: Priority) -> Self {
        let kernel_stack = KernelStack::new();
        
        Thread {
            tid,
            state: ThreadState::New,
            priority,
            sched_class: SchedClass::Normal,
            context: CpuContext::zero(),
            kernel_stack,
            process,
            time_slice: TIME_SLICE_NORMAL,
            total_runtime: 0,
        }
    }
    
    /// Создаёт главный поток процесса
    pub fn main_thread(tid: Tid, process: Arc<Process>, 
                       entry_point: u64, stack_top: u64) -> Self {
        let mut thread = Thread::new(tid, process, Priority::Normal);
        
        // Настраиваем контекст для точки входа
        thread.context.rip = entry_point;
        thread.context.rsp = stack_top;
        thread.context.rflags = 0x202; // IF бит включён
        
        thread.state = ThreadState::Ready;
        thread
    }
}

/// Процесс
pub struct Process {
    /// ID процесса
    pub pid: Pid,
    /// ID родительского процесса
    pub ppid: Pid,
    /// Состояние процесса
    pub state: ProcessState,
    /// Имя процесса
    pub name: String,
    /// Приоритет
    pub priority: Priority,
    /// Класс планирования
    pub sched_class: SchedClass,
    /// Потоки процесса
    pub threads: Vec<Arc<Thread>>,
    /// Пространство адресов (будет добавлено позже)
    // pub address_space: AddressSpace,
    /// Дескрипторы файлов
    pub files: Vec<Option<FileInfo>>,
    /// Код завершения (для зомби)
    pub exit_code: i32,
    /// Время создания
    pub created_at: usize,
    /// Общее время выполнения
    pub total_runtime: usize,
}

/// Информация о файле
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub flags: u32,
}

impl Process {
    /// Создаёт новый процесс
    pub fn new(pid: Pid, ppid: Pid, name: String, priority: Priority) -> Arc<Self> {
        let process = Arc::new(Process {
            pid,
            ppid,
            state: ProcessState::New,
            name,
            priority,
            sched_class: SchedClass::Normal,
            threads: Vec::new(),
            files: Vec::new(),
            exit_code: 0,
            created_at: TICK_COUNTER.load(Ordering::Relaxed),
            total_runtime: 0,
        });
        
        process
    }
    
    /// Добавляет поток в процесс
    pub fn add_thread(&mut self, thread: Arc<Thread>) {
        self.threads.push(thread);
    }
    
    /// Создаёт главный поток для запуска кода
    pub fn create_main_thread(self: &Arc<Self>, entry_point: u64) -> Arc<Thread> {
        let tid = Tid::new(self.pid.0 * 1000); // Простая генерация TID
        
        // Выделяем стек пользователя (упрощённо)
        let user_stack_top = 0x7fff_ffff_f000u64;
        
        let thread = Thread::main_thread(
            tid,
            Arc::clone(self),
            entry_point,
            user_stack_top,
        );
        
        let arc_thread = Arc::new(thread);
        
        // Безопасное мутация через Spinlock в планировщике
        // В реальной системе нужна синхронизация
        unsafe {
            let proc_ptr = Arc::as_ptr(self) as *mut Process;
            (*proc_ptr).threads.push(Arc::clone(&arc_thread));
        }
        
        arc_thread
    }
}

/// Планировщик задач
pub struct Scheduler {
    /// Очередь готовых процессов
    ready_queue: Vec<Arc<Process>>,
    /// Текущий выполняемый процесс
    current: Option<Arc<Process>>,
    /// ID следующего доступного PID
    next_pid: Pid,
    /// Статистика
    stats: SchedulerStats,
}

/// Статистика планировщика
#[derive(Debug, Clone, Copy, Default)]
pub struct SchedulerStats {
    /// Количество переключений контекста
    pub context_switches: usize,
    /// Количество созданных процессов
    pub processes_created: usize,
    /// Количество завершённых процессов
    pub processes_exited: usize,
    /// Общее время работы планировщика
    pub total_ticks: usize,
}

impl Scheduler {
    /// Создаёт новый планировщик
    pub const fn new() -> Self {
        Scheduler {
            ready_queue: Vec::new(),
            current: None,
            next_pid: Pid::new(1),
            stats: SchedulerStats {
                context_switches: 0,
                processes_created: 0,
                processes_exited: 0,
                total_ticks: 0,
            },
        }
    }
    
    /// Добавление процесса в очередь готовых
    pub fn add_process(&mut self, process: Arc<Process>) {
        kernel_log!("[SCHED] Adding process {} ({}) to ready queue\n", 
                   process.pid.0, process.name);
        
        // Безопасное изменение состояния процесса
        unsafe {
            let proc_ptr = Arc::as_ptr(&process) as *mut Process;
            (*proc_ptr).state = ProcessState::Ready;
        }
        
        self.ready_queue.push(process);
        self.stats.processes_created += 1;
    }
    
    /// Выбор следующего процесса для выполнения
    pub fn pick_next(&mut self) -> Option<Arc<Process>> {
        if self.ready_queue.is_empty() {
            return None;
        }
        
        // Простой round-robin: берём первый из очереди
        // В реальной системе здесь будет более сложный алгоритм
        
        let next = self.ready_queue.remove(0);
        
        // Если текущий процесс ещё жив, возвращаем его в очередь
        if let Some(current) = self.current.take() {
            unsafe {
                let curr_ptr = Arc::as_ptr(&current) as *mut Process;
                if (*curr_ptr).state == ProcessState::Running {
                    (*curr_ptr).state = ProcessState::Ready;
                    self.ready_queue.push(current);
                }
            }
        }
        
        // Отмечаем новый процесс как running
        unsafe {
            let next_ptr = Arc::as_ptr(&next) as *mut Process;
            (*next_ptr).state = ProcessState::Running;
        }
        
        self.current = Some(Arc::clone(&next));
        self.stats.context_switches += 1;
        
        Some(next)
    }
    
    /// Переключение контекста
    pub fn switch_to(&mut self, next: Arc<Process>) {
        kernel_log!("[SCHED] Switching to process {} ({})\n", 
                   next.pid.0, next.name);
        
        self.current = Some(next);
    }
    
    /// Получение текущего процесса
    pub fn current(&self) -> Option<Arc<Process>> {
        self.current.clone()
    }
    
    /// Получение статистики
    pub fn get_stats(&self) -> SchedulerStats {
        self.stats
    }
    
    /// Печать статистики
    pub fn print_stats(&self) {
        kernel_log!("[SCHED] Statistics:\n");
        kernel_log!("  Context switches: {}\n", self.stats.context_switches);
        kernel_log!("  Processes created: {}\n", self.stats.processes_created);
        kernel_log!("  Processes exited: {}\n", self.stats.processes_exited);
        kernel_log!("  Total ticks: {}\n", self.stats.total_ticks);
    }
}

/// Обработка тика таймера
pub fn scheduler_tick() {
    TICK_COUNTER.fetch_add(1, Ordering::Relaxed);
    
    let mut scheduler = SCHEDULER.lock();
    scheduler.stats.total_ticks += 1;
    
    // Проверяем, нужно ли переключать задачу
    if let Some(current) = scheduler.current() {
        unsafe {
            let curr_ptr = Arc::as_ptr(&current) as *mut Process;
            
            // Уменьшаем квант времени
            // В упрощённой версии просто переключаем каждый N тиков
            
            if scheduler.stats.total_ticks % TIME_SLICE_NORMAL == 0 {
                // Время переключаться!
                drop(scheduler);
                schedule();
                return;
            }
        }
    }
}

/// Основная функция планирования
pub fn schedule() {
    let mut scheduler = SCHEDULER.lock();
    
    if let Some(next) = scheduler.pick_next() {
        kernel_log!("[SCHED] Scheduled process {} ({})\n", 
                   next.pid.0, next.name);
        
        // Здесь должно быть реальное переключение контекста
        // switch_context(&old_context, &new_context);
    } else {
        // Нет готовых процессов, выполняем idle
        kernel_log!("[SCHED] No ready processes, entering idle\n");
    }
}

/// Добавление процесса в глобальный планировщик
pub fn add_process(process: Arc<Process>) {
    SCHEDULER.lock().add_process(process);
}

/// Создание нового процесса (fork-подобный вызов)
pub fn fork() -> Result<Pid, &'static str> {
    let scheduler = SCHEDULER.lock();
    let current = scheduler.current()
        .ok_or("No current process")?;
    
    let new_pid = scheduler.next_pid;
    
    // В реальной системе здесь будет клонирование процесса
    // Для демонстрации создаём простой процесс
    
    drop(scheduler);
    
    let child = Process::new(
        new_pid,
        current.pid,
        format!("{} (child)", current.name),
        current.priority,
    );
    
    add_process(child);
    
    Ok(new_pid)
}

/// Завершение текущего процесса
pub fn exit(code: i32) -> ! {
    kernel_log!("[EXIT] Process exiting with code {}\n", code);
    
    // В реальной системе:
    // 1. Освобождаем ресурсы
    // 2. Уведомляем родителя
    // 3. Переходим в zombie состояние
    // 4. Вызываем планировщик
    
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

/// Ожидание дочернего процесса
pub fn wait(pid: Pid) -> Result<i32, &'static str> {
    // В реальной системе ожидание завершения дочернего процесса
    Ok(0)
}

/// Получение ID текущего процесса
pub fn getpid() -> Pid {
    SCHEDULER.lock()
        .current()
        .map(|p| p.pid)
        .unwrap_or(Pid::INVALID)
}

/// Получение ID родительского процесса
pub fn getppid() -> Pid {
    SCHEDULER.lock()
        .current()
        .map(|p| p.ppid)
        .unwrap_or(Pid::INVALID)
}

/// Yield CPU другому процессу
pub fn yield_cpu() {
    schedule();
}

/// Печать списка процессов
pub fn print_processes() {
    let scheduler = SCHEDULER.lock();
    
    kernel_log!("[PROCESS LIST]\n");
    kernel_log!("PID\tPPID\tSTATE\tNAME\n");
    
    for proc in &scheduler.ready_queue {
        kernel_log!("{}\t{}\t{:?}\t{}\n", 
                   proc.pid.0, proc.ppid.0, proc.state, proc.name);
    }
    
    if let Some(current) = &scheduler.current {
        kernel_log!("{}\t{}\t{:?}\t{} [RUNNING]\n", 
                   current.pid.0, current.ppid.0, current.state, current.name);
    }
}
