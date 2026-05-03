//! Драйверы устройств операционной системы Zarya
//! 
//! Модули:
//! - serial: Последовательный порт (16550 UART) для отладки
//! - keyboard: Клавиатура PS/2
//! - mouse: Мышь PS/2
//! - framebuffer: Графический буфер

pub mod serial;
pub mod keyboard;
pub mod mouse;
pub mod framebuffer;

use crate::sync::Spinlock;

/// Инициализация всех драйверов
pub fn init() {
    kernel_log!("[DRIVERS] Initializing drivers...\n");
    
    // Инициализация в порядке зависимости
    serial::init();
    keyboard::init();
    mouse::init();
    framebuffer::init();
    
    kernel_log!("[DRIVERS] All drivers initialized\n");
}

/// Порты ввода-вывода x86
/// 
/// Безопасные обёртки для asm инструкций in/out
#[inline(always)]
pub unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!(
        "outb %al, %dx",
        in("dx") port,
        in("al") value,
        options(nomem, nostack),
    );
}

#[inline(always)]
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!(
        "inb %dx, %al",
        in("dx") port,
        out("al") value,
        options(nomem, nostack),
    );
    value
}

#[inline(always)]
pub unsafe fn outw(port: u16, value: u16) {
    core::arch::asm!(
        "outw %ax, %dx",
        in("dx") port,
        in("ax") value,
        options(nomem, nostack),
    );
}

#[inline(always)]
pub unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    core::arch::asm!(
        "inw %dx, %ax",
        in("dx") port,
        out("ax") value,
        options(nomem, nostack),
    );
    value
}

#[inline(always)]
pub unsafe fn outl(port: u16, value: u32) {
    core::arch::asm!(
        "outl %eax, %dx",
        in("dx") port,
        in("eax") value,
        options(nomem, nostack),
    );
}

#[inline(always)]
pub unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    core::arch::asm!(
        "inl %dx, %eax",
        in("dx") port,
        out("eax") value,
        options(nomem, nostack),
    );
    value
}

/// Задержка для синхронизации с устройствами
#[inline(always)]
pub fn io_delay() {
    // Чтение из порта 0x80 вызывает небольшую задержку
    unsafe {
        inb(0x80);
    }
}

/// Прерывания IRQ
pub mod irq {
    /// IRQ линии
    pub const TIMER: usize = 0;
    pub const KEYBOARD: usize = 1;
    pub const CASCADE: usize = 2;
    pub const COM2: usize = 3;
    pub const COM1: usize = 4;
    pub const LPT2: usize = 5;
    pub const FLOPPY: usize = 6;
    pub const SPURIOUS: usize = 7;
    pub const RTC: usize = 8;
    pub const FREE1: usize = 9;
    pub const FREE2: usize = 10;
    pub const FREE3: usize = 11;
    pub const PS2_MOUSE: usize = 12;
    pub const FPU: usize = 13;
    pub const PRIMARY_ATA: usize = 14;
    pub const SECONDARY_ATA: usize = 15;
    
    /// Базовый номер прерывания для master PIC
    pub const PIC1_OFFSET: usize = 0x20;
    /// Базовый номер прерывания для slave PIC
    pub const PIC2_OFFSET: usize = 0x28;
    
    /// Получение номера прерывания по IRQ
    pub const fn irq_to_vector(irq: usize) -> u8 {
        (PIC1_OFFSET + irq) as u8
    }
}

/// Команды PIC (Programmable Interrupt Controller)
pub mod pic {
    use super::{inb, outb, io_delay};
    
    /// Порты PIC
    pub const PIC1_CMD: u16 = 0x20;
    pub const PIC1_DATA: u16 = 0x21;
    pub const PIC2_CMD: u16 = 0xA0;
    pub const PIC2_DATA: u16 = 0xA1;
    
    /// Команда End Of Interrupt
    pub const EOI: u8 = 0x20;
    
    /// ICW1 flags
    pub const ICW1_ICW4: u8 = 0x01;
    pub const ICW1_SINGLE: u8 = 0x02;
    pub const ICW1_INTERVAL4: u8 = 0x04;
    pub const ICW1_LEVEL: u8 = 0x08;
    pub const ICW1_INIT: u8 = 0x10;
    
    /// ICW4 flags
    pub const ICW4_8086: u8 = 0x01;
    pub const ICW4_AUTO: u8 = 0x02;
    pub const ICW4_BUF_SLAVE: u8 = 0x08;
    pub const ICW4_BUF_MASTER: u8 = 0x0C;
    pub const ICW4_SFNM: u8 = 0x10;
    
    /// Инициализация PIC
    pub fn init(offset1: u8, offset2: u8) {
        unsafe {
            // Сохраняем маски
            let a1 = inb(PIC1_DATA);
            let a2 = inb(PIC2_DATA);
            
            // ICW1: начинаем инициализацию
            outb(PIC1_CMD, ICW1_INIT | ICW1_ICW4);
            io_delay();
            outb(PIC2_CMD, ICW1_INIT | ICW1_ICW4);
            io_delay();
            
            // ICW2: векторы прерываний
            outb(PIC1_DATA, offset1);
            io_delay();
            outb(PIC2_DATA, offset2);
            io_delay();
            
            // ICW3: конфигурация каскадирования
            outb(PIC1_DATA, 0b00000100); // Slave на IRQ2
            io_delay();
            outb(PIC2_DATA, 0b00000010); // Slave identity
            io_delay();
            
            // ICW4: режим 8086
            outb(PIC1_DATA, ICW4_8086);
            io_delay();
            outb(PIC2_DATA, ICW4_8086);
            io_delay();
            
            // Восстанавливаем маски
            outb(PIC1_DATA, a1);
            outb(PIC2_DATA, a2);
        }
        
        kernel_log!("[PIC] Initialized with offsets 0x{:x}, 0x{:x}\n", offset1, offset2);
    }
    
    /// Отправка EOI (End Of Interrupt)
    pub fn send_eoi() {
        unsafe {
            outb(PIC1_CMD, EOI);
            outb(PIC2_CMD, EOI);
        }
    }
    
    /// Маскирование IRQ линии
    pub fn mask_irq(irq: usize) {
        let port = if irq < 8 { PIC1_DATA } else { PIC2_DATA };
        let bit = if irq < 8 { irq } else { irq - 8 };
        
        unsafe {
            let mut mask = inb(port);
            mask |= 1 << bit;
            outb(port, mask);
        }
    }
    
    /// Размаскирование IRQ линии
    pub fn unmask_irq(irq: usize) {
        let port = if irq < 8 { PIC1_DATA } else { PIC2_DATA };
        let bit = if irq < 8 { irq } else { irq - 8 };
        
        unsafe {
            let mut mask = inb(port);
            mask &= !(1 << bit);
            outb(port, mask);
        }
    }
    
    /// Получение маски прерываний
    pub fn get_mask() -> u16 {
        unsafe {
            (inb(PIC1_DATA) as u16) | ((inb(PIC2_DATA) as u16) << 8)
        }
    }
}
