//! # Драйверы устройств операционной системы Zarya
//!
//! Модуль содержит драйверы для базовых устройств:
//! - Последовательный порт (UART 16550)
//! - Клавиатура PS/2
//! - Мышь PS/2
//! - Framebuffer (графический буфер)

#![no_std]

pub mod serial;
pub mod keyboard;
pub mod mouse;
pub mod framebuffer;

pub use serial::SerialPort;
pub use keyboard::PS2Keyboard;
pub use mouse::PS2Mouse;
pub use framebuffer::Framebuffer;

/// Trait для всех драйверов устройств
pub trait DeviceDriver {
    /// Инициализация устройства
    fn init(&mut self);
    
    /// Проверка наличия устройства
    fn detect(&self) -> bool;
    
    /// Получение имени устройства
    fn name(&self) -> &'static str;
}

/// Базовая структура для прерываний устройств
#[derive(Debug, Clone, Copy)]
pub struct InterruptInfo {
    pub irq: u8,
    pub vector: u8,
}
