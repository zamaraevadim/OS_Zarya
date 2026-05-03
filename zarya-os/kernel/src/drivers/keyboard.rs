//! Keyboard Driver (PS/2)
//! 
//! Handles keyboard input via PS/2 controller.

use x86_64::instructions::port::Port;
use spin::Mutex;

/// PS/2 data port
const DATA_PORT: u16 = 0x60;
/// PS/2 command/status port
const STATUS_PORT: u16 = 0x64;

/// Keyboard buffer size
const BUFFER_SIZE: usize = 256;

/// Scancode set 1 mapping (simplified)
static SCANCODE_MAP: &[char] = &[
    '?', '?', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\b',
    '\t', 'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', '\n',
    '?', 'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', ';', '\'', '`',
    '?', '\\', 'z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/', '?',
    '*', '?', ' ',
];

/// Keyboard state
struct KeyboardState {
    buffer: [u8; BUFFER_SIZE],
    read_pos: usize,
    write_pos: usize,
    shift_pressed: bool,
    caps_lock: bool,
}

impl KeyboardState {
    const fn new() -> Self {
        KeyboardState {
            buffer: [0; BUFFER_SIZE],
            read_pos: 0,
            write_pos: 0,
            shift_pressed: false,
            caps_lock: false,
        }
    }
    
    fn push_scancode(&mut self, scancode: u8) {
        let next_write = (self.write_pos + 1) % BUFFER_SIZE;
        if next_write != self.read_pos {
            self.buffer[self.write_pos] = scancode;
            self.write_pos = next_write;
        }
    }
    
    fn pop_char(&mut self) -> Option<char> {
        if self.read_pos == self.write_pos {
            return None;
        }
        
        let scancode = self.buffer[self.read_pos];
        self.read_pos = (self.read_pos + 1) % BUFFER_SIZE;
        
        // Handle special keys
        match scancode {
            0x2A | 0x36 => { // Shift pressed
                self.shift_pressed = true;
                return None;
            }
            0xAA | 0xB6 => { // Shift released
                self.shift_pressed = false;
                return None;
            }
            0x3A => { // Caps Lock
                self.caps_lock = !self.caps_lock;
                return None;
            }
            _ => {}
        }
        
        // Check for release event (bit 7 set)
        if scancode & 0x80 != 0 {
            return None;
        }
        
        // Convert scancode to character
        if (scancode as usize) < SCANCODE_MAP.len() {
            let mut ch = SCANCODE_MAP[scancode as usize];
            
            // Apply modifiers
            if self.shift_pressed {
                ch = match ch {
                    'a'..='z' => (ch as u8 - b'a' + b'A') as char,
                    '1' => '!', '2' => '@', '3' => '#', '4' => '$',
                    '5' => '%', '6' => '^', '7' => '&', '8' => '*',
                    '9' => '(', '0' => ')', '-' => '_', '=' => '+',
                    '[' => '{', ']' => '}', ';' => ':', '\'' => '"',
                    ',' => '<', '.' => '>', '/' => '?', '\\' => '|',
                    '`' => '~',
                    _ => ch,
                };
            } else if self.caps_lock && ch.is_alphabetic() {
                if ch.is_lowercase() {
                    ch = (ch as u8 - b'a' + b'A') as char;
                } else {
                    ch = (ch as u8 - b'A' + b'a') as char;
                }
            }
            
            Some(ch)
        } else {
            None
        }
    }
}

static KEYBOARD_STATE: Mutex<KeyboardState> = Mutex::new(KeyboardState::new());

/// Initialize keyboard driver
pub fn init() {
    println!("Initializing keyboard driver...");
    unsafe {
        // Enable keyboard interrupts in PS/2 controller
        let mut command_port = Port::new(STATUS_PORT);
        command_port.write(0xAE); // Enable keyboard
        
        // Set keyboard to use scancode set 2
        let mut data_port = Port::new(DATA_PORT);
        data_port.write(0xF0); // Set scancode set command
        data_port.write(0x02); // Scancode set 2
    }
}

/// Handle keyboard interrupt
pub fn handle_scancode() {
    unsafe {
        let mut data_port = Port::new(DATA_PORT);
        let scancode = data_port.read();
        KEYBOARD_STATE.lock().push_scancode(scancode);
    }
}

/// Read a character from keyboard (blocking)
pub fn read_char() -> char {
    loop {
        if let Some(ch) = KEYBOARD_STATE.lock().pop_char() {
            return ch;
        }
        x86_64::instructions::hlt();
    }
}

/// Check if keyboard buffer has data
pub fn has_data() -> bool {
    let state = KEYBOARD_STATE.lock();
    state.read_pos != state.write_pos
}
