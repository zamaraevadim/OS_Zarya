//! Драйвер мыши PS/2
//! 
//! Обрабатывает события от мыши PS/2 (движение, кнопки).

use crate::drivers::{inb, outb, io_delay};
use crate::sync::Spinlock;

/// Порт данных контроллера PS/2
const PS2_DATA_PORT: u16 = 0x60;
/// Порт команд контроллера PS/2
const PS2_CMD_PORT: u16 = 0x64;

/// Команды PS/2 для мыши
const MOUSE_ENABLE: u8 = 0xF4;
const MOUSE_DISABLE: u8 = 0xF5;
const MOUSE_SET_DEFAULT: u8 = 0xF6;
const MOUSE_SET_SAMPLE_RATE: u8 = 0xF3;
const MOUSE_SET_RESOLUTION: u8 = 0xE8;
const MOUSE_GET_ID: u8 = 0xF2;

/// Буфер событий мыши
static MOUSE_BUFFER: Spinlock<MouseBuffer> = Spinlock::new(MouseBuffer::new());

/// Размер буфера мыши
const BUFFER_SIZE: usize = 64;

/// Буфер событий мыши
struct MouseBuffer {
    buffer: [MouseEvent; BUFFER_SIZE],
    read_pos: usize,
    write_pos: usize,
}

impl MouseBuffer {
    const fn new() -> Self {
        MouseBuffer {
            buffer: [MouseEvent::empty(); BUFFER_SIZE],
            read_pos: 0,
            write_pos: 0,
        }
    }
    
    fn push(&mut self, event: MouseEvent) {
        let next_write = (self.write_pos + 1) % BUFFER_SIZE;
        
        if next_write != self.read_pos {
            self.buffer[self.write_pos] = event;
            self.write_pos = next_write;
        }
    }
    
    fn pop(&mut self) -> Option<MouseEvent> {
        if self.read_pos == self.write_pos {
            return None;
        }
        
        let event = self.buffer[self.read_pos];
        self.read_pos = (self.read_pos + 1) % BUFFER_SIZE;
        Some(event)
    }
    
    fn is_empty(&self) -> bool {
        self.read_pos == self.write_pos
    }
}

bitflags::bitflags! {
    /// Состояние кнопок мыши
    pub struct MouseButtons: u8 {
        const LEFT = 0b0001;
        const RIGHT = 0b010;
        const MIDDLE = 0b0100;
    }
}

/// Событие мыши
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    /// Состояние кнопок
    pub buttons: MouseButtons,
    /// Смещение по X
    pub dx: i16,
    /// Смещение по Y
    pub dy: i16,
    /// Смещение колеса прокрутки
    pub dz: i8,
}

impl MouseEvent {
    const fn empty() -> Self {
        MouseEvent {
            buttons: MouseButtons::empty(),
            dx: 0,
            dy: 0,
            dz: 0,
        }
    }
}

/// Текущее состояние мыши
static MOUSE_STATE: Spinlock<MouseState> = Spinlock::new(MouseState::new());

/// Состояние мыши
struct MouseState {
    current_x: i32,
    current_y: i32,
    buttons: MouseButtons,
    enabled: bool,
}

impl MouseState {
    const fn new() -> Self {
        MouseState {
            current_x: 0,
            current_y: 0,
            buttons: MouseButtons::empty(),
            enabled: false,
        }
    }
}

