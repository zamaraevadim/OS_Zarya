//! Менеджер памяти операционной системы Zarya
//! 
//! Реализует:
//! - Физический аллокатор (buddy system)
//! - Виртуальную память с таблицами страниц
//! - Управление областями виртуальной памяти (VMA)

use core::sync::atomic::{AtomicBool, Ordering};
use crate::sync::Spinlock;
use crate::BootInfo;

/// Инициализирован ли менеджер памяти
static mut INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Глобальный физический аллокатор
static PHYSICAL_ALLOCATOR: Spinlock<PhysicalAllocator> = 
    Spinlock::new(PhysicalAllocator::new());

/// Максимальный порядок аллокации (2^MAX_ORDER страниц)
const MAX_ORDER: usize = 10;

/// Размер страницы в байтах (4 KB)
pub const PAGE_SIZE: usize = 4096;

/// Инициализация менеджера памяти
pub fn init(boot_info: &BootInfo) {
    unsafe {
        if INITIALIZED.load(Ordering::Relaxed) {
            kernel_log!("[MEMORY] Already initialized!\n");
            return;
        }
        
        // Получаем карту памяти от загрузчика
        let memory_map = boot_info.memory_map as *const MemoryMapEntry;
        let entries_count = boot_info.memory_map_entries;
        
        kernel_log!("[MEMORY] Parsing {} memory map entries...\n", entries_count);
        
        // Передаём информацию об.available памяти аллокатору
        let mut allocator = PHYSICAL_ALLOCATOR.lock();
        allocator.init_from_memory_map(memory_map, entries_count);
        
        INITIALIZED.store(true, Ordering::Relaxed);
        kernel_log!("[MEMORY] Initialization complete\n");
    }
}

/// Проверка инициализации
fn check_initialized() {
    unsafe {
        assert!(INITIALIZED.load(Ordering::Relaxed), 
                "Memory manager not initialized!");
    }
}

/// Запись карты памяти
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryMapEntry {
    /// Базовый адрес региона
    pub base_addr: u64,
    /// Длина региона в байтах
    pub length: u64,
    /// Тип региона
    pub entry_type: u32,
    /// Расширенные атрибуты (для EFI)
    pub extended_attr: u32,
}

impl MemoryMapEntry {
    /// Тип: доступная память
    pub const USABLE: u32 = 1;
    /// Тип: зарезервировано
    pub const RESERVED: u32 = 2;
    /// Тип: ACPI данные
    pub const ACPI_RECLAIMABLE: u32 = 3;
    /// Тип: ACPI NVS
    pub const ACPI_NVS: u32 = 4;
    /// Тип: область с ошибками
    pub const BAD_MEMORY: u32 = 5;
}

/// Физический аллокатор страниц (Buddy System)
pub struct PhysicalAllocator {
    /// Свободные списки для каждого порядка
    free_lists: [core::option::Option<NonNull<Page>>; MAX_ORDER],
    /// Общее количество свободных страниц
    free_pages: usize,
    /// Общее количество выделенных страниц
    allocated_pages: usize,
    /// Нижняя граница доступной памяти
    min_addr: PhysAddr,
    /// Верхняя граница доступной памяти
    max_addr: PhysAddr,
}

/// Физический адрес
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr(pub u64);

impl PhysAddr {
    pub const fn new(addr: u64) -> Self {
        PhysAddr(addr)
    }
    
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
    
    pub fn align_up(&self, align: usize) -> Self {
        let mask = (align - 1) as u64;
        PhysAddr((self.0 + mask) & !(mask))
    }
}

/// Виртуальный адрес
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtAddr(pub u64);

impl VirtAddr {
    pub const fn new(addr: u64) -> Self {
        VirtAddr(addr)
    }
    
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
    
    pub fn align_up(&self, align: usize) -> Self {
        let mask = (align - 1) as u64;
        VirtAddr((self.0 + mask) & !(mask))
    }
}

/// Страница памяти
#[repr(C)]
pub struct Page {
    /// Указатель на следующую страницу в свободном списке
    next: core::option::Option<NonNull<Page>>,
}

/// Узел свободного списка
use core::ptr::NonNull;

