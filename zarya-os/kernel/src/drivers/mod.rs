//! Device Drivers Subsystem
//! 
//! Implements drivers for:
//! - Serial port (for debugging)
//! - Keyboard (PS/2 and USB HID)
//! - Framebuffer (VESA/GOP)
//! - Mouse

use x86_64::instructions::{port::Port, interrupts};
use spin::Mutex;

pub mod serial;
pub mod keyboard;
pub mod framebuffer;
pub mod mouse;

/// Initialize all device drivers
pub fn init() {
    serial::init();
    keyboard::init();
    framebuffer::init();
}
