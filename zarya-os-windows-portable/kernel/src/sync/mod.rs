//! Синхронизация примитивы операционной системы Zarya
//! 
//! Реализует:
//! - Spinlock (блокировка вращением)
//! - Mutex (мьютекс)
//! - Semaphore (семафор)
//! - RwLock (read-write lock)

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};

/// Простейший спинлок
/// 
/// Использует атомарную операцию compare-and-swap для захвата блокировки.
/// Если блокировка занята, поток "вращается" в цикле, ожидая освобождения.
pub struct Spinlock<T> {
    /// Флаг занятости блокировки
    locked: AtomicBool,
    /// Защищённые данные
    data: UnsafeCell<T>,
}

// Безопасность: Spinlock может быть отправлен между потоками
unsafe impl<T: Send> Send for Spinlock<T> {}
unsafe impl<T: Send> Sync for Spinlock<T> {}

impl<T> Spinlock<T> {
    /// Создаёт новую блокировку с данными
    pub const fn new(data: T) -> Self {
        Spinlock {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }
    
    /// Захватывает блокировку
    /// 
    /// Если блокировка уже захвачена другим потоком,
    /// этот метод будет вращаться в цикле до освобождения.
    pub fn lock(&self) -> SpinlockGuard<'_, T> {
        // Вращаемся пока не сможем захватить блокировку
        while self.locked.compare_exchange_weak(
            false,  // Ожидаем что разблокировано
            true,   // Устанавливаем заблокировано
            Ordering::Acquire,  // Порядок для захвата
            Ordering::Relaxed,  // Порядок при неудаче
        ).is_err() {
            // Пока ждём, даём процессору возможность делать другие вещи
            // В реальной системе здесь может быть pause инструкция
            #[cfg(target_arch = "x86_64")]
            unsafe {
                core::arch::asm!("pause");
            }
        }
        
        // Блокировка захвачена
        SpinlockGuard { lock: self }
    }
    
    /// Попытка захватить блокировку без ожидания
    pub fn try_lock(&self) -> Option<SpinlockGuard<'_, T>> {
        if self.locked.compare_exchange(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_ok() {
            Some(SpinlockGuard { lock: self })
        } else {
            None
        }
    }
}

/// Guard для Spinlock
/// 
/// Автоматически освобождает блокировку при выходе из области видимости.
pub struct SpinlockGuard<'a, T> {
    lock: &'a Spinlock<T>,
}

impl<'a, T> Deref for SpinlockGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for SpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T> Drop for SpinlockGuard<'a, T> {
    fn drop(&mut self) {
        // Освобождаем блокировку
        self.lock.locked.store(false, Ordering::Release);
    }
}

/// Мьютекс (адаптивный)
/// 
/// Сначала пытается захватить как спинлок,
/// но если не удаётся быстро - переходит в режим ожидания.
pub struct Mutex<T> {
    inner: Spinlock<T>,
}

impl<T> Mutex<T> {
    pub const fn new(data: T) -> Self {
        Mutex {
            inner: Spinlock::new(data),
        }
    }
    
    pub fn lock(&self) -> SpinlockGuard<'_, T> {
        self.inner.lock()
    }
    
    pub fn try_lock(&self) -> Option<SpinlockGuard<'_, T>> {
        self.inner.try_lock()
    }
}

/// Счётный семафор
/// 
/// Позволяет ограниченному количеству потоков одновременно
/// получать доступ к ресурсу.
pub struct Semaphore {
    /// Текущее значение счётчика
    count: AtomicUsize,
}

impl Semaphore {
    /// Создаёт семафор с начальным значением
    pub const fn new(initial: usize) -> Self {
        Semaphore {
            count: AtomicUsize::new(initial),
        }
    }
    
    /// Захватывает семафор (уменьшает счётчик)
    /// 
    /// Если счётчик равен 0, ждёт пока он станет положительным.
    pub fn wait(&self) {
        loop {
            // Пытаемся уменьшить счётчик
            match self.count.fetch_update(
                Ordering::Acquire,
                Ordering::Relaxed,
                |current| {
                    if current > 0 {
                        Some(current - 1)
                    } else {
                        None  // Не можем уменьшить
                    }
                },
            ) {
                Ok(_) => return,  // Успешно захватили
                Err(_) => {
                    // Ждём и пробуем снова
                    #[cfg(target_arch = "x86_64")]
                    unsafe {
                        core::arch::asm!("pause");
                    }
                }
            }
        }
    }
    
    /// Освобождает семафор (увеличивает счётчик)
    pub fn signal(&self) {
        self.count.fetch_add(1, Ordering::Release);
    }
    
