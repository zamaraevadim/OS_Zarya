//! Межпроцессное взаимодействие (IPC) операционной системы Zarya
//! 
//! Реализует L4-стиль IPC с портами и сообщениями.

use crate::sync::Spinlock;
use alloc::vec::Vec;
use alloc::string::String;

/// Инициализация IPC подсистемы
pub fn init() {
    kernel_log!("[IPC] IPC subsystem initialized\n");
}

/// ID порта
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortId(pub u32);

impl PortId {
    pub const INVALID: PortId = PortId(0);
}

/// ID сообщения
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageId(pub u32);

/// Сообщение IPC
#[derive(Debug, Clone)]
pub struct Message {
    pub id: MessageId,
    pub sender: u32,  // PID отправителя
    pub data: Vec<u8>,
    pub msg_type: MessageType,
}

/// Типы сообщений
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    Normal,
    Request,
    Reply,
    Notification,
}

/// Порт для IPC коммуникации
pub struct Port {
    pub id: PortId,
    pub name: String,
    pub owner_pid: u32,
    pub max_messages: usize,
    pub messages: Vec<Message>,
}

impl Port {
    pub fn new(id: PortId, name: &str, owner_pid: u32) -> Self {
        Port {
            id,
            name: name.to_string(),
            owner_pid,
            max_messages: 64,
            messages: Vec::new(),
        }
    }
    
    /// Отправка сообщения в порт
    pub fn send(&mut self, msg: Message) -> Result<(), &'static str> {
        if self.messages.len() >= self.max_messages {
            return Err("Port buffer full");
        }
        
        self.messages.push(msg);
        Ok(())
    }
    
    /// Получение сообщения из порта
    pub fn recv(&mut self) -> Option<Message> {
        if self.messages.is_empty() {
            None
        } else {
            Some(self.messages.remove(0))
        }
    }
}

/// Глобальная таблица портов
static PORT_TABLE: Spinlock<Vec<Port>> = Spinlock::new(Vec::new());

/// Счётчик ID портов
static NEXT_PORT_ID: Spinlock<u32> = Spinlock::new(1);

/// Создание нового порта
pub fn create_port(name: &str, owner_pid: u32) -> Result<PortId, &'static str> {
    let mut next_id = NEXT_PORT_ID.lock();
    let id = PortId(*next_id);
    *next_id += 1;
    drop(next_id);
    
    let port = Port::new(id, name, owner_pid);
    
    let mut table = PORT_TABLE.lock();
    table.push(port);
    
    kernel_log!("[IPC] Created port '{}' with ID {}\n", name, id.0);
    
    Ok(id)
}

/// Удаление порта
pub fn destroy_port(id: PortId) -> Result<(), &'static str> {
    let mut table = PORT_TABLE.lock();
    
    if let Some(pos) = table.iter().position(|p| p.id == id) {
        table.remove(pos);
        kernel_log!("[IPC] Destroyed port with ID {}\n", id.0);
        return Ok(());
    }
    
    Err("Port not found")
}

/// Отправка сообщения
pub fn send_message(port_id: PortId, msg: Message) -> Result<(), &'static str> {
    let mut table = PORT_TABLE.lock();
    
    for port in table.iter_mut() {
        if port.id == port_id {
            return port.send(msg);
        }
    }
    
    Err("Port not found")
}

/// Получение сообщения
pub fn receive_message(port_id: PortId) -> Result<Message, &'static str> {
    let mut table = PORT_TABLE.lock();
    
    for port in table.iter_mut() {
        if port.id == port_id {
            return port.recv().ok_or("No messages available");
        }
    }
    
    Err("Port not found")
}

/// Получение сообщения с таймаутом (упрощённая версия)
pub fn receive_message_timeout(port_id: PortId, _timeout_ms: u32) -> Result<Message, &'static str> {
    // В реальной системе здесь было бы ожидание с таймаутом
    receive_message(port_id)
}

/// Поиск порта по имени
pub fn find_port_by_name(name: &str) -> Option<PortId> {
    let table = PORT_TABLE.lock();
    
    table.iter()
        .find(|p| p.name == name)
        .map(|p| p.id)
}

/// Разделяемая память для IPC
pub struct SharedMemory {
    pub id: ShmId,
    pub size: usize,
    pub owner_pid: u32,
    pub attached_pids: Vec<u32>,
    pub addr: u64,  // Физический адрес
}

/// ID разделяемой памяти
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShmId(pub u32);

static NEXT_SHM_ID: Spinlock<u32> = Spinlock::new(1);
static SHM_REGIONS: Spinlock<Vec<SharedMemory>> = Spinlock::new(Vec::new());

/// Создание региона разделяемой памяти
pub fn create_shm(size: usize, owner_pid: u32) -> Result<ShmId, &'static str> {
    let mut next_id = NEXT_SHM_ID.lock();
    let id = ShmId(*next_id);
    *next_id += 1;
    drop(next_id);
    
    // Выделение физической памяти
    use crate::memory::{alloc_physical_page, PAGE_SIZE};
    
    let pages_needed = (size + PAGE_SIZE - 1) / PAGE_SIZE;
    let mut addr = 0u64;
    
    for i in 0..pages_needed {
        if let Some(page) = alloc_physical_page() {
            if i == 0 {
                addr = page.as_u64();
            }
        } else {
            return Err("Failed to allocate shared memory");
        }
    }
    
    let shm = SharedMemory {
        id,
        size,
        owner_pid,
        attached_pids: vec![owner_pid],
        addr,
    };
    
    SHM_REGIONS.lock().push(shm);
    
    kernel_log!("[IPC] Created shared memory region {} (size: {} bytes)\n", id.0, size);
    
    Ok(id)
}

/// Прикрепление к разделяемой памяти
pub fn attach_shm(id: ShmId, pid: u32) -> Result<u64, &'static str> {
    let mut regions = SHM_REGIONS.lock();
    
    for region in regions.iter_mut() {
        if region.id == id {
            if !region.attached_pids.contains(&pid) {
                region.attached_pids.push(pid);
            }
            return Ok(region.addr);
        }
    }
    
    Err("Shared memory region not found")
}

/// Отсоединение от разделяемой памяти
pub fn detach_shm(id: ShmId, pid: u32) -> Result<(), &'static str> {
    let mut regions = SHM_REGIONS.lock();
    
    for region in regions.iter_mut() {
        if region.id == id {
            if let Some(pos) = region.attached_pids.iter().position(|&p| p == pid) {
                region.attached_pids.remove(pos);
            }
            
            // Если больше нет прикреплённых процессов, удаляем регион
            if region.attached_pids.is_empty() {
                return Ok(()); // В реальной системе освобождение памяти
            }
            
            return Ok(());
        }
    }
    
    Err("Shared memory region not found")
}

/// Удаление региона разделяемой памяти
pub fn remove_shm(id: ShmId) -> Result<(), &'static str> {
    let mut regions = SHM_REGIONS.lock();
    
    if let Some(pos) = regions.iter().position(|r| r.id == id) {
        regions.remove(pos);
        kernel_log!("[IPC] Removed shared memory region {}\n", id.0);
        return Ok(());
    }
    
    Err("Shared memory region not found")
}
