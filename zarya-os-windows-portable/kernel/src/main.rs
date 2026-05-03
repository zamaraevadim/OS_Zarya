#![no_std]
#![no_main]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(panic_info_message)]

//! # Ядро операционной системы «Zarya»
//! 
//! Точка входа и основная инициализация ядра.
//! 
//! ## Архитектура
//! - Гибридное ядро (микроядро + модули)
//! - Написано на Rust с минимальным использованием unsafe
//! - Поддержка x86_64 (long mode)
//! - UEFI и Legacy BIOS загрузка

extern crate alloc;

mod memory;
mod task;
mod ipc;
mod drivers;
mod fs;
mod syscall;
mod net;
mod sync;

use core::panic::PanicInfo;
use core::ptr;
use drivers::serial::SerialWriter;
use memory::{PhysicalAllocator, VirtualMemorySpace};
use task::Scheduler;
use sync::Spinlock;

/// Глобальный экземпляр серийного порта для логирования
static SERIAL: Spinlock<SerialWriter> = Spinlock::new(SerialWriter::new());

/// Макрос для логирования через serial порт
#[macro_export]
macro_rules! kernel_log {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let mut serial = $crate::SERIAL.lock();
        let _ = write!(serial, $($arg)*);
    }};
}

/// Точка входа ядра
/// 
/// Вызывается загрузчиком после переключения в long mode.
/// Получает указатель на boot information структуру.
#[no_mangle]
pub extern "C" fn kernel_main(boot_info: *const BootInfo) -> ! {
    // Инициализация серийного порта для отладки
    drivers::serial::init();
    
    kernel_log!("\n");
    kernel_log!("╔════════════════════════════════════════════════════════╗\n");
    kernel_log!("║           ZARYA OPERATING SYSTEM KERNEL                ║\n");
    kernel_log!("║                    Version 1.0.0                       ║\n");
    kernel_log!("║              Copyright (c) 2024 Zarya Team             ║\n");
    kernel_log!("╚════════════════════════════════════════════════════════╝\n");
    kernel_log!("\n");
    
    // Безопасное чтение boot_info
    let boot_info = unsafe {
        &*boot_info
    };
    
    kernel_log!("[BOOT] Bootloader info at {:p}\n", boot_info);
    kernel_log!("[BOOT] Memory map entries: {}\n", boot_info.memory_map_entries);
    kernel_log!("[BOOT] Kernel loaded at: 0x{:x} - 0x{:x}\n", 
                boot_info.kernel_start, boot_info.kernel_end);
    
    // Инициализация менеджера памяти
    kernel_log!("[INIT] Initializing memory manager...\n");
    memory::init(boot_info);
    kernel_log!("[INIT] Memory manager initialized\n");
    
    // Инициализация планировщика задач
    kernel_log!("[INIT] Initializing task scheduler...\n");
    task::init();
    kernel_log!("[INIT] Task scheduler initialized\n");
    
    // Инициализация IPC подсистемы
    kernel_log!("[INIT] Initializing IPC subsystem...\n");
    ipc::init();
    kernel_log!("[INIT] IPC subsystem initialized\n");
    
    // Инициализация драйверов
    kernel_log!("[INIT] Initializing device drivers...\n");
    drivers::init();
    kernel_log!("[INIT] Device drivers initialized\n");
    
    // Инициализация виртуальной файловой системы
    kernel_log!("[INIT] Initializing virtual filesystem...\n");
    fs::init();
    kernel_log!("[INIT] Virtual filesystem initialized\n");
    
    // Инициализация сетевого стека
    kernel_log!("[INIT] Initializing network stack...\n");
    net::init();
    kernel_log!("[INIT] Network stack initialized\n");
    
    // Инициализация таблицы системных вызовов
    kernel_log!("[INIT] Initializing syscall table...\n");
    syscall::init();
    kernel_log!("[INIT] Syscall table initialized\n");
    
    kernel_log!("\n");
    kernel_log!("═══════════════════════════════════════════════════════\n");
    kernel_log!("✓ Kernel initialization complete!\n");
    kernel_log!("═══════════════════════════════════════════════════════\n");
    kernel_log!("\n");
    kernel_log!("[INFO] Starting system servers...\n");
    
    // Запуск первого пользовательского процесса (init)
    start_init_process();
    
    // Главный цикл ядра (никогда не должен завершиться)
    kernel_log!("[IDLE] Entering kernel idle loop...\n");
    
    loop {
        // Ожидание прерываний (HLT инструкция)
        unsafe {
            core::arch::asm!("hlt");
        }
        
        // Планировщик будет переключать задачи при прерываниях таймера
    }
}

