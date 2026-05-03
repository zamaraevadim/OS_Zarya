//! # Драйвер последовательного порта (UART 16550)
//!
//! Обеспечивает вывод отладочной информации через COM1 порт.
//! Используется для раннего логгирования до инициализации других устройств.

#![no_std]

use x86_64::instructions::port::Port;
use spin::Mutex;
use core::fmt::{self, Write};

/// Базовый адрес COM1 порта
const COM1_BASE: u16 = 0x3F8;

/// Глобальный экземпляр UART (для использования из макросов print!)
static UART: Mutex<Option<Uart16550>> = Mutex::new(None);

/// Инициализация UART драйвера
pub fn init() {
    let mut uart = Uart16550::new(COM1_BASE);
    uart.init();
    *UART.lock() = Some(uart);
}

/// Написание строки в UART
pub fn write_str(s: &str) {
    let mut uart_guard = UART.lock();
    if let Some(ref mut uart) = *uart_guard {
        let _ = uart.write_str(s);
    }
}

/// Структура UART 16550
pub struct Uart16550 {
    base_port: u16,
}

impl Uart16550 {
    /// Создание нового экземпляра UART
    pub const fn new(base_port: u16) -> Self {
        Self { base_port }
    }
    
    /// Инициализация UART
    pub fn init(&mut self) {
        unsafe {
            // Отключаем прерывания
            Port::new(self.base_port + 1).write(0x00u8);
            
            // Включаем DLAB (доступ к делителю частоты)
            Port::new(self.base_port + 3).write(0x80u8);
            
            // Устанавливаем делитель частоты для 9600 бод
            // При тактовой частоте 1.8432 MHz: 1.8432MHz / (16 * 9600) = 12
            Port::new(self.base_port + 0).write(0x0Cu8); // Low byte
            Port::new(self.base_port + 1).write(0x00u8); // High byte
            
            // Настраиваем формат данных: 8 бит, без четности, 1 стоп-бит
            Port::new(self.base_port + 3).write(0x03u8);
            
            // Включаем FIFO буфер
            Port::new(self.base_port + 2).write(0xC7u8);
            
            // Включаем RTS и DTR
            Port::new(self.base_port + 4).write(0x0Bu8);
        }
    }
    
    /// Проверка, готов ли передатчик
    #[inline]
    fn is_transmit_empty(&self) -> bool {
        unsafe {
            Port::new(self.base_port + 5).read() & 0x20 != 0
        }
    }
    
    /// Отправка одного байта
    fn send_byte(&mut self, data: u8) {
        while !self.is_transmit_empty() {
            core::hint::spin_loop();
        }
        unsafe {
            Port::new(self.base_port).write(data);
        }
    }
    
    /// Получение одного байта (блокирующее)
    pub fn recv_byte(&mut self) -> Option<u8> {
        unsafe {
            if Port::new(self.base_port + 5).read() & 0x01 != 0 {
                Some(Port::new(self.base_port).read())
            } else {
                None
            }
        }
    }
}

impl fmt::Write for Uart16550 {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.send_byte(b'\r');
            }
            self.send_byte(byte);
        }
        Ok(())
    }
}

/// Обертка для использования с глобальным UART
pub struct SerialPort;

impl SerialPort {
    /// Создание нового экземпляра
    pub const fn new() -> Self {
        Self
    }
    
    /// Инициализация
    pub fn init(&self) {
        init();
    }
    
    /// Написание формата
    pub fn write_fmt(args: fmt::Arguments) {
        use core::fmt::Write;
        
        let mut uart_guard = UART.lock();
        if let Some(ref mut uart) = *uart_guard {
            let _ = uart.write_fmt(args);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_uart_creation() {
        let uart = Uart16550::new(COM1_BASE);
        assert_eq!(uart.base_port, COM1_BASE);
    }
}
