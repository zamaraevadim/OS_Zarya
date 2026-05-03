//! Диспетчер системных вызовов операционной системы Zarya

use crate::sync::Spinlock;

/// Инициализация таблицы системных вызовов
pub fn init() {
    kernel_log!("[SYSCALL] Syscall table initialized\n");
}

/// Номера системных вызовов
pub mod syscall_numbers {
    // Стандартные POSIX вызовы
    pub const SYS_READ: usize = 0;
    pub const SYS_WRITE: usize = 1;
    pub const SYS_OPEN: usize = 2;
    pub const SYS_CLOSE: usize = 3;
    pub const SYS_STAT: usize = 4;
    pub const SYS_MMAP: usize = 5;
    pub const SYS_MUNMAP: usize = 6;
    pub const SYS_FORK: usize = 7;
    pub const SYS_EXEC: usize = 8;
    pub const SYS_EXIT: usize = 9;
    pub const SYS_WAIT: usize = 10;
    
    // Расширения Zarya (IPC)
    pub const SYS_ZPORT_CREATE: usize = 100;
    pub const SYS_ZPORT_SEND: usize = 101;
    pub const SYS_ZPORT_RECV: usize = 102;
    pub const SYS_ZPORT_DESTROY: usize = 103;
    
    // Расширения Zarya (окна)
    pub const SYS_ZWINDOW_CREATE: usize = 200;
    pub const SYS_ZWINDOW_DRAW: usize = 201;
    pub const SYS_ZWINDOW_DESTROY: usize = 202;
    
    // Расширения Zarya (разделяемая память)
    pub const SYS_SHM_CREATE: usize = 300;
    pub const SYS_SHM_ATTACH: usize = 301;
    pub const SYS_SHM_DETACH: usize = 302;
    pub const SYS_SHM_REMOVE: usize = 303;
}

/// Контекст системного вызова (регистры при входе в syscall)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SyscallContext {
    pub rax: u64,  // Номер вызова / результат
    pub rdi: u64,  // Аргумент 1
    pub rsi: u64,  // Аргумент 2
    pub rdx: u64,  // Аргумент 3
    pub r10: u64,  // Аргумент 4
    pub r8: u64,   // Аргумент 5
    pub r9: u64,   // Аргумент 6
    pub rip: u64,  // Возвратный адрес
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

/// Тип обработчика системного вызова
type SyscallHandler = fn(&SyscallContext) -> u64;

/// Таблица системных вызовов
static SYSCALL_TABLE: Spinlock<[Option<SyscallHandler>; 512]> = 
    Spinlock::new([None; 512]);

/// Регистрация обработчика системного вызова
pub fn register_syscall(number: usize, handler: SyscallHandler) -> Result<(), &'static str> {
    if number >= 512 {
        return Err("Invalid syscall number");
    }
    
    let mut table = SYSCALL_TABLE.lock();
    table[number] = Some(handler);
    
    Ok(())
}

/// Главный диспетчер системных вызовов
/// 
/// Вызывается из assembly stub при выполнении инструкции syscall.
#[no_mangle]
pub extern "C" fn syscall_dispatch(ctx: &mut SyscallContext) {
    let syscall_num = ctx.rax as usize;
    
    let table = SYSCALL_TABLE.lock();
    
    if let Some(Some(handler)) = table.get(syscall_num) {
        // Вызов обработчика
        ctx.rax = handler(ctx);
    } else {
        kernel_log!("[SYSCALL] Unknown syscall number: {}\n", syscall_num);
        ctx.rax = u64::MAX;  // Ошибка (EINVAL)
    }
}

/// Обработчик sys_read
fn sys_read(ctx: &SyscallContext) -> u64 {
    use crate::fs::{read, FileDescriptor};
    
    let fd = FileDescriptor(ctx.rdi as i32);
    let buf_ptr = ctx.rsi as *mut u8;
    let count = ctx.rdx as usize;
    
    if buf_ptr.is_null() || count == 0 {
        return u64::MAX;
    }
    
    let buf = unsafe {
        core::slice::from_raw_parts_mut(buf_ptr, count)
    };
    
    match read(fd, buf) {
        Ok(bytes) => bytes as u64,
        Err(_) => u64::MAX,
    }
}

