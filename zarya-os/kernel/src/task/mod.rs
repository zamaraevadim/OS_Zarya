//! Task Scheduler Subsystem
//! 
//! Implements:
//! - Process and thread management
//! - Preemptive multitasking with priority scheduling
//! - Context switching

use alloc::{vec::Vec, collections::VecDeque};
use spin::Mutex;
use x86_64::{
    structures::paging::{PageTable, PhysFrame, Size4KiB},
    PhysAddr, VirtAddr,
};

/// Maximum number of processes
const MAX_PROCESSES: usize = 256;

/// Global scheduler instance
static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

/// Process states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Ready,
    Blocked,
    Terminated,
}

/// Priority levels for scheduling
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Idle = 0,
    Normal = 1,
    High = 2,
    RealTime = 3,
}

/// Thread control block
#[derive(Debug)]
pub struct Thread {
    pub id: u64,
    pub stack_pointer: VirtAddr,
    pub base_pointer: VirtAddr,
    pub instruction_pointer: VirtAddr,
    pub state: ProcessState,
    pub priority: Priority,
    pub cpu_state: CpuState,
}

/// CPU register state for context switching
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CpuState {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

/// Process control block
#[derive(Debug)]
pub struct Process {
    pub id: u64,
    pub name: &'static str,
    pub state: ProcessState,
    pub priority: Priority,
    pub threads: Vec<Thread>,
    pub page_table: Option<*mut PageTable>,
    pub memory_regions: Vec<(VirtAddr, VirtAddr)>,
}

impl Process {
    pub fn new(id: u64, name: &'static str) -> Self {
        Process {
            id,
            name,
            state: ProcessState::Ready,
            priority: Priority::Normal,
            threads: Vec::new(),
            page_table: None,
            memory_regions: Vec::new(),
        }
    }
}

/// Scheduler implementation
struct Scheduler {
    processes: [Option<Process>; MAX_PROCESSES],
    ready_queue: VecDeque<u64>,
    current_process: Option<u64>,
    next_pid: u64,
    tick_count: u64,
}

impl Scheduler {
    const fn new() -> Self {
        // Create array of None values
        let processes = [const { None }; MAX_PROCESSES];
        Scheduler {
            processes,
            ready_queue: VecDeque::new(),
            current_process: None,
            next_pid: 1,
            tick_count: 0,
        }
    }
    
    /// Add a new process to the scheduler
    pub fn add_process(&mut self, process: Process) -> u64 {
        let pid = self.next_pid;
        self.next_pid += 1;
        
        if pid as usize < MAX_PROCESSES {
            self.processes[pid as usize] = Some(process);
            self.ready_queue.push_back(pid);
        }
        
        pid
    }
    
    /// Get reference to process by ID
    pub fn get_process(&self, pid: u64) -> Option<&Process> {
        if pid as usize < MAX_PROCESSES {
            self.processes[pid as usize].as_ref()
        } else {
            None
        }
    }
    
    /// Get mutable reference to process by ID
    pub fn get_process_mut(&mut self, pid: u64) -> Option<&mut Process> {
        if pid as usize < MAX_PROCESSES {
            self.processes[pid as usize].as_mut()
        } else {
            None
        }
    }
    
    /// Select next process to run (round-robin with priority)
    pub fn schedule(&mut self) -> Option<u64> {
        // Simple round-robin for now
        if let Some(next_pid) = self.ready_queue.pop_front() {
            // Mark current as ready
            if let Some(current) = self.current_process {
                if let Some(proc) = self.get_process_mut(current) {
                    proc.state = ProcessState::Ready;
                    self.ready_queue.push_back(current);
                }
            }
            
            // Mark next as running
            if let Some(proc) = self.get_process_mut(next_pid) {
                proc.state = ProcessState::Running;
            }
            
            self.current_process = Some(next_pid);
            Some(next_pid)
        } else {
            self.current_process
        }
    }
    
    /// Increment system tick counter
    pub fn tick(&mut self) {
        self.tick_count += 1;
    }
    
    /// Get current tick count
    pub fn get_tick_count(&self) -> u64 {
        self.tick_count
    }
}

/// Initialize the task scheduler
pub fn init() {
    println!("Initializing task scheduler...");
}

/// Increment system tick
pub fn tick() {
    let mut scheduler = SCHEDULER.lock();
    scheduler.tick();
}

/// Yield CPU to next process
pub fn yield_now() {
    let mut scheduler = SCHEDULER.lock();
    scheduler.schedule();
    // Context switch would happen here via assembly
}

/// Spawn the initial user process
pub fn spawn_init_process() -> u64 {
    let mut scheduler = SCHEDULER.lock();
    let process = Process::new(1, "init");
    scheduler.add_process(process)
}

/// Create a new kernel thread
pub fn spawn_kernel_thread(entry: extern "C" fn() -> !) -> u64 {
    let mut scheduler = SCHEDULER.lock();
    let pid = scheduler.next_pid;
    
    let mut process = Process::new(pid, "kernel_thread");
    process.priority = Priority::High;
    
    // Set up initial thread context
    let thread = Thread {
        id: 0,
        stack_pointer: VirtAddr::new(0), // Would be allocated stack
        base_pointer: VirtAddr::new(0),
        instruction_pointer: VirtAddr::new(entry as u64),
        state: ProcessState::Ready,
        priority: Priority::High,
        cpu_state: CpuState::default(),
    };
    
    process.threads.push(thread);
    scheduler.add_process(process)
}

/// Block current process until event
pub fn block_current() {
    let mut scheduler = SCHEDULER.lock();
    if let Some(pid) = scheduler.current_process {
        if let Some(proc) = scheduler.get_process_mut(pid) {
            proc.state = ProcessState::Blocked;
        }
    }
    scheduler.schedule();
}

/// Wake up a blocked process
pub fn wake_up(pid: u64) {
    let mut scheduler = SCHEDULER.lock();
    if let Some(proc) = scheduler.get_process_mut(pid) {
        if proc.state == ProcessState::Blocked {
            proc.state = ProcessState::Ready;
            scheduler.ready_queue.push_back(pid);
        }
    }
}

/// Get current process ID
pub fn current_pid() -> Option<u64> {
    let scheduler = SCHEDULER.lock();
    scheduler.current_process
}
