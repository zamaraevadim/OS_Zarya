//! Драйвер клавиатуры PS/2
//! 
//! Обрабатывает скан-коды от клавиатуры и преобразует их в ASCII символы.
//! Поддерживает базовую раскладку US QWERTY.

use crate::drivers::{inb, outb, io_delay};
use crate::sync::Spinlock;

/// Порт данных контроллера PS/2
const PS2_DATA_PORT: u16 = 0x60;
/// Порт команд контроллера PS/2
const PS2_CMD_PORT: u16 = 0x64;

/// Скан-код нажатия (make code)
const KEY_PRESSED: u8 = 0x80;

/// Таблица соответствия скан-кодов ASCII (US QWERTY)
static SCANCODE_TABLE: &[char] = &[
    '?', '?', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\b',
    '\t', 'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', '\n',
    '?', 'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', ';', '\'', '`',
    '?', '\\', 'z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/', '?',
    '*', '?', ' ',
];

/// Расширенная таблица для специальных клавиш
const EXT_SCANCODE_F1: u8 = 0x3B;
const EXT_SCANCODE_F2: u8 = 0x3C;
const EXT_SCANCODE_F3: u8 = 0x3D;
const EXT_SCANCODE_F4: u8 = 0x3E;
const EXT_SCANCODE_F5: u8 = 0x3F;
const EXT_SCANCODE_F6: u8 = 0x40;
const EXT_SCANCODE_F7: u8 = 0x41;
const EXT_SCANCODE_F8: u8 = 0x42;
const EXT_SCANCODE_F9: u8 = 0x43;
const EXT_SCANCODE_F10: u8 = 0x44;
const EXT_SCANCODE_F11: u8 = 0x57;
const EXT_SCANCODE_F12: u8 = 0x58;

/// Специальные скан-коды
const SCANCODE_LSHIFT: u8 = 0x2A;
const SCANCODE_RSHIFT: u8 = 0x36;
const SCANCODE_LCTRL: u8 = 0x1D;
const SCANCODE_LALT: u8 = 0x38;
const SCANCODE_CAPSLOCK: u8 = 0x3A;
const SCANCODE_NUMLOCK: u8 = 0x45;
const SCANCODE_SCROLLLOCK: u8 = 0x46;

/// Буфер клавиатуры
static KEYBOARD_BUFFER: Spinlock<KeyboardBuffer> = Spinlock::new(KeyboardBuffer::new());

/// Размер буфера клавиатуры
const BUFFER_SIZE: usize = 256;

/// Буфер нажатых клавиш
struct KeyboardBuffer {
    buffer: [KeyEvent; BUFFER_SIZE],
    read_pos: usize,
    write_pos: usize,
}

impl KeyboardBuffer {
    const fn new() -> Self {
        KeyboardBuffer {
            buffer: [KeyEvent::empty(); BUFFER_SIZE],
            read_pos: 0,
            write_pos: 0,
        }
    }
    
    /// Добавление события в буфер
    fn push(&mut self, event: KeyEvent) {
        let next_write = (self.write_pos + 1) % BUFFER_SIZE;
        
        if next_write != self.read_pos {
            self.buffer[self.write_pos] = event;
            self.write_pos = next_write;
        }
    }
    
    /// Извлечение события из буфера
    fn pop(&mut self) -> Option<KeyEvent> {
        if self.read_pos == self.write_pos {
            return None;
        }
        
        let event = self.buffer[self.read_pos];
        self.read_pos = (self.read_pos + 1) % BUFFER_SIZE;
        Some(event)
    }
    
    /// Проверка наличия событий
    fn is_empty(&self) -> bool {
        self.read_pos == self.write_pos
    }
}

/// Событие клавиатуры
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    /// Скан-код клавиши
    pub scancode: u8,
    /// Была ли клавиша нажата (true) или отпущена (false)
    pub pressed: bool,
    /// ASCII символ (если есть)
    pub character: Option<char>,
    /// Модификаторы
    pub modifiers: ModifierKeys,
}

impl KeyEvent {
    const fn empty() -> Self {
        KeyEvent {
            scancode: 0,
            pressed: false,
            character: None,
            modifiers: ModifierKeys::empty(),
        }
    }
}

bitflags::bitflags! {
    /// Модификаторы клавиатуры
    pub struct ModifierKeys: u8 {
        const SHIFT = 0b0001;
        const CTRL = 0b0010;
        const ALT = 0b0100;
        const CAPS = 0b1000;
    }
}

