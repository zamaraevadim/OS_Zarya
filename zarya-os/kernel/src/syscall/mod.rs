//! # Диспетчер системных вызовов операционной системы Zarya
//!
//! Обрабатывает системные вызовы от пользовательских процессов.
//! Реализует POSIX-совместимый слой + уникальные вызовы Zarya.

#![no_std]

use x86_64::structures::idt::InterruptStackFrame;

/// Таблица системных вызовов
pub struct SyscallTable {
    entries: [Option<SyscallHandler>; 256],
}

type SyscallHandler = fn(u64, u64, u64) -> u64;

impl SyscallTable {
    pub const fn new() -> Self {
        Self {
            entries: [None; 256],
        }
    }
    
    pub fn register(&mut self, num: usize, handler: SyscallHandler) {
        if num < 256 {
            self.entries[num] = Some(handler);
        }
    }
    
    pub fn dispatch(&self, num: usize, arg1: u64, arg2: u64, arg3: u64) -> u64 {
        if num < 256 {
            if let Some(handler) = self.entries[num] {
                return handler(arg1, arg2, arg3);
            }
        }
        // Неизвестный системный вызов
        u64::MAX
    }
}

/// Номера системных вызовов
pub mod syscall_numbers {
    // Файловые операции (0-19)
    pub const SYS_OPEN: usize = 0;
    pub const SYS_CLOSE: usize = 1;
    pub const SYS_READ: usize = 2;
    pub const SYS_WRITE: usize = 3;
    pub const SYS_STAT: usize = 4;
    pub const SYS_FSTAT: usize = 5;
    pub const SYS_LSEEK: usize = 6;
    pub const SYS_UNLINK: usize = 7;
    pub const SYS_MKDIR: usize = 8;
    pub const SYS_RMDIR: usize = 9;
    pub const SYS_GETDENTS: usize = 10;
    pub const SYS_ACCESS: usize = 11;
    pub const SYS_CHMOD: usize = 12;
    pub const SYS_RENAME: usize = 13;
    pub const SYS_LINK: usize = 14;
    pub const SYS_SYMLINK: usize = 15;
    pub const SYS_READLINK: usize = 16;
    pub const SYS_TRUNCATE: usize = 17;
    pub const SYS_FTRUNCATE: usize = 18;
    pub const SYS_FCNTL: usize = 19;
    
    // Процессы (20-39)
    pub const SYS_EXIT: usize = 20;
    pub const SYS_FORK: usize = 21;
    pub const SYS_EXECVE: usize = 22;
    pub const SYS_WAITPID: usize = 23;
    pub const SYS_GETPID: usize = 24;
    pub const SYS_GETPPID: usize = 25;
    pub const SYS_GETUID: usize = 26;
    pub const SYS_SETUID: usize = 27;
    pub const SYS_GETGID: usize = 28;
    pub const SYS_SETGID: usize = 29;
    pub const SYS_GETEUID: usize = 30;
    pub const SYS_SETEUID: usize = 31;
    pub const SYS_GETEGID: usize = 32;
    pub const SYS_SETEGID: usize = 33;
    pub const SYS_SETPGID: usize = 34;
    pub const SYS_GETPGID: usize = 35;
    pub const SYS_SETSID: usize = 36;
    pub const SYS_GETSID: usize = 37;
    pub const SYS_KILL: usize = 38;
    pub const SYS_SPAWN: usize = 39;
    
    // Память (40-49)
    pub const SYS_BRK: usize = 40;
    pub const SYS_MMAP: usize = 41;
    pub const SYS_MUNMAP: usize = 42;
    pub const SYS_MPROTECT: usize = 43;
    pub const SYS_MADVISE: usize = 44;
    pub const SYS_MLOCK: usize = 45;
    pub const SYS_MUNLOCK: usize = 46;
    pub const SYS_MLOCKALL: usize = 47;
    pub const SYS_MUNLOCKALL: usize = 48;
    pub const SYS_MINCORE: usize = 49;
    
    // IPC (50-59)
    pub const SYS_PIPE: usize = 50;
    pub const SYS_SOCKET: usize = 51;
    pub const SYS_CONNECT: usize = 52;
    pub const SYS_ACCEPT: usize = 53;
    pub const SYS_SEND: usize = 54;
    pub const SYS_RECV: usize = 55;
    pub const SYS_BIND: usize = 56;
    pub const SYS_LISTEN: usize = 57;
    pub const SYS_SHUTDOWN: usize = 58;
    pub const SYS_GETSOCKOPT: usize = 59;
    