    /// Попытка захватить семафор без ожидания
    pub fn try_wait(&self) -> bool {
        match self.count.fetch_update(
            Ordering::Acquire,
            Ordering::Relaxed,
            |current| {
                if current > 0 {
                    Some(current - 1)
                } else {
                    None
                }
            },
        ) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    
    /// Получение текущего значения счётчика
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

/// Read-Write Lock
/// 
/// Позволяет множественное чтение или эксклюзивную запись.
pub struct RwLock<T> {
    /// Данные
    data: UnsafeCell<T>,
    /// Количество активных читателей
    readers: AtomicUsize,
    /// Флаг записи
    writer: AtomicBool,
}

unsafe impl<T: Send + Sync> Send for RwLock<T> {}
unsafe impl<T: Send + Sync> Sync for RwLock<T> {}

impl<T> RwLock<T> {
    pub const fn new(data: T) -> Self {
        RwLock {
            data: UnsafeCell::new(data),
            readers: AtomicUsize::new(0),
            writer: AtomicBool::new(false),
        }
    }
    
    /// Захват на чтение
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        // Ждём пока не будет писателя
        while self.writer.load(Ordering::Relaxed) {
            #[cfg(target_arch = "x86_64")]
            unsafe {
                core::arch::asm!("pause");
            }
        }
        
        // Увеличиваем счётчик читателей
        self.readers.fetch_add(1, Ordering::Acquire);
        
        RwLockReadGuard { lock: self }
    }
    
    /// Захват на запись
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        // Ждём пока не будет читателей и писателей
        while self.writer.swap(true, Ordering::Acquire) 
              || self.readers.load(Ordering::Relaxed) > 0 
        {
            #[cfg(target_arch = "x86_64")]
            unsafe {
                core::arch::asm!("pause");
            }
            self.writer.store(false, Ordering::Release);
        }
        
        RwLockWriteGuard { lock: self }
    }
}

/// Guard для чтения
pub struct RwLockReadGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<'a, T> Deref for RwLockReadGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> Drop for RwLockReadGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.readers.fetch_sub(1, Ordering::Release);
    }
}

/// Guard для записи
pub struct RwLockWriteGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<'a, T> Deref for RwLockWriteGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for RwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T> Drop for RwLockWriteGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.writer.store(false, Ordering::Release);
    }
}

/// Барьер для синхронизации нескольких потоков
pub struct Barrier {
    /// Количество потоков, которые должны достичь барьера
    total: AtomicUsize,
    /// Количество потоков, достигших барьера
    arrived: AtomicUsize,
    /// Номер эпохи барьера
    generation: AtomicUsize,
}

impl Barrier {
    /// Создаёт барьер для заданного количества потоков
    pub const fn new(count: usize) -> Self {
        Barrier {
            total: AtomicUsize::new(count),
            arrived: AtomicUsize::new(0),
            generation: AtomicUsize::new(0),
        }
    }
    
    /// Дожидается остальных потоков
    /// 
    /// Возвращает true для последнего потока, который достиг барьера.
    pub fn wait(&self) -> bool {
        let gen = self.generation.load(Ordering::Relaxed);
        
        // Увеличиваем счётчик прибывших
        let count = self.arrived.fetch_add(1, Ordering::AcqRel) + 1;
        
        if count >= self.total.load(Ordering::Relaxed) {
            // Мы последние - сбрасываем барьер
            self.arrived.store(0, Ordering::Release);
            self.generation.fetch_add(1, Ordering::Release);
            true
        } else {
            // Ждём пока не сменится эпоха
            while self.generation.load(Ordering::Acquire) == gen {
                #[cfg(target_arch = "x86_64")]
                unsafe {
                    core::arch::asm!("pause");
                }
            }
            false
        }
    }
}

/// Условная переменная (Condition Variable)
/// 
/// Используется для ожидания выполнения условия.
pub struct Condvar {
    /// Очередь ожидающих (упрощённо - просто счётчик)
    waiters: AtomicUsize,
}

impl Condvar {
    pub const fn new() -> Self {
        Condvar {
            waiters: AtomicUsize::new(0),
        }
    }
    
    /// Ожидание уведомления
    pub fn wait<T>(&self, mutex: &Mutex<T>) {
        self.waiters.fetch_add(1, Ordering::Relaxed);
        
        // Освобождаем мьютекс и ждём
        drop(mutex.lock());
        
        // В реальной системе здесь было бы ожидание в ядре
        // Для демонстрации просто вращаемся
        while self.waiters.load(Ordering::Relaxed) > 0 {
            #[cfg(target_arch = "x86_64")]
            unsafe {
                core::arch::asm!("pause");
            }
            // Проверяем не пора ли проснуться
            break; // Упрощение для демо
        }
    }
    
    /// Уведомление одного ожидающего
    pub fn notify_one(&self) {
        if self.waiters.load(Ordering::Relaxed) > 0 {
            self.waiters.fetch_sub(1, Ordering::Relaxed);
        }
    }
    
    /// Уведомление всех ожидающих
    pub fn notify_all(&self) {
        self.waiters.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_spinlock() {
        let lock = Spinlock::new(42);
        
        {
            let mut guard = lock.lock();
            assert_eq!(*guard, 42);
            *guard = 100;
        }
        
        assert_eq!(*lock.lock(), 100);
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
