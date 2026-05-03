//! Synchronization Primitives Subsystem
//! 
//! Implements:
//! - Spinlocks
//! - Mutexes
//! - Semaphores
//! - Condition variables

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::cell::UnsafeCell;

/// Simple spinlock implementation
pub struct Spinlock {
    locked: AtomicBool,
}

impl Spinlock {
    pub const fn new() -> Self {
        Spinlock {
            locked: AtomicBool::new(false),
        }
    }
    
    pub fn lock(&self) {
        while self.locked.compare_exchange_weak(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_err() {
            // Spin until lock is acquired
            core::hint::spin_loop();
        }
    }
    
    pub fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
    
    pub fn try_lock(&self) -> bool {
        self.locked.compare_exchange(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_ok()
    }
}

unsafe impl Sync for Spinlock {}
unsafe impl Send for Spinlock {}

/// Mutex wrapper using spinlock
pub struct Mutex<T> {
    lock: Spinlock,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for Mutex<T> {}
unsafe impl<T: Send> Send for Mutex<T> {}

impl<T> Mutex<T> {
    pub const fn new(data: T) -> Self {
        Mutex {
            lock: Spinlock::new(),
            data: UnsafeCell::new(data),
        }
    }
    
    pub fn lock(&self) -> MutexGuard<T> {
        self.lock.lock();
        MutexGuard {
            mutex: self,
        }
    }
    
    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        if self.lock.try_lock() {
            Some(MutexGuard { mutex: self })
        } else {
            None
        }
    }
}

/// Mutex guard for RAII-style locking
pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<'a, T> core::ops::Deref for MutexGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T> core::ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.lock.unlock();
    }
}

/// Counting semaphore
pub struct Semaphore {
    count: AtomicUsize,
}

impl Semaphore {
    pub const fn new(initial: usize) -> Self {
        Semaphore {
            count: AtomicUsize::new(initial),
        }
    }
    
    pub fn wait(&self) {
        loop {
            let current = self.count.load(Ordering::Acquire);
            if current > 0 {
                if self.count.compare_exchange_weak(
                    current,
                    current - 1,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                ).is_ok() {
                    return;
                }
            } else {
                core::hint::spin_loop();
            }
        }
    }
    
    pub fn signal(&self) {
        self.count.fetch_add(1, Ordering::Release);
    }
    
    pub fn try_wait(&self) -> bool {
        loop {
            let current = self.count.load(Ordering::Acquire);
            if current == 0 {
                return false;
            }
            if self.count.compare_exchange_weak(
                current,
                current - 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ).is_ok() {
                return true;
            }
        }
    }
}

unsafe impl Sync for Semaphore {}
unsafe impl Send for Semaphore {}

/// Read-Write lock
pub struct RwLock<T> {
    lock: Mutex<RwLockState<T>>,
}

struct RwLockState<T> {
    readers: usize,
    writer: bool,
    data: T,
}

impl<T> RwLock<T> {
    pub const fn new(data: T) -> Self {
        RwLock {
            lock: Mutex::new(RwLockState {
                readers: 0,
                writer: false,
                data,
            }),
        }
    }
    
    pub fn read(&self) -> RwLockReadGuard<T> {
        let mut state = self.lock.lock();
        state.readers += 1;
        RwLockReadGuard { state }
    }
    
    pub fn write(&self) -> RwLockWriteGuard<T> {
        let mut state = self.lock.lock();
        while state.writer || state.readers > 0 {
            drop(state);
            state = self.lock.lock();
        }
        state.writer = true;
        RwLockWriteGuard { state }
    }
}

pub struct RwLockReadGuard<'a, T> {
    state: core::sync::atomic::AtomicBool,
}

pub struct RwLockWriteGuard<'a, T> {
    state: core::sync::atomic::AtomicBool,
}
