//! # Драйвер клавиатуры PS/2
//!
//! Обрабатывает ввод с клавиатуры через контроллер PS/2.
//! Поддерживает скан-коды Set 1, модификаторы (Shift, Ctrl, Alt).

#![no_std]

use x86_64::instructions::port::Port;
use spin::Mutex;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// Порт данных клавиатуры
const KBD_DATA_PORT: u16 = 0x60;
/// Порт команд контроллера
const KBD_CMD_PORT: u16 = 0x64;

/// Глобальный буфер клавиатуры
static KEYBOARD_BUFFER: Mutex<[u8; 256]> = Mutex::new([0; 256]);
/// Индекс записи в буфер
static BUFFER_WRITE_INDEX: AtomicU8 = AtomicU8::new(0);
/// Индекс чтения из буфера
static BUFFER_READ_INDEX: AtomicU8 = AtomicU8::new(0);
/// Флаг наличия данных в буфере
static KEYBOARD_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Состояние модификаторов
static SHIFT_PRESSED: AtomicBool = AtomicBool::new(false);
static CTRL_PRESSED: AtomicBool = AtomicBool::new(false);
static ALT_PRESSED: AtomicBool = AtomicBool::new(false);
static CAPS_LOCK: AtomicBool = AtomicBool::new(false);

/// Коды клавиш (scan code set 1)
pub mod scan_codes {
    pub const ESCAPE: u8 = 0x01;
    pub const KEY_1: u8 = 0x02;
    pub const KEY_2: u8 = 0x03;
    pub const KEY_3: u8 = 0x04;
    pub const KEY_4: u8 = 0x05;
    pub const KEY_5: u8 = 0x06;
    pub const KEY_6: u8 = 0x07;
    pub const KEY_7: u8 = 0x08;
    pub const KEY_8: u8 = 0x09;
    pub const KEY_9: u8 = 0x0A;
    pub const KEY_0: u8 = 0x0B;
    pub const MINUS: u8 = 0x0C;
    pub const EQUALS: u8 = 0x0D;
    pub const BACKSPACE: u8 = 0x0E;
    pub const TAB: u8 = 0x0F;
    pub const Q: u8 = 0x10;
    pub const W: u8 = 0x11;
    pub const E: u8 = 0x12;
    pub const R: u8 = 0x13;
    pub const T: u8 = 0x14;
    pub const Y: u8 = 0x15;
    pub const U: u8 = 0x16;
    pub const I: u8 = 0x17;
    pub const O: u8 = 0x18;
    pub const P: u8 = 0x19;
    pub const LBRACKET: u8 = 0x1A;
    pub const RBRACKET: u8 = 0x1B;
    pub const ENTER: u8 = 0x1C;
    pub const LCTRL: u8 = 0x1D;
    pub const A: u8 = 0x1E;
    pub const S: u8 = 0x1F;
    pub const D: u8 = 0x20;
    pub const F: u8 = 0x21;
    pub const G: u8 = 0x22;
    pub const H: u8 = 0x23;
    pub const J: u8 = 0x24;
    pub const K: u8 = 0x25;
    pub const L: u8 = 0x26;
    pub const SEMICOLON: u8 = 0x27;
    pub const QUOTE: u8 = 0x28;
    pub const BACKTICK: u8 = 0x29;
    pub const LSHIFT: u8 = 0x2A;
    pub const BACKSLASH: u8 = 0x2B;
    pub const Z: u8 = 0x2C;
    pub const X: u8 = 0x2D;
    pub const C: u8 = 0x2E;
    pub const V: u8 = 0x2F;
    pub const B: u8 = 0x30;
    pub const N: u8 = 0x31;
    pub const M: u8 = 0x32;
    pub const COMMA: u8 = 0x33;
    pub const DOT: u8 = 0x34;
    pub const SLASH: u8 = 0x35;
    pub const RSHIFT: u8 = 0x36;
    pub const SPACE: u8 = 0x39;
    pub const CAPSLOCK: u8 = 0x3A;
    pub const F1: u8 = 0x3B;
    pub const F2: u8 = 0x3C;
    pub const F3: u8 = 0x3D;
    pub const F4: u8 = 0x3E;
    pub const F5: u8 = 0x3F;
    pub const F6: u8 = 0x40;
    pub const F7: u8 = 0x41;
    pub const F8: u8 = 0x42;
    pub const F9: u8 = 0x43;
    pub const F10: u8 = 0x44;
    pub const NUMLOCK: u8 = 0x45;
    pub const SCROLLLOCK: u8 = 0x46;
    pub const HOME: u8 = 0x47;
    pub const UP: u8 = 0x48;
    pub const PAGEUP: u8 = 0x49;
    pub const LEFT: u8 = 0x4B;
    pub const RIGHT: u8 = 0x4D;
    pub const END: u8 = 0x4F;
    pub const DOWN: u8 = 0x50;
    pub const PAGEDOWN: u8 = 0x51;
    pub const INSERT: u8 = 0x52;
    pub const DELETE: u8 = 0x53;
}

