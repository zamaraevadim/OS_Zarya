//! # Примитивы синхронизации операционной системы Zarya
//!
//! Реализует:
//! - Spinlock (блокировка вращением)
//! - Mutex (мьютекс)
//! - Semaphore (семафор)
//! - RwLock (читатели-писатели)
//! - Barrier (барьер)
//! - Condvar (условная переменная)

#![no_std]

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::cell::UnsafeCell;
use x86_64::instructions::interrupts;
use x86_64::instructions::hlt;

/// Простейшая спин-блокировка
pub struct Spinlock {
    locked: AtomicBool,
}

impl Spinlock {
    /// Создание новой разблокированной спин-блокировки
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }
    
    /// Захват блокировки
    pub fn lock(&self) {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Вращаемся пока не сможем захватить
            // Можно добавить pause инструкцию для оптимизации
            #[cfg(target_arch = "x86_64")]
            core::hint::spin_loop();
        }
    }
    
    /// Попытка захвата блокировки без ожидания
    pub fn try_lock(&self) -> bool {
        self.locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }
    
    /// Освобождение блокировки
    pub fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
    
    /// Проверка, заблокирована ли блокировка
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }
}

unsafe impl Sync for Spinlock {}
unsafe impl Send for Spinlock {}

/// RAII guard для Spinlock
pub struct SpinlockGuard<'a> {
    lock: &'a Spinlock,
}

impl<'a> Drop for SpinlockGuard<'a> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}

impl<'a> SpinlockGuard<'a> {
    /// Получение доступа к защищенным данным
    pub fn data<T>(&self, data: &'a UnsafeCell<T>) -> &'a mut T {
        unsafe { &mut *data.get() }
    }
}

/// Мьютекс с отключением прерываний
pub struct Mutex<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for Mutex<T> {}
unsafe impl<T: Send> Send for Mutex<T> {}

impl<T> Mutex<T> {
    /// Создание нового мьютекса
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }
    
    /// Захват мьютекса
    pub fn lock(&self) -> MutexGuard<T> {
        // Отключаем прерывания для предотвращения deadlock
        let interrupts_were_enabled = interrupts::are_enabled();
        interrupts::disable();
        
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            if interrupts_were_enabled {
                // Если прерывания были включены, даем процессору отдохнуть
                interrupts::enable();
                hlt();
                interrupts::disable();
            } else {
                #[cfg(target_arch = "x86_64")]
                core::hint::spin_loop();
            }
        }
        
        MutexGuard {
            mutex: self,
            interrupts_were_enabled,
        }
    }
    
    /// Попытка захвата мьютекса
    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        let interrupts_were_enabled = interrupts::are_enabled();
        interrupts::disable();
        
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(MutexGuard {
                mutex: self,
                interrupts_were_enabled,
            })
        } else {
            if interrupts_were_enabled {
                interrupts::enable();
            }
            None
        }
    }
}

/// RAII guard для Mutex
pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
    interrupts_were_enabled: bool,
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.locked.store(false, Ordering::Release);
        if self.interrupts_were_enabled {
            interrupts::enable();
        }
    }
}

impl<'a, T> core::ops::Deref for MutexGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T> core::ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}

/// Семафор
pub struct Semaphore {
    count: AtomicUsize,
}

impl Semaphore {
    /// Создание семафора с начальным значением
    pub const fn new(initial: usize) -> Self {
        Self {
            count: AtomicUsize::new(initial),
        }
    }
    
    /// Захват семафора (уменьшение счетчика)
    pub fn wait(&self) {
        loop {
            let current = self.count.load(Ordering::Acquire);
            
            if current == 0 {
                // Ждем пока счетчик не станет положительным
                #[cfg(target_arch = "x86_64")]
                core::hint::spin_loop();
                continue;
            }
            
            if self
                .count
                .compare_exchange_weak(current, current - 1, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                return;
            }
        }
    }
    