impl PhysicalAllocator {
    /// Создаёт новый пустой аллокатор
    pub const fn new() -> Self {
        PhysicalAllocator {
            free_lists: [None; MAX_ORDER],
            free_pages: 0,
            allocated_pages: 0,
            min_addr: PhysAddr::new(u64::MAX),
            max_addr: PhysAddr::new(0),
        }
    }
    
    /// Инициализация из карты памяти
    pub fn init_from_memory_map(&mut self, map: *const MemoryMapEntry, count: usize) {
        kernel_log!("[MEMORY] Initializing physical allocator...\n");
        
        let map_slice = unsafe {
            core::slice::from_raw_parts(map, count)
        };
        
        let mut total_usable: u64 = 0;
        let mut regions_count = 0;
        
        for entry in map_slice.iter() {
            if entry.entry_type == MemoryMapEntry::USABLE {
                total_usable += entry.length;
                regions_count += 1;
                
                // Обновляем границы
                let start = PhysAddr::new(entry.base_addr).align_up(PAGE_SIZE);
                let end = PhysAddr::new(entry.base_addr + entry.length);
                
                if start.0 < self.min_addr.0 {
                    self.min_addr = start;
                }
                if end.0 > self.max_addr.0 {
                    self.max_addr = end;
                }
                
                // Добавляем страницы в свободный список
                self.add_memory_region(start, end);
            }
        }
        
        kernel_log!("[MEMORY] Found {} usable regions\n", regions_count);
        kernel_log!("[MEMORY] Total usable memory: {} MB\n", total_usable / (1024 * 1024));
        kernel_log!("[MEMORY] Address range: 0x{:x} - 0x{:x}\n", 
                   self.min_addr.0, self.max_addr.0);
    }
    
    /// Добавление региона памяти в аллокатор
    fn add_memory_region(&mut self, start: PhysAddr, end: PhysAddr) {
        let mut addr = start;
        
        while addr.0 + PAGE_SIZE as u64 <= end.0 {
            // Выравниваем по размеру страницы
            let page_addr = addr.align_up(PAGE_SIZE);
            
            if page_addr.0 + PAGE_SIZE as u64 > end.0 {
                break;
            }
            
            // Добавляем страницу в свободный список порядка 0
            let page_ptr = page_addr.0 as *mut Page;
            unsafe {
                (*page_ptr).next = None;
            }
            
            if let Some(page) = NonNull::new(page_ptr) {
                self.free_lists[0] = Some(page);
                self.free_pages += 1;
            }
            
            addr = PhysAddr::new(page_addr.0 + PAGE_SIZE as u64);
        }
    }
    
    /// Выделение одной физической страницы
    pub fn alloc_page(&mut self) -> Option<PhysAddr> {
        self.alloc_pages_order(0)
    }
    
    /// Выделение страниц указанного порядка (2^order страниц)
    pub fn alloc_pages_order(&mut self, order: usize) -> Option<PhysAddr> {
        if order >= MAX_ORDER {
            return None;
        }
        
        // Ищем свободный блок нужного или большего размера
        for current_order in order..MAX_ORDER {
            if let Some(mut block) = self.free_lists[current_order].take() {
                // Нашли блок, возможно нужно разделить
                let mut current_size = current_order;
                
                while current_size > order {
                    current_size -= 1;
                    
                    // Разделяем блок на два
                    let half_size = PAGE_SIZE << current_size;
                    let second_half_addr = block.as_ptr() as u64 + half_size;
                    
                    let second_half = NonNull::new(second_half_addr as *mut Page)?;
                    unsafe {
                        second_half.as_mut().next = self.free_lists[current_size];
                    }
                    self.free_lists[current_size] = Some(second_half);
                    self.free_pages += 1 << current_size;
                }
                
                // Удаляем блок из свободного списка
                let next_block = unsafe { block.as_mut().next };
                self.free_lists[order] = next_block;
                self.free_pages -= 1 << order;
                self.allocated_pages += 1 << order;
                
                return Some(PhysAddr::new(block.as_ptr() as u64));
            }
        }
        
        // Нет свободной памяти
        None
    }
    
    /// Освобождение физической страницы
    pub fn free_page(&mut self, addr: PhysAddr) {
        self.free_pages_order(addr, 0);
    }
    