/// Текущее состояние модификаторов
static MODIFIERS: Spinlock<ModifierKeys> = Spinlock::new(ModifierKeys::empty());

/// Инициализация драйвера клавиатуры
pub fn init() {
    // Очистка буфера ввода
    unsafe {
        while inb(PS2_CMD_PORT) & 0x01 != 0 {
            inb(PS2_DATA_PORT);
            io_delay();
        }
    }
    
    // Включаем IRQ1 (клавиатура)
    crate::drivers::pic::unmask_irq(crate::drivers::irq::KEYBOARD);
    
    kernel_log!("[KEYBOARD] PS/2 keyboard initialized\n");
}

/// Обработка скан-кода от клавиатуры
pub fn handle_scancode(scancode: u8) {
    // Проверяем, отпущена ли клавиша (бит 7 установлен)
    let released = (scancode & KEY_PRESSED) != 0;
    let key = scancode & !KEY_PRESSED;
    
    let mut modifiers = MODIFIERS.lock();
    
    // Обновляем состояние модификаторов
    match key {
        SCANCODE_LSHIFT | SCANCODE_RSHIFT => {
            if released {
                modifiers.remove(ModifierKeys::SHIFT);
            } else {
                modifiers.insert(ModifierKeys::SHIFT);
            }
            return;
        }
        SCANCODE_LCTRL => {
            if released {
                modifiers.remove(ModifierKeys::CTRL);
            } else {
                modifiers.insert(ModifierKeys::CTRL);
            }
            return;
        }
        SCANCODE_LALT => {
            if released {
                modifiers.remove(ModifierKeys::ALT);
            } else {
                modifiers.insert(ModifierKeys::ALT);
            }
            return;
        }
        SCANCODE_CAPSLOCK => {
            if !released {
                modifiers.toggle(ModifierKeys::CAPS);
            }
            return;
        }
        _ => {}
    }
    
    // Преобразуем скан-код в символ
    let character = if (key as usize) < SCANCODE_TABLE.len() {
        let mut c = SCANCODE_TABLE[key as usize];
        
        // Применяем модификаторы
        if modifiers.contains(ModifierKeys::SHIFT) {
            c = apply_shift(c);
        } else if modifiers.contains(ModifierKeys::CAPS) && c.is_alphabetic() {
            c = apply_caps(c);
        }
        
        Some(c)
    } else {
        None
    };
    
    // Создаём событие
    let event = KeyEvent {
        scancode: key,
        pressed: !released,
        character,
        modifiers: *modifiers,
    };
    
    drop(modifiers);
    
    // Добавляем в буфер только нажатия
    if !released {
        KEYBOARD_BUFFER.lock().push(event);
        
        kernel_log!("[KEYBOARD] Scancode: 0x{:02X}, Char: {:?}\n", 
                   key, character.unwrap_or('?'));
    }
}

/// Применение Shift к символу
fn apply_shift(c: char) -> char {
    match c {
        'a'..='z' => c.to_ascii_uppercase(),
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        '\\' => '|',
        '`' => '~',
        _ => c,
    }
}

/// Применение Caps Lock к символу
fn apply_caps(c: char) -> char {
    if c.is_lowercase() {
        c.to_ascii_uppercase()
    } else if c.is_uppercase() {
        c.to_ascii_lowercase()
    } else {
        c
    }
}

/// Получение следующего события клавиатуры
pub fn get_event() -> Option<KeyEvent> {
    KEYBOARD_BUFFER.lock().pop()
}

/// Чтение символа с клавиатуры (blocking)
pub fn read_char() -> char {
    loop {
        if let Some(event) = get_event() {
            if event.pressed {
                if let Some(c) = event.character {
                    return c;
                }
            }
        }
        
        // Ждём следующее прерывание
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

/// Чтение строки с клавиатуры
pub fn read_line(buf: &mut [u8]) -> usize {
    let mut pos = 0;
    
    loop {
        let c = read_char();
        
        match c {
            '\n' | '\r' => {
                buf[pos] = b'\n';
                return pos + 1;
            }
            '\b' => {
                if pos > 0 {
                    pos -= 1;
                }
            }
            _ => {
                if pos < buf.len() - 1 {
                    buf[pos] = c as u8;
                    pos += 1;
                }
            }
        }
    }
}

/// Проверка наличия нажатий в буфере
pub fn has_input() -> bool {
    !KEYBOARD_BUFFER.lock().is_empty()
}
