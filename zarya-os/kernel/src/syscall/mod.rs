//! System Call Subsystem
//! 
//! Implements syscall interface for userland applications.

use x86_64::structures::idt::InterruptStackFrame;

/// Syscall numbers
#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum SyscallNumber {
    Read = 0,
    Write = 1,
    Open = 2,
    Close = 3,
    Stat = 4,
    Fstat = 5,
    Lseek = 8,
    Mmap = 9,
    Mprotect = 10,
    Munmap = 11,
    Brk = 12,
    Exit = 60,
    Fork = 57,
    Execve = 59,
    Wait4 = 61,
    Kill = 62,
    Getpid = 39,
    Getuid = 102,
    Access = 21,
    Chdir = 80,
    Rename = 82,
    Mkdir = 83,
    Rmdir = 84,
    Unlink = 87,
    Readlink = 89,
    Chmod = 90,
    Fchmod = 91,
    Gettimeofday = 96,
    Getdents = 217,
    
    // Zarya-specific syscalls
    CreatePort = 1000,
    SendMessage = 1001,
    ReceiveMessage = 1002,
    AllocateSharedMemory = 1003,
    Yield = 1004,
    SpawnThread = 1005,
}

/// Syscall handler function type
type SyscallHandler = extern "C" fn(
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
) -> u64;

/// Syscall table
static mut SYSCALL_TABLE: [Option<SyscallHandler>; 1024] = [None; 1024];

/// Initialize syscall subsystem
pub fn init() {
    println!("Initializing syscall subsystem...");
    
    unsafe {
        // Register standard POSIX-like syscalls
        register_syscall(SyscallNumber::Read as usize, sys_read);
        register_syscall(SyscallNumber::Write as usize, sys_write);
        register_syscall(SyscallNumber::Open as usize, sys_open);
        register_syscall(SyscallNumber::Close as usize, sys_close);
        register_syscall(SyscallNumber::Exit as usize, sys_exit);
        register_syscall(SyscallNumber::Getpid as usize, sys_getpid);
        
        // Register Zarya-specific syscalls
        register_syscall(SyscallNumber::CreatePort as usize, sys_create_port);
        register_syscall(SyscallNumber::SendMessage as usize, sys_send_message);
        register_syscall(SyscallNumber::Yield as usize, sys_yield);
    }
}

/// Register a syscall handler
unsafe fn register_syscall(num: usize, handler: SyscallHandler) {
    if num < SYSCALL_TABLE.len() {
        SYSCALL_TABLE[num] = Some(handler);
    }
}

/// Main syscall dispatcher (called from assembly stub)
#[no_mangle]
pub extern "C" fn syscall_dispatch(
    num: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
) -> u64 {
    unsafe {
        if num < SYSCALL_TABLE.len() {
            if let Some(handler) = SYSCALL_TABLE[num] {
                return handler(arg1, arg2, arg3, arg4, arg5, arg6);
            }
        }
    }
    
    // Invalid syscall number
    u64::MAX
}

// === Syscall Implementations ===

extern "C" fn sys_read(fd: u64, buf: u64, count: u64, _: u64, _: u64, _: u64) -> u64 {
    // In real implementation, would read from file descriptor
    0
}

extern "C" fn sys_write(fd: u64, buf: u64, count: u64, _: u64, _: u64, _: u64) -> u64 {
    // In real implementation, would write to file descriptor
    count
}

extern "C" fn sys_open(path: u64, flags: u64, mode: u64, _: u64, _: u64, _: u64) -> u64 {
    // In real implementation, would open file
    0
}

extern "C" fn sys_close(fd: u64, _: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    0
}

extern "C" fn sys_exit(status: u64, _: u64, _: u64, _: u64, _: u64, _: u64) -> ! {
    // Terminate current process
    loop {
        x86_64::instructions::hlt();
    }
}

extern "C" fn sys_getpid(_: u64, _: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    crate::task::current_pid().unwrap_or(0)
}

extern "C" fn sys_create_port(_: u64, _: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    let pid = crate::task::current_pid().unwrap_or(0);
    crate::ipc::create_port(pid) as u64
}

extern "C" fn sys_send_message(port_id: u64, msg_ptr: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    // In real implementation, would copy message from user space
    0
}

extern "C" fn sys_yield(_: u64, _: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    crate::task::yield_now();
    0
}
