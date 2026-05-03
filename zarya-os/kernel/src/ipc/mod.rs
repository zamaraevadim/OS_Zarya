//! IPC (Inter-Process Communication) Subsystem
//! 
//! Implements:
//! - Message passing (synchronous and asynchronous)
//! - Shared memory regions
//! - Ports and endpoints (L4-style)

use alloc::{vec::Vec, collections::VecDeque};
use spin::Mutex;
use crate::task::ProcessState;

/// Maximum message size in bytes
const MAX_MESSAGE_SIZE: usize = 4096;

/// Message structure for IPC
#[derive(Debug, Clone)]
pub struct Message {
    pub sender_pid: u64,
    pub msg_type: u32,
    pub data: [u8; MAX_MESSAGE_SIZE],
    pub data_len: usize,
}

impl Message {
    pub fn new(sender_pid: u64, msg_type: u32, data: &[u8]) -> Self {
        let mut msg = Message {
            sender_pid,
            msg_type,
            data: [0; MAX_MESSAGE_SIZE],
            data_len: data.len().min(MAX_MESSAGE_SIZE),
        };
        msg.data[..msg.data_len].copy_from_slice(&data[..msg.data_len.min(MAX_MESSAGE_SIZE)]);
        msg
    }
}

/// Port endpoint for message passing
pub struct Port {
    pub id: u64,
    pub owner_pid: u64,
    pub message_queue: VecDeque<Message>,
    pub max_queue_size: usize,
}

impl Port {
    pub fn new(id: u64, owner_pid: u64) -> Self {
        Port {
            id,
            owner_pid,
            message_queue: VecDeque::new(),
            max_queue_size: 16,
        }
    }
    
    pub fn send(&mut self, msg: Message) -> Result<(), &'static str> {
        if self.message_queue.len() >= self.max_queue_size {
            return Err("Queue full");
        }
        self.message_queue.push_back(msg);
        Ok(())
    }
    
    pub fn receive(&mut self) -> Option<Message> {
        self.message_queue.pop_front()
    }
}

/// Global port registry
static PORTS: Mutex<Vec<Port>> = Mutex::new(Vec::new());
static NEXT_PORT_ID: Mutex<u64> = Mutex::new(1);

/// Create a new port
pub fn create_port(owner_pid: u64) -> u64 {
    let mut ports = PORTS.lock();
    let mut next_id = NEXT_PORT_ID.lock();
    
    let port = Port::new(*next_id, owner_pid);
    let id = port.id;
    ports.push(port);
    
    *next_id += 1;
    id
}

/// Send message to port
pub fn send_message(port_id: u64, msg: Message) -> Result<(), &'static str> {
    let mut ports = PORTS.lock();
    
    for port in ports.iter_mut() {
        if port.id == port_id {
            return port.send(msg);
        }
    }
    
    Err("Port not found")
}

/// Receive message from port
pub fn receive_message(port_id: u64) -> Option<Message> {
    let mut ports = PORTS.lock();
    
    for port in ports.iter_mut() {
        if port.id == port_id {
            return port.receive();
        }
    }
    
    None
}

/// Shared memory region
#[derive(Debug)]
pub struct SharedMemory {
    pub id: u64,
    pub addr: u64,
    pub size: usize,
    pub owner_pid: u64,
    pub allowed_pids: Vec<u64>,
}

static SHARED_MEMORIES: Mutex<Vec<SharedMemory>> = Mutex::new(Vec::new());
static NEXT_SHM_ID: Mutex<u64> = Mutex::new(1);

/// Allocate shared memory region
pub fn allocate_shared_memory(owner_pid: u64, size: usize) -> u64 {
    let mut memories = SHARED_MEMORIES.lock();
    let mut next_id = NEXT_SHM_ID.lock();
    
    let shm = SharedMemory {
        id: *next_id,
        addr: 0, // Would be allocated in real implementation
        size,
        owner_pid,
        allowed_pids: vec![owner_pid],
    };
    
    let id = shm.id;
    memories.push(shm);
    *next_id += 1;
    id
}

/// Grant access to shared memory
pub fn grant_shared_memory_access(shm_id: u64, pid: u64) -> Result<(), &'static str> {
    let mut memories = SHARED_MEMORIES.lock();
    
    for shm in memories.iter_mut() {
        if shm.id == shm_id && shm.owner_pid == pid {
            shm.allowed_pids.push(pid);
            return Ok(());
        }
    }
    
    Err("Shared memory not found or no permission")
}

/// Initialize IPC subsystem
pub fn init() {
    println!("Initializing IPC subsystem...");
}
