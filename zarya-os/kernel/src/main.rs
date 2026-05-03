#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(naked_functions)]

//! # Ядро операционной системы Zarya
//! 
//! Точка входа ядра, инициализация всех подсистем:
//! - Менеджер памяти (физический и виртуальный)
//! - Планировщик задач (процессы и потоки)
//! - Драйверы устройств (клавиатура, мышь, framebuffer)
//! - Система прерываний (PIC, IDT)
//! - Системные вызовы

extern crate bootloader;

use core::panic::PanicInfo;
use x86_64::instructions::interrupts;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use x86_64::VirtAddr;

// Модули ядра
pub mod memory;
pub mod task;
pub mod sync;
pub mod ipc;
pub mod syscall;
pub mod fs;
pub mod net;
pub mod drivers;

// Ре-экспорт основных типов
pub use memory::{PhysicalAllocator, VirtualMemoryManager};
pub use task::{Process, Thread, Scheduler};
pub use drivers::{SerialPort, PS2Keyboard, PS2Mouse, Framebuffer};

/// Портативная точка входа из bootloader crate
pub fn kernel_main(boot_info: &'static bootloader::BootInfo) -> ! {
    // Инициализация логгера через UART
    let mut serial = drivers::SerialPort::new();
    serial.init();
    
    println!("\n=== ZARYA OS v0.1.0 ===");
    println!("Загрузка ядра...");
    
    // Инициализация менеджера памяти
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut memory_manager = memory::init(phys_mem_offset, boot_info.memory_map.as_ref().unwrap());
    println!("[OK] Менеджер памяти инициализирован");
    
    // Инициализация таблицы прерываний (IDT)
    let mut idt = InterruptDescriptorTable::new();
    init_idt(&mut idt);
    unsafe {
        idt.load();
    }
    println!("[OK] Таблица прерываний загружена");
    
    // Инициализация PIC (контроллер прерываний)
    let mut pic = interrupt_config();
    println!("[OK] PIC настроен");
    
    // Инициализация драйверов
    let mut keyboard = drivers::PS2Keyboard::new();
    keyboard.init();
    println!("[OK] Клавиатура готова");
    
    let mut mouse = drivers::PS2Mouse::new();
    mouse.init();
    println!("[OK] Мышь готова");
    
    // Инициализация framebuffer для графики
    if let Some(framebuffer_info) = boot_info.framebuffer {
        let fb = drivers::Framebuffer::from_info(framebuffer_info);
        fb.clear_color(0x1a, 0x1a, 0x1a); // Темно-серый фон
        fb.draw_logo(); // Рисуем логотип Zarya
        println!("[OK] Графический режим активирован");
    }
    
    // Инициализация планировщика задач
    task::init();
    println!("[OK] Планировщик запущен");
    
    // Создание начального процесса (init)
    let init_process = Process::new_kernel_thread(|| {
        println!("[INIT] Процесс init запущен");
        
        // Запуск системных серверов
        spawn_system_servers();
        
        // Главный цикл ядра
        loop {
            interrupts::enable();
            interrupts::disable();
            // Idle - ожидание прерываний
            x86_64::instructions::hlt();
        }
    });
    
    // Добавляем процесс в планировщик
    Scheduler::add_process(init_process);
    
    println!("\n=== Система готова к работе ===");
    println!("Запуск пользовательской среды...\n");
    
    // Запускаем планировщик
    Scheduler::run_first_task()
}

/// Инициализация таблицы прерываний
fn init_idt(idt: &mut InterruptDescriptorTable) {
    // Обработчик двойного сбоя (Double Fault)
    idt.double_fault.set_handler_fn(double_fault_handler);
    idt.double_fault.set_stack_index(0);
    
    // Обработчики исключений
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.page_fault.set_handler_fn(page_fault_handler);
    
    // IRQ от PIC
    idt[32].set_handler_fn(timer_interrupt_handler);   // IRQ 0 - таймер
    idt[33].set_handler_fn(keyboard_interrupt_handler); // IRQ 1 - клавиатура
    idt[34].set_handler_fn(mouse_interrupt_handler);    // IRQ 12 - мышь
}

/// Обработчик двойного сбоя
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("\n!!! DOUBLE FAULT !!!\n{:?}", stack_frame);
}

/// Обработчик breakpoint (для отладки)
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("\n[BREAKPOINT] {:?}", stack_frame);
}

/// Обработчик ошибки страницы
extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: x86_64::registers::control_regs::PageFaultErrorCode,
) {
    use x86_64::registers::control_regs::Cr2;
    let addr = Cr2::read();
    println!("\n[PAGE FAULT] Адрес: {:?}, Код ошибки: {:?}", addr, error_code);
    println!("Контекст: {:?}", stack_frame);
    panic!("Page fault halted execution");
}

/// Обработчик таймера (IRQ 0)
extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Вызов планировщика для переключения задач
    task::Scheduler::tick();
    
    // Отправляем EOI в PIC
    unsafe {
        pic8259::ChainedPics::new(0x20, 0x28).notify_end_of_interrupt(32);
    }
}

/// Обработчик клавиатуры (IRQ 1)
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Чтение скан-кода из буфера клавиатуры
    drivers::PS2Keyboard::handle_interrupt();
    
    unsafe {
        pic8259::ChainedPics::new(0x20, 0x28).notify_end_of_interrupt(33);
    }
}

/// Обработчик мыши (IRQ 12)
extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Чтение данных от мыши
    drivers::PS2Mouse::handle_interrupt();
    
    unsafe {
        pic8259::ChainedPics::new(0x20, 0x28).notify_end_of_interrupt(34);
    }
}

/// Настройка PIC
fn interrupt_config() -> pic8259::ChainedPics {
    use x86_64::instructions::port::Port;
    
    let mut pic = pic8259::ChainedPics::new(0x20, 0x28);
    
    unsafe {
        // Маскируем все прерывания на время настройки
        let mut data_port = Port::new(0x21);
        let mut data_port_slave = Port::new(0xA1);
        data_port.write(0xffu8);
        data_port_slave.write(0xffu8);
        
        // Размаскируем нужные прерывания (таймер, клавиатура, мышь)
        data_port.write(0xf8u8); // Оставляем только IRQ 0, 1
        data_port_slave.write(0xefu8); // Оставляем IRQ 12 (мышь)
    }
    
    pic
}

/// Запуск системных серверов (user-space процессы)
fn spawn_system_servers() {
    // В реальной системе здесь создаются процессы:
    // - Zarya Display Server (ZDS)
    // - Input Server
    // - Filesystem Server
    // - Network Server
    
    println!("[SERVERS] Запуск системных демонов...");
    
    // Создаем процесс для Display Server
    let _zds_pid = task::Process::spawn_user_process("/bin/zds");
    
    // Создаем процесс для оконного менеджера
    let _wm_pid = task::Process::spawn_user_process("/bin/zarya-desktop");
    
    // Создаем процесс для панели задач
    let _panel_pid = task::Process::spawn_user_process("/bin/zarya-panel");
}

/// Точка входа при панике
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("\n!!! KERNEL PANIC !!!");
    println!("{}", info);
    
    // Бесконечный цикл с отключенными прерываниями
    loop {
        interrupts::disable();
        x86_64::instructions::hlt();
    }
}

/// Макрос для вывода в UART (упрощенный)
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::drivers::SerialPort::write_fmt(format_args!($($arg)*)));
}
