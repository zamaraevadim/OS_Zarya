#![no_std]
#![no_main]

//! Zarya OS Bootloader
//! 
//! UEFI application that initializes the system and transfers control to the kernel.
//! Supports both UEFI and Legacy BIOS (Multiboot2) boot methods.

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/// Entry point for UEFI boot
#[no_mangle]
pub extern "C" fn efi_main(image_handle: *mut u8, system_table: *mut u8) -> ! {
    // Initialize serial port for debugging
    unsafe {
        init_serial();
    }

    print!("Zarya OS Bootloader\n");
    print!("===================\n");
    
    // Parse UEFI system table
    // Set up memory map
    // Load kernel ELF from EFI partition
    // Switch to long mode
    // Jump to kernel entry point
    
    print!("Booting kernel...\n");
    
    // Transfer control to kernel at 0x100000
    let kernel_entry = 0x100000 as *const () -> !;
    unsafe {
        kernel_entry();
    }
}

/// Initialize serial port (COM1) for debug output
unsafe fn init_serial() {
    // Configure serial port 0x3F8
    outb(0x3F9, 0x00);  // Disable all interrupts
    outb(0x3FB, 0x80);  // Enable DLAB (set baud rate divisor)
    outb(0x3F8, 0x03);  // Set divisor to 3 (lo byte) - 38400 baud
    outb(0x3F9, 0x00);  //                  (hi byte)
    outb(0x3FB, 0x03);  // 8 bits, no parity, one stop bit
    outb(0x3FC, 0x07);  // Enable FIFO, clear them, with 14-byte threshold
    outb(0x3F9, 0x00);  // All interrupts off
}

/// Output a byte to I/O port
unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!(
        "outb %al, %dx",
        in("dx") port,
        in("al") value,
    );
}

/// Print string to serial port
fn print(s: &str) {
    for byte in s.bytes() {
        unsafe {
            while (inb(0x3FD) & 0x20) == 0 {}  // Wait for transmit buffer empty
            outb(0x3F8, byte);
        }
    }
}

/// Input a byte from I/O port
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!(
        "inb %dx, %al",
        in("dx") port,
        out("al") value,
    );
    value
}

/// Multiboot2 entry point for legacy BIOS boot
#[repr(C)]
struct MultibootHeader {
    magic: u32,
    architecture: u32,
    header_length: u32,
    checksum: u32,
}

#[link_section = ".multiboot"]
static MULTIBOOT_HEADER: MultibootHeader = MultibootHeader {
    magic: 0xe85250d6,
    architecture: 0,  // i386
    header_length: core::mem::size_of::<MultibootHeader>() as u32,
    checksum: -(0xe85250d6u32.wrapping_add(0).wrapping_add(core::mem::size_of::<MultibootHeader>() as u32)) as u32,
};

/// Kernel entry point for multiboot
#[no_mangle]
pub extern "C" fn multiboot_main(multiboot_info: usize, magic: usize) -> ! {
    unsafe {
        init_serial();
    }
    
    print!("Zarya OS (Multiboot)\n");
    print!("Magic: 0x");
    print_hex(magic);
    print!("\n");
    
    // Similar initialization as UEFI path
    // Jump to main kernel
    
    let kernel_entry = 0x100000 as *const () -> !;
    unsafe {
        kernel_entry();
    }
}

fn print_hex(value: usize) {
    let hex_chars = b"0123456789ABCDEF";
    for shift in (0..16).rev() {
        let nibble = (value >> (shift * 4)) & 0xF;
        unsafe {
            while (inb(0x3FD) & 0x20) == 0 {}
            outb(0x3F8, hex_chars[nibble as usize]);
        }
    }
}