/// Событие клавиатуры
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    /// Scan код клавиши
    pub scan_code: u8,
    /// Была ли клавиша отпущена
    pub released: bool,
    /// ASCII символ (если есть)
    pub character: Option<char>,
}

impl KeyEvent {
    pub const fn new(scan_code: u8, released: bool, character: Option<char>) -> Self {
        Self {
            scan_code,
            released,
            character,
        }
    }
}

/// Драйвер клавиатуры PS/2
pub struct PS2Keyboard {
    initialized: bool,
}

impl PS2Keyboard {
    /// Создание нового экземпляра
    pub const fn new() -> Self {
        Self { initialized: false }
    }
    
    /// Инициализация клавиатуры
    pub fn init(&mut self) {
        // Ожидаем пока контроллер не будет готов
        self.wait_for_kbd();
        
        // Включаем сканирование клавиатуры
        unsafe {
            Port::new(KBD_CMD_PORT).write(0xAEu8); // Enable keyboard
            Port::new(KBD_DATA_PORT).write(0xF4u8); // Enable scanning
        }
        
        self.initialized = true;
    }
    
    /// Ожидание готовности контроллера клавиатуры
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
    
    /// Чтение скан-кода из порта
    fn read_scan_code(&self) -> u8 {
        unsafe {
            Port::new(KBD_DATA_PORT).read()
        }
    }
    
    /// Обработка прерывания клавиатуры
    pub fn handle_interrupt() {
        let scan_code = unsafe { Port::new(KBD_DATA_PORT).read() };
        
        // Проверяем бит отпускания клавиши (бит 7)
        let released = (scan_code & 0x80) != 0;
        let key = scan_code & 0x7F;
        
        // Обработка модификаторов
        match key {
            scan_codes::LSHIFT | scan_codes::RSHIFT => {
                SHIFT_PRESSED.store(!released, Ordering::Relaxed);
                return;
            }
            scan_codes::LCTRL => {
                CTRL_PRESSED.store(!released, Ordering::Relaxed);
                return;
            }
            scan_codes::ALT => {
                ALT_PRESSED.store(!released, Ordering::Relaxed);
                return;
            }
            scan_codes::CAPSLOCK => {
                if !released {
                    CAPS_LOCK.fetch_xor(true, Ordering::Relaxed);
                }
                return;
            }
            _ => {}
        }
        
        // Преобразование скан-кода в символ
        let character = if !released {
            scan_code_to_char(key, SHIFT_PRESSED.load(Ordering::Relaxed), CAPS_LOCK.load(Ordering::Relaxed))
        } else {
            None
        };
        
        // Добавление в буфер
        let mut buffer = KEYBOARD_BUFFER.lock();
        let write_idx = BUFFER_WRITE_INDEX.load(Ordering::Relaxed);
        let next_idx = (write_idx + 1) % 256;
        
        if next_idx != BUFFER_READ_INDEX.load(Ordering::Relaxed) {
            buffer[write_idx as usize] = scan_code;
            BUFFER_WRITE_INDEX.store(next_idx, Ordering::Relaxed);
            KEYBOARD_AVAILABLE.store(true, Ordering::Relaxed);
        }
    }
    
    /// Чтение события из буфера (блокирующее)
    pub fn read_event(&self) -> Option<KeyEvent> {
        if !KEYBOARD_AVAILABLE.load(Ordering::Relaxed) {
            return None;
        }
        
        let buffer = KEYBOARD_BUFFER.lock();
        let read_idx = BUFFER_READ_INDEX.load(Ordering::Relaxed);
        
        if read_idx == BUFFER_WRITE_INDEX.load(Ordering::Relaxed) {
            return None;
        }
        
        let scan_code = buffer[read_idx as usize];
        let next_idx = (read_idx + 1) % 256;
        BUFFER_READ_INDEX.store(next_idx, Ordering::Relaxed);
        
        let released = (scan_code & 0x80) != 0;
        let key = scan_code & 0x7F;
        
        let character = if !released {
            scan_code_to_char(key, SHIFT_PRESSED.load(Ordering::Relaxed), CAPS_LOCK.load(Ordering::Relaxed))
        } else {
            None
        };
        
        Some(KeyEvent::new(scan_code, released, character))
    }
    
    /// Проверка наличия данных в буфере
    pub fn has_data(&self) -> bool {
        KEYBOARD_AVAILABLE.load(Ordering::Relaxed) && 
        BUFFER_READ_INDEX.load(Ordering::Relaxed) != BUFFER_WRITE_INDEX.load(Ordering::Relaxed)
    }
    