    /// Освобождение страниц указанного порядка
    pub fn free_pages_order(&mut self, addr: PhysAddr, order: usize) {
        if order >= MAX_ORDER {
            return;
        }
        
        let page_ptr = addr.0 as *mut Page;
        
        if let Some(mut block) = NonNull::new(page_ptr) {
            // Пытаемся объединить с соседними блоками (coalescing)
            let mut current_order = order;
            
            while current_order < MAX_ORDER - 1 {
                // Вычисляем адрес buddy блока
                let buddy_addr = addr.0 ^ (PAGE_SIZE << current_order) as u64;
                let buddy_ptr = buddy_addr as *mut Page;
                
                // Проверяем, является ли buddy свободным блоком того же порядка
                // (упрощённая проверка - в реальной системе нужен более сложный механизм)
                let mut found_buddy = false;
                
                if let Some(mut head) = self.free_lists[current_order] {
                    let mut prev: Option<NonNull<Page>> = None;
                    
                    while let Some(current) = head {
                        if current.as_ptr() == buddy_ptr {
                            // Нашли buddy, удаляем его из списка
                            if let Some(p) = prev {
                                unsafe {
                                    p.as_mut().next = current.as_mut().next;
                                }
                            } else {
                                self.free_lists[current_order] = unsafe { current.as_mut().next };
                            }
                            
                            // Объединяем блоки
                            let merged_addr = core::cmp::min(addr.0, buddy_addr);
                            block = NonNull::new(merged_addr as *mut Page).unwrap();
                            found_buddy = true;
                            self.free_pages -= 1 << current_order;
                            break;
                        }
                        
                        prev = Some(current);
                        head = unsafe { current.as_mut().next };
                    }
                }
                
                if !found_buddy {
                    break;
                }
                
                current_order += 1;
            }
            
            // Добавляем блок в соответствующий свободный список
            unsafe {
                block.as_mut().next = self.free_lists[current_order];
            }
            self.free_lists[current_order] = Some(block);
            self.free_pages += 1 << current_order;
            self.allocated_pages -= 1 << order;
        }
    }
    
    /// Получение статистики использования памяти
    pub fn get_stats(&self) -> MemoryStats {
        MemoryStats {
            free_pages: self.free_pages,
            allocated_pages: self.allocated_pages,
            total_pages: self.free_pages + self.allocated_pages,
            min_addr: self.min_addr,
            max_addr: self.max_addr,
        }
    }
}

/// Статистика использования памяти
#[derive(Debug, Clone, Copy)]
pub struct MemoryStats {
    pub free_pages: usize,
    pub allocated_pages: usize,
    pub total_pages: usize,
    pub min_addr: PhysAddr,
    pub max_addr: PhysAddr,
}

impl MemoryStats {
    pub fn print(&self) {
        kernel_log!("[MEMORY] Stats:\n");
        kernel_log!("  Free pages: {}\n", self.free_pages);
        kernel_log!("  Allocated pages: {}\n", self.allocated_pages);
        kernel_log!("  Total pages: {}\n", self.total_pages);
        kernel_log!("  Memory usage: {}%\n", 
                   (self.allocated_pages * 100) / self.total_pages.max(1));
    }
}

/// Область виртуальной памяти (VMA)
#[derive(Debug, Clone)]
pub struct VMA {
    /// Начальный виртуальный адрес
    pub start: VirtAddr,
    /// Конечный виртуальный адрес
    pub end: VirtAddr,
    /// Флаги доступа
    pub flags: VMAFlags,
    /// Связанный файл (для mmap)
    pub file: Option<VMAFile>,
}

/// Флаги VMA
bitflags::bitflags! {
    pub struct VMAFlags: u32 {
        const READ = 0b0001;
        const WRITE = 0b0010;
        const EXECUTE = 0b0100;
        const SHARED = 0b1000;
        const PRIVATE = 0b10000;
        const ANONYMOUS = 0b100000;
        const GROWS_DOWN = 0b1000000;
        const GROWS_UP = 0b10000000;
    }
}

