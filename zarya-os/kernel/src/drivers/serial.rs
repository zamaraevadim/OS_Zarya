//! Serial Port Driver (COM1)
//! 
//! Provides basic serial I/O for debugging output.

use x86_64::instructions::port::Port;
use spin::Mutex;
use core::fmt::{self, Write};

/// Base port for COM1
const SERIAL_PORT: u16 = 0x3F8;

/// Serial port writer for println! macro
static SERIAL_WRITER: Mutex<SerialPort> = Mutex::new(SerialPort {
    port: Port::new(SERIAL_PORT),
});

/// Serial port interface
pub struct SerialPort {
    port: Port<u8>,
}

impl SerialPort {
    /// Initialize serial port with standard settings
    pub fn init(&mut self) {
        unsafe {
            // Disable interrupts
            self.port.write(0x00);
            
            // Enable DLAB (set baud rate divisor)
            let mut line_control = Port::new(SERIAL_PORT + 3);
            line_control.write(0x80);
            
            // Set divisor to 3 (38400 baud)
            let mut divisor_latch_low = Port::new(SERIAL_PORT);
            let mut divisor_latch_high = Port::new(SERIAL_PORT + 1);
            divisor_latch_low.write(0x03);
            divisor_latch_high.write(0x00);
            
            // 8 bits, no parity, one stop bit
            line_control.write(0x03);
            
            // Enable FIFO
            let mut fifo_control = Port::new(SERIAL_PORT + 2);
            fifo_control.write(0x07);
            
            // Ready for use
        }
    }
    
    /// Write a byte to serial port
    fn write_byte(&mut self, byte: u8) {
        unsafe {
            // Wait until transmit buffer is empty
            let mut line_status = Port::new(SERIAL_PORT + 5);
            while line_status.read() & 0x20 == 0 {}
            
            // Send byte
            self.port.write(byte);
        }
    }
}

impl Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
        Ok(())
    }
}

/// Initialize serial console
pub fn init() {
    SERIAL_WRITER.lock().init();
}

/// Print formatted string to serial port
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    SERIAL_WRITER.lock().write_fmt(args).unwrap();
}

/// Macro for serial output
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::drivers::serial::_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