    // Время (60-69)
    pub const SYS_TIME: usize = 60;
    pub const SYS_GETTIMEOFDAY: usize = 61;
    pub const SYS_NANOSLEEP: usize = 62;
    pub const SYS_CLOCK_GETTIME: usize = 63;
    pub const SYS_CLOCK_SETTIME: usize = 64;
    pub const SYS_ALARM: usize = 65;
    pub const SYS_SETITIMER: usize = 66;
    pub const SYS_GETITIMER: usize = 67;
    pub const SYS_UTIME: usize = 68;
    pub const SYS_UTIMES: usize = 69;
    
    // Уникальные вызовы Zarya (100-127)
    pub const SYS_ZARYA_VERSION: usize = 100;
    pub const SYS_ZARYA_CREATE_PORT: usize = 101;
    pub const SYS_ZARYA_SEND_MESSAGE: usize = 102;
    pub const SYS_ZARYA_RECV_MESSAGE: usize = 103;
    pub const SYS_ZARYA_CREATE_SHARED_MEM: usize = 104;
    pub const SYS_ZARYA_MAP_SHARED_MEM: usize = 105;
    pub const SYS_ZARYA_UNMAP_SHARED_MEM: usize = 106;
    pub const SYS_ZARYA_REQUEST_PERMISSION: usize = 107;
    pub const SYS_ZARYA_GET_PROCESS_INFO: usize = 108;
    pub const SYS_ZARYA_SET_THREAD_PRIORITY: usize = 109;
    pub const SYS_ZARYA_CREATE_WINDOW: usize = 110;
    pub const SYS_ZARYA_DESTROY_WINDOW: usize = 111;
    pub const SYS_ZARYA_SHOW_WINDOW: usize = 112;
    pub const SYS_ZARYA_HIDE_WINDOW: usize = 113;
    pub const SYS_ZARYA_MOVE_WINDOW: usize = 114;
    pub const SYS_ZARYA_RESIZE_WINDOW: usize = 115;
}

/// Обработчик системного вызова (вызывается из ассемблерной обертки)
#[no_mangle]
pub extern "C" fn syscall_handler(
    syscall_num: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    _stack_frame: &InterruptStackFrame,
) -> u64 {
    // В полной версии здесь был бы диспетчер с таблицей вызовов
    match syscall_num as usize {
        syscall_numbers::SYS_EXIT => sys_exit(arg1 as i32),
        syscall_numbers::SYS_GETPID => sys_getpid(),
        syscall_numbers::SYS_WRITE => sys_write(arg1, arg2, arg3),
        syscall_numbers::SYS_READ => sys_read(arg1, arg2, arg3),
        syscall_numbers::SYS_ZARYA_VERSION => sys_zarya_version(),
        _ => u64::MAX, // Неизвестный вызов
    }
}

/// Системный вызов: exit
fn sys_exit(status: i32) -> u64 {
    println!("[SYSCALL] exit({})", status);
    // В реальной реализации завершение процесса
    loop {
        x86_64::instructions::hlt();
    }
}

/// Системный вызов: getpid
fn sys_getpid() -> u64 {
    // В реальной версии получение PID текущего процесса
    1
}

/// Системный вызов: write
fn sys_write(fd: u64, buf: u64, count: u64) -> u64 {
    // В реальной версии запись в файловый дескриптор
    println!("[SYSCALL] write(fd={}, buf={:#x}, count={})", fd, buf, count);
    count
}

/// Системный вызов: read
fn sys_read(fd: u64, buf: u64, count: u64) -> u64 {
    // В реальной версии чтение из файлового дескриптора
    println!("[SYSCALL] read(fd={}, buf={:#x}, count={})", fd, buf, count);
    0
}

/// Системный вызов: zarya_version
fn sys_zarya_version() -> u64 {
    // Возвращает версию ядра в формате major << 16 | minor
    (0 << 16) | 1
}

/// Инициализация таблицы системных вызовов
pub fn init_syscalls() -> SyscallTable {
    let mut table = SyscallTable::new();
    
    // Регистрация обработчиков
    table.register(syscall_numbers::SYS_EXIT, |a, _, _| sys_exit(a as i32) as u64);
    table.register(syscall_numbers::SYS_GETPID, |_, _, _| sys_getpid());
    table.register(syscall_numbers::SYS_WRITE, sys_write);
    table.register(syscall_numbers::SYS_READ, sys_read);
    table.register(syscall_numbers::SYS_ZARYA_VERSION, |_, _, _| sys_zarya_version());
    
    table
}