/// Обработчик sys_write
fn sys_write(ctx: &SyscallContext) -> u64 {
    use crate::fs::{write, FileDescriptor};
    
    let fd = FileDescriptor(ctx.rdi as i32);
    let buf_ptr = ctx.rsi as *const u8;
    let count = ctx.rdx as usize;
    
    if buf_ptr.is_null() || count == 0 {
        return u64::MAX;
    }
    
    let buf = unsafe {
        core::slice::from_raw_parts(buf_ptr, count)
    };
    
    match write(fd, buf) {
        Ok(bytes) => bytes as u64,
        Err(_) => u64::MAX,
    }
}

/// Обработчик sys_exit
fn sys_exit(ctx: &SyscallContext) -> u64 {
    use crate::task::exit;
    
    let code = ctx.rdi as i32;
    exit(code);
}

/// Обработчик sys_fork
fn sys_fork(ctx: &SyscallContext) -> u64 {
    use crate::task::fork;
    
    match fork() {
        Ok(pid) => pid.as_u32() as u64,
        Err(_) => u64::MAX,
    }
}

/// Обработчик создания IPC порта
fn sys_zport_create(ctx: &SyscallContext) -> u64 {
    use crate::ipc::create_port;
    use crate::task::getpid;
    
    let name_ptr = ctx.rdi as *const u8;
    
    if name_ptr.is_null() {
        return u64::MAX;
    }
    
    // Безопасное чтение строки из пользовательской памяти
    let name = unsafe {
        core::ffi::CStr::from_ptr(name_ptr as *const i8)
            .to_str()
            .unwrap_or("")
    };
    
    match create_port(name, getpid().as_u32()) {
        Ok(port_id) => port_id.0 as u64,
        Err(_) => u64::MAX,
    }
}

/// Обработчик отправки IPC сообщения
fn sys_zport_send(ctx: &SyscallContext) -> u64 {
    use crate::ipc::{send_message, Message, MessageId, MessageType};
    
    let port_id = ctx.rdi as u32;
    let data_ptr = ctx.rsi as *const u8;
    let data_len = ctx.rdx as usize;
    
    if data_ptr.is_null() || data_len == 0 {
        return u64::MAX;
    }
    
    let data = unsafe {
        core::slice::from_raw_parts(data_ptr, data_len)
    }.to_vec();
    
    let msg = Message {
        id: MessageId(0),
        sender: 0,  // Будет установлено ядром
        data,
        msg_type: MessageType::Normal,
    };
    
    match send_message(crate::ipc::PortId(port_id), msg) {
        Ok(()) => 0,
        Err(_) => u64::MAX,
    }
}

/// Обработчик получения IPC сообщения
fn sys_zport_recv(ctx: &SyscallContext) -> u64 {
    use crate::ipc::receive_message;
    
    let port_id = ctx.rdi as u32;
    let buf_ptr = ctx.rsi as *mut u8;
    let buf_len = ctx.rdx as usize;
    
    if buf_ptr.is_null() || buf_len == 0 {
        return u64::MAX;
    }
    
    match receive_message(crate::ipc::PortId(port_id)) {
        Ok(msg) => {
            let copy_len = core::cmp::min(msg.data.len(), buf_len);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    msg.data.as_ptr(),
                    buf_ptr,
                    copy_len,
                );
            }
            copy_len as u64
        }
        Err(_) => u64::MAX,
    }
}

/// Инициализация всех обработчиков
fn init_handlers() {
    register_syscall(syscall_numbers::SYS_READ, sys_read).unwrap();
    register_syscall(syscall_numbers::SYS_WRITE, sys_write).unwrap();
    register_syscall(syscall_numbers::SYS_EXIT, sys_exit).unwrap();
    register_syscall(syscall_numbers::SYS_FORK, sys_fork).unwrap();
    register_syscall(syscall_numbers::SYS_ZPORT_CREATE, sys_zport_create).unwrap();
    register_syscall(syscall_numbers::SYS_ZPORT_SEND, sys_zport_send).unwrap();
    register_syscall(syscall_numbers::SYS_ZPORT_RECV, sys_zport_recv).unwrap();
}
