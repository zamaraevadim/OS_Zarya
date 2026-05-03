//! Mouse Driver
//! 
//! Handles mouse input via PS/2.

use x86_64::instructions::port::Port;
use spin::Mutex;

/// Mouse state
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseState {
    pub x: i32,
    pub y: i32,
    pub left_button: bool,
    pub right_button: bool,
    pub middle_button: bool,
}

static MOUSE_STATE: Mutex<MouseState> = Mutex::new(MouseState::default());

/// Initialize mouse driver
pub fn init() {
    println!("Initializing mouse driver...");
    unsafe {
        // Enable mouse in PS/2 controller
        let mut command_port = Port::new(0x64);
        command_port.write(0xA8); // Enable auxiliary device
        
        // Set mouse sample rate
        let mut data_port = Port::new(0x60);
        data_port.write(0xF6); // Set defaults
        data_port.write(0xF4); // Enable data reporting
    }
}

/// Handle mouse data packet
pub fn handle_packet(byte: u8) {
    static mut PACKET: [u8; 3] = [0; 3];
    static mut BYTE_INDEX: usize = 0;
    
    unsafe {
        match BYTE_INDEX {
            0 => {
                if byte & 0x08 != 0 {
                    PACKET[0] = byte;
                    BYTE_INDEX = 1;
                }
            }
            1 => {
                PACKET[1] = byte;
                BYTE_INDEX = 2;
            }
            2 => {
                PACKET[2] = byte;
                BYTE_INDEX = 0;
                
                // Parse packet
                let mut state = MOUSE_STATE.lock();
                
                // Button states
                state.left_button = PACKET[0] & 0x01 != 0;
                state.right_button = PACKET[0] & 0x02 != 0;
                state.middle_button = PACKET[0] & 0x04 != 0;
                
                // Movement deltas (signed)
                let dx = PACKET[1] as i8 as i32;
                let dy = -(PACKET[2] as i8 as i32); // Invert Y for screen coordinates
                
                state.x += dx;
                state.y += dy;
                
                // Clamp to screen bounds (assuming 1920x1080)
                state.x = state.x.max(0).min(1919);
                state.y = state.y.max(0).min(1079);
            }
            _ => BYTE_INDEX = 0,
        }
    }
}

/// Get current mouse state
pub fn get_state() -> MouseState {
    *MOUSE_STATE.lock()
}
