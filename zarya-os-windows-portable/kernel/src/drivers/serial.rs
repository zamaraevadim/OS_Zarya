//! Драйвер последовательного порта (16550 UART)
//! 
//! Используется для отладочного вывода через COM1 (порт 0x3F8).
//! Поддерживает только синхронный вывод (blocking I/O).

use core::fmt::{self, Write};
use crate::drivers::{inb, outb, io_delay};

/// Базовый адрес COM1 порта
pub const COM1_BASE: u16 = 0x3F8;

/// Регистры UART
mod regs {
    pub const RX: u16 = 0;      // Receiver Buffer Register (read)
    pub const TX: u16 = 0;      // Transmitter Holding Register (write)
    pub const IE: u16 = 1;      // Interrupt Enable Register
    pub const FC: u16 = 2;      // FIFO Control Register (write)
    pub const IR: u16 = 2;      // Interrupt Identification Register (read)
    pub const LC: u16 = 3;      // Line Control Register
    pub const MC: u16 = 4;      // Modem Control Register
    pub const LS: u16 = 5;      // Line Status Register
    pub const MS: u16 = 6;      // Modem Status Register
    pub const SR: u16 = 7;      // Scratch Register
}

/// Биты регистра Line Status
mod ls {
    pub const DRS: u8 = 0x01;   // Data Ready
    pub const THRE: u8 = 0x20;  // Transmit Holding Register Empty
    pub const TEMT: u8 = 0x40;  // Transmitter Empty
}

/// Глобальный экземпляр SerialWriter
static mut SERIAL_WRITER: Option<SerialWriter> = None;

/// Инициализация последовательного порта
pub fn init() {
    unsafe {
        // Отключаем прерывания
        outb(COM1_BASE + regs::IE, 0x00);
        
        // Включаем DLAB (Divisor Latch Access Bit)
        outb(COM1_BASE + regs::LC, 0x80);
        
        // Устанавливаем делитель частоты для 9600 бод
        // При частоте 1.8432 MHz: divisor = 1.8432MHz / (16 * 9600) = 12
        outb(COM1_BASE + regs::TX, 0x0C);  // Low byte
        outb(COM1_BASE + regs::RX, 0x00);  // High byte
        
        // Настройка линии: 8 бит данных, 1 стоп-бит, без паритета
        outb(COM1_BASE + regs::LC, 0x03);
        
        // Включаем FIFO с порогом 14 байт
        outb(COM1_BASE + regs::FC, 0xC7);
        
        // Включаем RTS и DTR
        outb(COM1_BASE + regs::MC, 0x0B);
        
        // Создаём глобальный writer
        SERIAL_WRITER = Some(SerialWriter::new());
    }
    
    kernel_log!("[SERIAL] COM1 initialized at 0x{:x} (9600 baud)\n", COM1_BASE);
}

/// Writer для последовательного порта
pub struct SerialWriter {
    base_addr: u16,
}

impl SerialWriter {
    /// Создаёт новый SerialWriter
    pub const fn new() -> Self {
        SerialWriter {
            base_addr: COM1_BASE,
        }
    }
    
    /// Проверка готовности передатчика
    fn is_transmit_empty(&self) -> bool {
        unsafe {
            (inb(self.base_addr + regs::LS) & ls::THRE) != 0
        }
    }
    
    /// Запись одного байта
    pub fn write_byte(&self, byte: u8) {
        while !self.is_transmit_empty() {
            io_delay();
        }
        unsafe {
            outb(self.base_addr + regs::TX, byte);
        }
    }
    
    /// Запись строки
    pub fn write_str(&self, s: &str) {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
    }
}

impl Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
        Ok(())
    }
}

/// Получение глобального SerialWriter
pub fn get_writer() -> Option<&'static mut SerialWriter> {
    unsafe { SERIAL_WRITER.as_mut() }
}

/// Написание символа в serial порт
pub fn write_char(c: char) {
    if let Some(writer) = get_writer() {
        let _ = writer.write_str(&c.to_string());
    }
}

/// Написание строки в serial порт
pub fn write_str(s: &str) {
    if let Some(writer) = get_writer() {
        let _ = writer.write_str(s);
    }
}

/// Чтение одного байта (blocking)
pub fn read_byte() -> Option<u8> {
    unsafe {
        if (inb(COM1_BASE + regs::LS) & ls::DRS) != 0 {
            Some(inb(COM1_BASE + regs::RX))
        } else {
            None
        }
    }
}

/// Тестирование serial порта
pub fn test() {
    kernel_log!("[SERIAL] Running self-test...\n");
    
    let test_msg = "Serial port test message\n";
    write_str(test_msg);
    
    kernel_log!("[SERIAL] Self-test complete\n");
}