/// Инициализация драйвера мыши
pub fn init() {
    kernel_log!("[MOUSE] Initializing PS/2 mouse...\n");
    
    unsafe {
        // Отправляем команду установки мыши
        outb(PS2_CMD_PORT, 0xA8); // Enable AUX port
        io_delay();
        
        // Включаем прерывания мыши
        outb(PS2_CMD_PORT, 0x20); // Read controller command byte
        io_delay();
        let mut status = inb(PS2_DATA_PORT);
        io_delay();
        
        status |= 0x02; // Set bit 1 (AUX interrupt enable)
        outb(PS2_CMD_PORT, 0x60); // Write controller command byte
        io_delay();
        outb(PS2_DATA_PORT, status);
        io_delay();
        
        // Включаем мышь
        outb(PS2_DATA_PORT, MOUSE_ENABLE);
        io_delay();
        
        // Читаем ответ
        let _response = inb(PS2_DATA_PORT);
    }
    
    // Включаем IRQ12 (мышь)
    crate::drivers::pic::unmask_irq(crate::drivers::irq::PS2_MOUSE);
    
    MOUSE_STATE.lock().enabled = true;
    
    kernel_log!("[MOUSE] PS/2 mouse initialized\n");
}

/// Обработка данных от мыши
/// 
/// Данные приходят в формате 3 байт:
/// Byte 0: Buttons | X overflow | Y overflow | X sign | Y sign | Always 1 | Middle | Right | Left
/// Byte 1: Delta X
/// Byte 2: Delta Y
pub fn handle_mouse_event(buttons_byte: u8, dx: i8, dy: i8) {
    let mut state = MOUSE_STATE.lock();
    
    // Парсим биты кнопок из первого байта
    let mut buttons = MouseButtons::empty();
    
    if buttons_byte & 0x01 != 0 {
        buttons.insert(MouseButtons::LEFT);
    }
    if buttons_byte & 0x02 != 0 {
        buttons.insert(MouseButtons::RIGHT);
    }
    if buttons_byte & 0x04 != 0 {
        buttons.insert(MouseButtons::MIDDLE);
    }
    
    // Преобразуем смещения с учётом знака
    let dx = if buttons_byte & 0x10 != 0 {
        // Отрицательное значение (дополнительный код)
        !(dx as i16)
    } else {
        dx as i16
    };
    
    let dy = if buttons_byte & 0x20 != 0 {
        // Отрицательное значение
        -(dy as i16)
    } else {
        dy as i16
    };
    
    // Обновляем позицию
    state.current_x += dx as i32;
    state.current_y -= dy as i32; // Y инвертирован в экранных координатах
    state.buttons = buttons;
    
    drop(state);
    
    // Создаём событие
    let event = MouseEvent {
        buttons,
        dx: dx as i16,
        dy,
        dz: 0,
    };
    
    MOUSE_BUFFER.lock().push(event);
    
    kernel_log!("[MOUSE] Buttons: {:?}, dX: {}, dY: {}\n", buttons, dx, dy);
}

/// Получение текущего события мыши
pub fn get_event() -> Option<MouseEvent> {
    MOUSE_BUFFER.lock().pop()
}

/// Получение текущей позиции курсора
pub fn get_position() -> (i32, i32) {
    let state = MOUSE_STATE.lock();
    (state.current_x, state.current_y)
}

/// Установка позиции курсора
pub fn set_position(x: i32, y: i32) {
    let mut state = MOUSE_STATE.lock();
    state.current_x = x;
    state.current_y = y;
}

/// Проверка состояния кнопки
pub fn is_button_pressed(button: MouseButtons) -> bool {
    MOUSE_STATE.lock().buttons.contains(button)
}

/// Включение/выключение мыши
pub fn set_enabled(enabled: bool) {
    let mut state = MOUSE_STATE.lock();
    
    if enabled && !state.enabled {
        unsafe {
            outb(PS2_DATA_PORT, MOUSE_ENABLE);
            io_delay();
            let _ = inb(PS2_DATA_PORT);
        }
        state.enabled = true;
    } else if !enabled && state.enabled {
        unsafe {
            outb(PS2_DATA_PORT, MOUSE_DISABLE);
            io_delay();
            let _ = inb(PS2_DATA_PORT);
        }
        state.enabled = false;
    }
}

/// Получение статуса мыши
pub fn is_enabled() -> bool {
    MOUSE_STATE.lock().enabled
}