/// Информация о загрузке от bootloader
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BootInfo {
    /// Количество записей в карте памяти
    pub memory_map_entries: usize,
    /// Указатель на карту памяти (физический адрес)
    pub memory_map: u64,
    /// Начальный адрес ядра
    pub kernel_start: u64,
    /// Конечный адрес ядра
    pub kernel_end: u64,
    /// Физический адрес framebuffer (если есть)
    pub framebuffer_addr: u64,
    /// Размер framebuffer в байтах
    pub framebuffer_size: usize,
    /// Ширина экрана
    pub screen_width: u32,
    /// Высота экрана
    pub screen_height: u32,
    /// Количество бит на пиксель
    pub bits_per_pixel: u32,
}

impl BootInfo {
    /// Создаёт дефолтную boot_info для тестирования
    pub const fn default() -> Self {
        BootInfo {
            memory_map_entries: 0,
            memory_map: 0,
            kernel_start: 0x100000,
            kernel_end: 0x200000,
            framebuffer_addr: 0,
            framebuffer_size: 0,
            screen_width: 0,
            screen_height: 0,
            bits_per_pixel: 0,
        }
    }
}

/// Запуск начального пользовательского процесса (init)
fn start_init_process() {
    kernel_log!("[INIT] Creating init process (PID: 1)...\n");
    
    // В реальной системе здесь будет загрузка ELF бинарника init
    // Для демонстрации создаём простой тестовый процесс
    
    use task::{Process, Thread, ProcessState, Priority};
    use alloc::sync::Arc;
    
    // Создание процесса init
    let init_process = Process::new(
        1,  // PID
        0,  // PPID (нет родителя)
        "init".into(),
        Priority::Normal,
    );
    
    kernel_log!("[INIT] Init process created with PID 1\n");
    
    // Добавляем процесс в планировщик
    task::add_process(init_process);
    
    kernel_log!("[INIT] Init process added to scheduler\n");
}

/// Обработчик паники
/// 
/// Вызывается при возникновении неустранимой ошибки в ядре.
/// Выводит информацию об ошибке и останавливает систему.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kernel_log!("\n");
    kernel_log!("╔════════════════════════════════════════════════════════╗\n");
    kernel_log!("║                  KERNEL PANIC!                         ║\n");
    kernel_log!("╚════════════════════════════════════════════════════════╝\n");
    kernel_log!("\n");
    
    if let Some(location) = info.location() {
        kernel_log!("Location: {}:{}:{}\n", 
                   location.file(), 
                   location.line(), 
                   location.column());
    }
    
    if let Some(message) = info.message() {
        kernel_log!("Message: {}\n", message);
    }
    
    kernel_log!("\n");
    kernel_log!("System halted. Please restart.\n");
    
    // Остановка процессора
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

/// Обработчик прерывания таймера
/// 
/// Вызывается планировщиком для переключения задач.
#[no_mangle]
pub extern "C" fn timer_interrupt_handler() {
    // Уведомляем планировщик о тике таймера
    task::scheduler_tick();
}

/// Обработчик прерывания клавиатуры
#[no_mangle]
pub extern "C" fn keyboard_interrupt_handler(scancode: u8) {
    drivers::keyboard::handle_scancode(scancode);
}

/// Обработчик прерывания мыши
#[no_mangle]
pub extern "C" fn mouse_interrupt_handler(buttons: u8, dx: i8, dy: i8) {
    drivers::mouse::handle_mouse_event(buttons, dx, dy);
}