/// Информация о файле для VMA
#[derive(Debug, Clone)]
pub struct VMAFile {
    /// Дескриптор файла
    pub fd: i32,
    /// Смещение в файле
    pub offset: u64,
}

/// Пространство виртуальной памяти процесса
pub struct AddressSpace {
    /// Корневая таблица страниц (CR3)
    page_table_root: PhysAddr,
    /// Список областей виртуальной памяти
    vmas: alloc::vec::Vec<VMA>,
    /// Базовый адрес для mmap
    mmap_base: VirtAddr,
}

impl AddressSpace {
    /// Создаёт новое пространство адресов
    pub fn new() -> Self {
        // Выделяем страницу для корневой таблицы страниц
        let root_addr = PHYSICAL_ALLOCATOR.lock().alloc_page()
            .expect("Failed to allocate page table root");
        
        // Очищаем таблицу страниц
        unsafe {
            core::ptr::write_bytes(root_addr.0 as *mut u8, 0, PAGE_SIZE);
        }
        
        AddressSpace {
            page_table_root: root_addr,
            vmas: alloc::vec::Vec::new(),
            mmap_base: VirtAddr::new(0x0000_7fff_ffff_f000), // Высокие адреса для mmap
        }
    }
    
    /// Отображение физической памяти в виртуальную
    pub fn map(&mut self, virt: VirtAddr, phys: PhysAddr, flags: VMAFlags) {
        // В реальной системе здесь будет настройка таблиц страниц
        // Для демонстрации просто добавляем VMA
        
        let vma = VMA {
            start: virt.align_up(PAGE_SIZE),
            end: VirtAddr::new(virt.0 + PAGE_SIZE as u64),
            flags,
            file: None,
        };
        
        self.vmas.push(vma);
        
        kernel_log!("[MMAP] Mapped 0x{:x} -> 0x{:x} with flags {:?}\n", 
                   virt.0, phys.0, flags);
    }
    
    /// Системный вызов mmap
    pub fn mmap(&mut self, addr: VirtAddr, length: usize, 
                prot: VMAFlags, flags: u32, fd: i32, offset: u64) -> VirtAddr {
        
        let aligned_length = (length + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        
        // Если адрес не указан, выделяем новый
        let virt_addr = if addr.0 == 0 {
            let result = self.mmap_base;
            self.mmap_base = VirtAddr::new(result.0 - aligned_length as u64);
            result
        } else {
            addr.align_up(PAGE_SIZE)
        };
        
        // Создаём VMA
        let vma = VMA {
            start: virt_addr,
            end: VirtAddr::new(virt_addr.0 + aligned_length as u64),
            flags: prot,
            file: if fd >= 0 {
                Some(VMAFile { fd, offset })
            } else {
                None
            },
        };
        
        self.vmas.push(vma);
        
        kernel_log!("[MMAP] Allocated region at 0x{:x}, size {} bytes\n", 
                   virt_addr.0, aligned_length);
        
        virt_addr
    }
    
    /// Поиск VMA по виртуальному адресу
    pub fn find_vma(&self, addr: VirtAddr) -> Option<&VMA> {
        self.vmas.iter().find(|vma| {
            addr.0 >= vma.start.0 && addr.0 < vma.end.0
        })
    }
    
    /// Получение физического адреса корня таблиц страниц
    pub fn get_page_table_root(&self) -> PhysAddr {
        self.page_table_root
    }
}

/// Глобальная функция выделения страницы
pub fn alloc_physical_page() -> Option<PhysAddr> {
    PHYSICAL_ALLOCATOR.lock().alloc_page()
}

/// Глобальная функция освобождения страницы
pub fn free_physical_page(addr: PhysAddr) {
    PHYSICAL_ALLOCATOR.lock().free_page(addr);
}

/// Тестирование аллокатора
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_allocator_basic() {
        let mut allocator = PhysicalAllocator::new();
        
        // Выделение и освобождение страницы
        let page1 = allocator.alloc_page();
        assert!(page1.is_some());
        
        let page2 = allocator.alloc_page();
        assert!(page2.is_some());
        assert!(page1 != page2);
        
        allocator.free_page(page1.unwrap());
        
        let page3 = allocator.alloc_page();
        assert!(page3.is_some());
    }
}
