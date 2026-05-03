#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(naked_functions)]

//! Zarya OS Kernel
//! 
//! Hybrid kernel written in Rust with microkernel architecture.
//! Provides process scheduling, memory management, IPC, and device drivers.

extern crate alloc;

use core::panic::PanicInfo;

mod memory;
mod task;
mod ipc;
mod drivers;
mod fs;
mod syscall;
mod net;
mod sync;

use bootloader::{BootInfo, FrameRange};
use x86_64::{
    structures::paging::{PageTable, FrameAllocator},
    VirtAddr, PhysAddr,
};

/// Kernel entry point called by bootloader
#[no_mangle]
pub extern "C" fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // Initialize serial console for debugging
    drivers::serial::init();
    
    println!("Zarya OS Kernel");
    println!("===============");
    println!("Boot info: {:?}", boot_info);
    
    // Initialize physical memory allocator
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    memory::init(boot_info.memory_map.clone(), phys_mem_offset);
    
    // Initialize interrupt descriptor table
    init_idt();
    
    // Initialize scheduler
    task::init();
    
    // Initialize filesystem
    fs::init();
    
    // Initialize network stack
    net::init();
    
    // Start first user process (init)
    task::spawn_init_process();
    
    // Enable interrupts and start scheduler
    unsafe {
        x86_64::instructions::interrupts::enable();
    }
    
    // Idle loop - should never reach here
    loop {
        x86_64::instructions::hlt();
    }
}

/// Panic handler - print error and halt
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("\n\n!!! KERNEL PANIC !!!\n{}", info);
    loop {
        x86_64::instructions::hlt();
    }
}

/// Initialize Interrupt Descriptor Table
fn init_idt() {
    use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
    
    lazy_static! {
        static ref IDT: InterruptDescriptorTable = {
            let mut idt = InterruptDescriptorTable::new();
            
            // Register interrupt handlers
            idt.breakpoint.set_handler_fn(breakpoint_handler);
            idt.double_fault.set_handler_fn(double_fault_handler);
            idt.page_fault.set_handler_fn(page_fault_handler);
            
            // Hardware IRQs
            idt.timer.set_handler_fn(timer_interrupt_handler);
            idt.keyboard.set_handler_fn(keyboard_interrupt_handler);
            
            idt
        };
    }
    
    IDT.load();
}

extern "x86_64-abort" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("\nBREAKPOINT: {:?}", stack_frame);
}

extern "x86_64-abort" fn double_fault_handler(
    stack_frame: InterruptStackFrame, 
    _error_code: u64
) -> ! {
    panic!("\nDOUBLE FAULT:\n{:?}", stack_frame);
}

extern "x86_64-abort" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: x86_64::structures::idt::PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;
    
    println!("\nPAGE FAULT at address: {:?}", Cr2::read());
    println!("Error code: {:?}", error_code);
    println!("{:?}", stack_frame);
    panic!("Page fault handled by terminating process");
}

extern "x86_64-abort" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Increment system tick
    task::tick();
    
    // Schedule next task
    task::yield_now();
}

extern "x86_64-abort" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Read scancode from keyboard controller
    drivers::keyboard::handle_scancode();
}