    /// Попытка захвата семафора
    pub fn try_wait(&self) -> bool {
        loop {
            let current = self.count.load(Ordering::Acquire);
            
            if current == 0 {
                return false;
            }
            
            if self
                .count
                .compare_exchange_weak(current, current - 1, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
        }
    }
    
    /// Освобождение семафора (увеличение счетчика)
    pub fn signal(&self) {
        self.count.fetch_add(1, Ordering::Release);
    }
    
    /// Получение текущего значения счетчика
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

unsafe impl Sync for Semaphore {}
unsafe impl Send for Semaphore {}

/// Блокировка читатели-писатели
pub struct RwLock<T> {
    readers: AtomicUsize,
    writer: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for RwLock<T> {}
unsafe impl<T: Send> Send for RwLock<T> {}

impl<T> RwLock<T> {
    /// Создание новой блокировки
    pub const fn new(data: T) -> Self {
        Self {
            readers: AtomicUsize::new(0),
            writer: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }
    
    /// Захват для чтения
    pub fn read(&self) -> RwLockReadGuard<T> {
        // Ждем пока писатель не закончит
        while self.writer.load(Ordering::Acquire) {
            #[cfg(target_arch = "x86_64")]
            core::hint::spin_loop();
        }
        
        // Увеличиваем счетчик читателей
        self.readers.fetch_add(1, Ordering::Acquire);
        
        RwLockReadGuard { lock: self }
    }
    
    /// Захват для записи
    pub fn write(&self) -> RwLockWriteGuard<T> {
        // Ждем пока все читатели и писатели не закончат
        while self.writer.swap(true, Ordering::Acquire) || 
              self.readers.load(Ordering::Acquire) != 0 
        {
            #[cfg(target_arch = "x86_64")]
            core::hint::spin_loop();
        }
        
        RwLockWriteGuard { lock: self }
    }
    
    /// Попытка захвата для чтения
    pub fn try_read(&self) -> Option<RwLockReadGuard<T>> {
        if self.writer.load(Ordering::Acquire) {
            return None;
        }
        
        self.readers.fetch_add(1, Ordering::Acquire);
        Some(RwLockReadGuard { lock: self })
    }
    
    /// Попытка захвата для записи
    pub fn try_write(&self) -> Option<RwLockWriteGuard<T>> {
        if self.writer.swap(true, Ordering::Acquire) || 
           self.readers.load(Ordering::Acquire) != 0 
        {
            return None;
        }
        
        Some(RwLockWriteGuard { lock: self })
    }
}

/// Guard для чтения
pub struct RwLockReadGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<'a, T> Drop for RwLockReadGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.readers.fetch_sub(1, Ordering::Release);
    }
}

impl<'a, T> core::ops::Deref for RwLockReadGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

/// Guard для записи
pub struct RwLockWriteGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<'a, T> Drop for RwLockWriteGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.writer.store(false, Ordering::Release);
    }
}

impl<'a, T> core::ops::Deref for RwLockWriteGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> core::ops::DerefMut for RwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

/// Барьер для синхронизации нескольких потоков
pub struct Barrier {
    count: AtomicUsize,
    waiting: AtomicUsize,
    generation: AtomicUsize,
}

impl Barrier {
    /// Создание барьера для указанного количества потоков
    pub const fn new(count: usize) -> Self {
        Self {
            count: AtomicUsize::new(count),
            waiting: AtomicUsize::new(0),
            generation: AtomicUsize::new(0),
        }
    }
    
    /// Ожидание на барьере
    pub fn wait(&self) -> bool {
        let gen = self.generation.load(Ordering::Relaxed);
        
        // Увеличиваем счетчик ожидающих
        let remaining = self.count.load(Ordering::Relaxed) - 
                        self.waiting.fetch_add(1, Ordering::SeqCst) - 1;
        
        if remaining == 0 {
            // Последний поток достиг барьера
            self.waiting.store(0, Ordering::Relaxed);
            self.generation.fetch_add(1, Ordering::SeqCst);
            true // Этот поток был последним
        } else {
            // Ждем пока все потоки не достигнут барьера
            while gen == self.generation.load(Ordering::Relaxed) {
                #[cfg(target_arch = "x86_64")]
                core::hint::spin_loop();
            }
            false
        }
    }
}

unsafe impl Sync for Barrier {}
unsafe impl Send for Barrier {}

/// Условная переменная
pub struct Condvar {
    waiters_count: AtomicUsize,
}

impl Condvar {
    /// Создание новой условной переменной
    pub const fn new() -> Self {
        Self {
            waiters_count: AtomicUsize::new(0),
        }
    }
    
    /// Ожидание сигнала
    pub fn wait<T>(&self, mutex: &Mutex<T>) {
        self.waiters_count.fetch_add(1, Ordering::SeqCst);
        
        // Освобождаем мьютекс и ждем
        drop(mutex);
        
        // Ждем пока не будет сигнала
        while self.waiters_count.load(Ordering::Acquire) > 0 {
            interrupts::enable();
            hlt();
            interrupts::disable();
        }
        
        interrupts::enable();
    }
    
    /// Пробуждение одного ожидающего потока
    pub fn notify_one(&self) {
        if self.waiters_count.load(Ordering::Relaxed) > 0 {
            self.waiters_count.fetch_sub(1, Ordering::Release);
        }
    }
    
    /// Пробуждение всех ожидающих потоков
    pub fn notify_all(&self) {
        self.waiters_count.store(0, Ordering::Release);
    }
    
    /// Проверка, есть ли ожидающие потоки
    pub fn has_waiters(&self) -> bool {
        self.waiters_count.load(Ordering::Relaxed) > 0
    }
}

unsafe impl Sync for Condvar {}
unsafe impl Send for Condvar {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_spinlock() {
        let lock = Spinlock::new();
        assert!(!lock.is_locked());
        
        lock.lock();
        assert!(lock.is_locked());
        
        lock.unlock();
        assert!(!lock.is_locked());
    }
    
    #[test]
    fn test_mutex() {
        let mutex = Mutex::new(42);
        
        {
            let mut guard = mutex.lock();
            assert_eq!(*guard, 42);
            *guard = 100;
        }
        
        assert_eq!(*mutex.lock(), 100);
    }
    
    #[test]
    fn test_semaphore() {
        let sem = Semaphore::new(2);
        
        assert!(sem.try_wait());
        assert!(sem.try_wait());
        assert!(!sem.try_wait());
        
        sem.signal();
        assert!(sem.try_wait());
    }
}
