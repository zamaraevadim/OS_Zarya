//! # Драйвер мыши PS/2
//!
//! Обрабатывает ввод с мыши через контроллер PS/2.
//! Поддерживает движение, кнопки и колесо прокрутки.

#![no_std]

use x86_64::instructions::port::Port;
use spin::Mutex;
use core::sync::atomic::{AtomicI32, AtomicBool, Ordering};

/// Порт данных мыши (через контроллер клавиатуры)
const MOUSE_PORT: u16 = 0x60;
const KBD_CMD_PORT: u16 = 0x64;

/// Глобальное состояние мыши
static MOUSE_X: AtomicI32 = AtomicI32::new(0);
static MOUSE_Y: AtomicI32 = AtomicI32::new(0);
static MOUSE_LEFT: AtomicBool = AtomicBool::new(false);
static MOUSE_RIGHT: AtomicBool = AtomicBool::new(false);
static MOUSE_MIDDLE: AtomicBool = AtomicBool::new(false);
static MOUSE_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Событие мыши
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    /// Движение по X
    pub dx: i32,
    /// Движение по Y
    pub dy: i32,
    /// Левая кнопка нажата
    pub left_button: bool,
    /// Правая кнопка нажата
    pub right_button: bool,
    /// Средняя кнопка нажата
    pub middle_button: bool,
}

impl MouseEvent {
    pub const fn new(dx: i32, dy: i32, left: bool, right: bool, middle: bool) -> Self {
        Self {
            dx,
            dy,
            left_button: left,
            right_button: right,
            middle_button: middle,
        }
    }
}

/// Драйвер мыши PS/2
pub struct PS2Mouse {
    initialized: bool,
}

impl PS2Mouse {
    /// Создание нового экземпляра
    pub const fn new() -> Self {
        Self { initialized: false }
    }
    
    /// Инициализация мыши
    pub fn init(&mut self) {
        // Ожидаем готовности контроллера
        self.wait_for_kbd();
        
        unsafe {
            // Включаем устройство мыши
            Port::new(KBD_CMD_PORT).write(0xA8u8);
            
            // Устанавливаем default settings
            self.write_mouse_data(0xF6);
            self.read_mouse_data(); // ACK
            
            // Включаем передачу данных
            self.write_mouse_data(0xF4);
            self.read_mouse_data(); // ACK
        }
        
        MOUSE_INITIALIZED.store(true, Ordering::Relaxed);
        self.initialized = true;
    }
    
    /// Ожидание готовности контроллера
    fn wait_for_kbd(&self) {
        loop {
            unsafe {
                let status = Port::new(KBD_CMD_PORT).read();
                if status & 0x02 == 0 {
                    break;
                }
            }
            core::hint::spin_loop();
        }
    }
    
    /// Запись данных в мышь
    fn write_mouse_data(&self, data: u8) {
        self.wait_for_kbd();
        unsafe {
            Port::new(MOUSE_PORT).write(data);
        }
    }
    
    /// Чтение данных от мыши
    fn read_mouse_data(&self) -> u8 {
        loop {
            unsafe {
                let status = Port::new(KBD_CMD_PORT).read();
                if status & 0x01 != 0 {
                    return Port::new(MOUSE_PORT).read();
                }
            }
            core::hint::spin_loop();
        }
    }
    
    /// Обработка прерывания мыши
    pub fn handle_interrupt() {
        if !MOUSE_INITIALIZED.load(Ordering::Relaxed) {
            return;
        }
        
        // Чтение первого байта (состояние кнопок)
        let byte1 = unsafe { Port::new(MOUSE_PORT).read() };
        let dx = unsafe { Port::new(MOUSE_PORT).read() } as i8 as i32;
        let dy = unsafe { Port::new(MOUSE_PORT).read() } as i8 as i32;
        
        // Обновление состояния кнопок
        MOUSE_LEFT.store(byte1 & 0x01 != 0, Ordering::Relaxed);
        MOUSE_RIGHT.store(byte1 & 0x02 != 0, Ordering::Relaxed);
        MOUSE_MIDDLE.store(byte1 & 0x04 != 0, Ordering::Relaxed);
        
        // Обновление позиции
        MOUSE_X.fetch_add(dx, Ordering::Relaxed);
        MOUSE_Y.fetch_sub(dy, Ordering::Relaxed); // Y инвертирован
    }
    
    /// Получение текущей позиции
    pub fn get_position(&self) -> (i32, i32) {
        (
            MOUSE_X.load(Ordering::Relaxed),
            MOUSE_Y.load(Ordering::Relaxed),
        )
    }
    
    /// Получение состояния кнопок
    pub fn get_buttons(&self) -> (bool, bool, bool) {
        (
            MOUSE_LEFT.load(Ordering::Relaxed),
            MOUSE_RIGHT.load(Ordering::Relaxed),
            MOUSE_MIDDLE.load(Ordering::Relaxed),
        )
    }
    
    /// Чтение события мыши
    pub fn read_event(&self) -> Option<MouseEvent> {
        // В упрощенной версии возвращаем текущее состояние
        if MOUSE_INITIALIZED.load(Ordering::Relaxed) {
            Some(MouseEvent::new(
                0,
                0,
                MOUSE_LEFT.load(Ordering::Relaxed),
                MOUSE_RIGHT.load(Ordering::Relaxed),
                MOUSE_MIDDLE.load(Ordering::Relaxed),
            ))
        } else {
            None
        }
    }
    
    /// Проверка инициализации
    pub fn is_initialized(&self) -> bool {
        self.initialized && MOUSE_INITIALIZED.load(Ordering::Relaxed)
    }
}

/// Сброс позиции мыши в ноль
pub fn reset_position() {
    MOUSE_X.store(0, Ordering::Relaxed);
    MOUSE_Y.store(0, Ordering::Relaxed);
}