    /// Получение ASCII символа (блокирующее)
    pub fn read_char(&self) -> Option<char> {
        loop {
            if let Some(event) = self.read_event() {
                if !event.released {
                    if let Some(c) = event.character {
                        return Some(c);
                    }
                }
            }
            
            if !self.has_data() {
                return None;
            }
        }
    }
}

/// Преобразование скан-кода в ASCII символ
fn scan_code_to_char(scan_code: u8, shift: bool, caps_lock: bool) -> Option<char> {
    let c = match scan_code {
        scan_codes::KEY_1 => if shift { '!' } else { '1' },
        scan_codes::KEY_2 => if shift { '@' } else { '2' },
        scan_codes::KEY_3 => if shift { '#' } else { '3' },
        scan_codes::KEY_4 => if shift { '$' } else { '4' },
        scan_codes::KEY_5 => if shift { '%' } else { '5' },
        scan_codes::KEY_6 => if shift { '^' } else { '6' },
        scan_codes::KEY_7 => if shift { '&' } else { '7' },
        scan_codes::KEY_8 => if shift { '*' } else { '8' },
        scan_codes::KEY_9 => if shift { '(' } else { '9' },
        scan_codes::KEY_0 => if shift { ')' } else { '0' },
        scan_codes::MINUS => if shift { '_' } else { '-' },
        scan_codes::EQUALS => if shift { '+' } else { '=' },
        scan_codes::Q => apply_caps('q', shift, caps_lock),
        scan_codes::W => apply_caps('w', shift, caps_lock),
        scan_codes::E => apply_caps('e', shift, caps_lock),
        scan_codes::R => apply_caps('r', shift, caps_lock),
        scan_codes::T => apply_caps('t', shift, caps_lock),
        scan_codes::Y => apply_caps('y', shift, caps_lock),
        scan_codes::U => apply_caps('u', shift, caps_lock),
        scan_codes::I => apply_caps('i', shift, caps_lock),
        scan_codes::O => apply_caps('o', shift, caps_lock),
        scan_codes::P => apply_caps('p', shift, caps_lock),
        scan_codes::A => apply_caps('a', shift, caps_lock),
        scan_codes::S => apply_caps('s', shift, caps_lock),
        scan_codes::D => apply_caps('d', shift, caps_lock),
        scan_codes::F => apply_caps('f', shift, caps_lock),
        scan_codes::G => apply_caps('g', shift, caps_lock),
        scan_codes::H => apply_caps('h', shift, caps_lock),
        scan_codes::J => apply_caps('j', shift, caps_lock),
        scan_codes::K => apply_caps('k', shift, caps_lock),
        scan_codes::L => apply_caps('l', shift, caps_lock),
        scan_codes::Z => apply_caps('z', shift, caps_lock),
        scan_codes::X => apply_caps('x', shift, caps_lock),
        scan_codes::C => apply_caps('c', shift, caps_lock),
        scan_codes::V => apply_caps('v', shift, caps_lock),
        scan_codes::B => apply_caps('b', shift, caps_lock),
        scan_codes::N => apply_caps('n', shift, caps_lock),
        scan_codes::M => apply_caps('m', shift, caps_lock),
        scan_codes::SPACE => ' ',
        scan_codes::ENTER => '\n',
        scan_codes::TAB => '\t',
        scan_codes::BACKSPACE => '\x08',
        scan_codes::SEMICOLON => if shift { ':' } else { ';' },
        scan_codes::QUOTE => if shift { '"' } else { '\'' },
        scan_codes::BACKTICK => if shift { '~' } else { '`' },
        scan_codes::COMMA => if shift { '<' } else { ',' },
        scan_codes::DOT => if shift { '>' } else { '.' },
        scan_codes::SLASH => if shift { '?' } else { '/' },
        scan_codes::LBRACKET => if shift { '{' } else { '[' },
        scan_codes::RBRACKET => if shift { '}' } else { ']' },
        scan_codes::BACKSLASH => if shift { '|' } else { '\\' },
        _ => return None,
    };
    
    Some(c)
}

/// Применение Caps Lock к символу
fn apply_caps(c: char, shift: bool, caps_lock: bool) -> char {
    if shift ^ caps_lock {
        c.to_ascii_uppercase()
    } else {
        c
    }
}

/// Проверка состояния модификаторов
pub fn get_modifiers() -> (bool, bool, bool) {
    (
        SHIFT_PRESSED.load(Ordering::Relaxed),
        CTRL_PRESSED.load(Ordering::Relaxed),
        ALT_PRESSED.load(Ordering::Relaxed),
    )
}
