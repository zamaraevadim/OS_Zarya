//! # Менеджер памяти операционной системы Zarya
//! 
//! Реализует:
//! - Физический аллокатор страниц (Buddy Allocator)
//! - Виртуальную память с таблицами страниц x86_64
//! - Область виртуальной памяти (VMA - Virtual Memory Area)
//! - Выделение памяти для ядра и пользовательских процессов

#![no_std]

use x86_64::{
    structures::paging::{
        Page, PageTable, PageTableFlags, Size4KiB, FrameAllocator, FrameDeallocator,
        PhysFrame, MappedPageTable,
    },
    PhysAddr, VirtAddr,
    registers::control_regs::{Cr3, Cr3Flags},
};
use bootloader::BootInfo;
use spin::Mutex;
use bitflags::bitflags;
use core::ptr::NonNull;

/// Глобальный аллокатор физических страниц
static PHYS_ALLOCATOR: Mutex<PhysicalAllocator> = Mutex::new(PhysicalAllocator::new());

/// Глобальный менеджер виртуальной памяти
static mut VMM: Option<VirtualMemoryManager> = None;

/// Инициализация подсистемы памяти
pub fn init(
    physical_memory_offset: VirtAddr,
    memory_map: &'static BootInfo,
) -> &'static mut VirtualMemoryManager {
    // Инициализируем физический аллокатор на основе карты памяти
    let allocator = PhysicalAllocator::from_boot_info(memory_map);
    *PHYS_ALLOCATOR.lock() = allocator;
    
    // Создаем менеджер виртуальной памяти
    let vmm = VirtualMemoryManager::new(physical_memory_offset);
    
    unsafe {
        VMM = Some(vmm);
        VMM.as_mut().unwrap()
    }
}

/// Получение глобального менеджера виртуальной памяти
pub fn get_vmm() -> &'static mut VirtualMemoryManager {
    unsafe { VMM.as_mut().expect("VMM not initialized") }
}

/// Флаги доступа к страницам
bitflags! {
    pub struct PageFlags: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER = 1 << 2;
        const WRITE_THROUGH = 1 << 3;
        const NO_CACHE = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const HUGE_PAGE = 1 << 7;
        const GLOBAL = 1 << 8;
    }
}

/// Область виртуальной памяти (VMA)
#[derive(Debug, Clone)]
pub struct VirtualMemoryArea {
    /// Начальный виртуальный адрес
    pub start: VirtAddr,
    /// Конечный виртуальный адрес
    pub end: VirtAddr,
    /// Флаги доступа
    pub flags: PageFlags,
    /// Тип области (код, данные, куча, стек)
    pub area_type: VMAType,
}

/// Типы областей виртуальной памяти
#[derive(Debug, Clone, PartialEq)]
pub enum VMAType {
    KernelCode,
    KernelData,
    Heap,
    Stack,
    UserCode,
    UserData,
    SharedMemory,
    MappedFile,
}

impl VirtualMemoryArea {
    pub fn new(start: VirtAddr, end: VirtAddr, flags: PageFlags, area_type: VMAType) -> Self {
        Self { start, end, flags, area_type }
    }
    
    /// Проверка, принадлежит ли адрес этой области
    pub fn contains(&self, addr: VirtAddr) -> bool {
        addr >= self.start && addr < self.end
    }
    
    /// Размер области в байтах
    pub fn size(&self) -> u64 {
        self.end.as_u64() - self.start.as_u64()
    }
}

/// Физический аллокатор страниц (упрощенный Buddy Allocator)
pub struct PhysicalAllocator {
    /// Битовая карта свободных страниц
    free_pages: Vec<bool>,
    /// Общее количество страниц
    total_pages: usize,
    /// Количество свободных страниц
    free_count: usize,
}

impl PhysicalAllocator {
    /// Создание пустого аллокатора
    pub const fn new() -> Self {
        Self {
            free_pages: Vec::new(),
            total_pages: 0,
            free_count: 0,
        }
    }
    
    /// Инициализация из информации о загрузке
    pub fn from_boot_info(boot_info: &BootInfo) -> Self {
        let mut allocator = Self::new();
        
        // Подсчет доступных страниц из карты памяти
        let mut total_frames = 0u64;
        
        for region in boot_info.memory_map.iter() {
            if region.region_type == bootloader::MemoryRegionType::Usable {
                let start_addr = region.range.start;
                let end_addr = region.range.end;
                let frame_count = (end_addr - start_addr) / Size4KiB::SIZE;
                total_frames += frame_count;
            }
        }
        
        // Ограничиваем разумным максимумом для демонстрации
        let max_pages = core::cmp::min(total_frames as usize, 1024 * 1024); // 1M страниц = 4GB
        
        allocator.total_pages = max_pages;
        allocator.free_count = max_pages;
        allocator.free_pages = vec![true; max_pages];
        
        allocator
    }
    
    /// Выделение одной физической страницы
    pub fn allocate_frame(&mut self) -> Option<PhysFrame> {
        for (i, is_free) in self.free_pages.iter_mut().enumerate() {
            if *is_free {
                *is_free = false;
                self.free_count -= 1;
                
                let phys_addr = PhysAddr::new(i as u64 * Size4KiB::SIZE);
                return Some(PhysFrame::containing_address(phys_addr));
            }
        }
        None
    }
    
    /// Выделение n连续的 физических страниц
    pub fn allocate_frames(&mut self, count: usize) -> Option<PhysFrame> {
        if count == 0 || count > self.free_count {
            return None;
        }
        
        let mut consecutive = 0;
        let mut start_idx = None;
        
        for (i, is_free) in self.free_pages.iter().enumerate() {
            if *is_free {
                if consecutive == 0 {
                    start_idx = Some(i);
                }
                consecutive += 1;
                
                if consecutive == count {
                    break;
                }
            } else {
                consecutive = 0;
                start_idx = None;
            }
        }
        
        if let Some(start) = start_idx {
            for i in start..start + count {
                self.free_pages[i] = false;
            }
            self.free_count -= count;
            
            let phys_addr = PhysAddr::new(start as u64 * Size4KiB::SIZE);
            return Some(PhysFrame::containing_address(phys_addr));
        }
        
        None
    }
    
    /// Освобождение физической страницы
    pub fn deallocate_frame(&mut self, frame: PhysFrame) {
        let index = (frame.start_address().as_u64() / Size4KiB::SIZE) as usize;
        
        if index < self.total_pages && !self.free_pages[index] {
            self.free_pages[index] = true;
            self.free_count += 1;
        }
    }
    
    /// Статистика аллокатора
    pub fn stats(&self) -> (usize, usize, usize) {
        (self.total_pages, self.free_count, self.total_pages - self.free_count)
    }
}

unsafe impl FrameAllocator<Size4KiB> for PhysicalAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.allocate_frame()
    }
}

unsafe impl FrameDeallocator<Size4KiB> for PhysicalAllocator {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame<Size4KiB>) {
        self.deallocate_frame(frame);
    }
}

/// Менеджер виртуальной памяти
pub struct VirtualMemoryManager {
    /// Оффсет для отображения физической памяти
    physical_memory_offset: VirtAddr,
    /// Список областей виртуальной памяти текущего процесса
    vmas: Vec<VirtualMemoryArea>,
    /// Корневая таблица страниц (CR3)
    root_table: NonNull<PageTable>,
}

impl VirtualMemoryManager {
    pub fn new(physical_memory_offset: VirtAddr) -> Self {
        // Получаем текущую таблицу страниц из CR3
        let (root_table, _) = Cr3::read();
        let root_ptr = NonNull::new(root_table.start_address().as_mut_ptr())
            .expect("Invalid page table pointer");
        
        Self {
            physical_memory_offset,
            vmas: Vec::new(),
            root_table: root_ptr,
        }
    }
    
    /// Создание новой таблицы страниц для процесса
    pub fn create_process_page_table(&mut self) -> Result<(), &'static str> {
        // Выделяем новую страницу для корневой таблицы
        let mut allocator = PHYS_ALLOCATOR.lock();
        let frame = allocator.allocate_frame()
            .ok_or("Failed to allocate page table frame")?;
        drop(allocator);
        
        // Очищаем новую таблицу
        let table_ptr = self.physical_to_virtual(frame.start_address()) as *mut PageTable;
        unsafe {
            core::ptr::write_bytes(table_ptr, 0, 1);
        }
        
        self.root_table = NonNull::new(table_ptr).unwrap();
        
        // Устанавливаем новую таблицу в CR3
        unsafe {
            Cr3::write(
                PhysFrame::from_start_address(frame.start_address()).unwrap(),
                Cr3Flags::empty(),
            );
        }
        
        Ok(())
    }
    
    /// Отображение виртуального адреса в физический
    pub fn map(
        &mut self,
        virt_addr: VirtAddr,
        phys_addr: PhysAddr,
        flags: PageFlags,
    ) -> Result<(), &'static str> {
        let page = Page::containing_address(virt_addr);
        let frame = PhysFrame::containing_address(phys_addr);
        
        // Получаем мутабельный доступ к таблицам страниц
        let mut mapper = unsafe {
            MappedPageTable::new(
                x86_64::structures::paging::page_table::Frame::from_pointer(
                    self.root_table.as_ptr()
                ).unwrap(),
                |frame| self.physical_to_virtual(frame.start_address()),
            )
        };
        
        // Преобразуем наши флаги в флаги x86_64
        let x86_flags = self.flags_to_x86(flags);
        
        unsafe {
            mapper
                .map_to(page, frame, x86_flags, &mut PHYS_ALLOCATOR.lock())
                .map_err(|_| "Mapping failed")?
                .flush();
        }
        
        // Добавляем запись в VMA
        self.vmas.push(VirtualMemoryArea::new(
            virt_addr.align_down(Size4KiB::SIZE),
            virt_addr.align_up(Size4KiB::SIZE),
            flags,
            VMAType::KernelData,
        ));
        
        Ok(())
    }
    
    /// Выделение и отображение новых страниц для кучи
    pub fn allocate_heap(&mut self, size: usize) -> Result<VirtAddr, &'static str> {
        let mut allocator = PHYS_ALLOCATOR.lock();
        
        // Находим конец текущей кучи
        let heap_start = if let Some(last_vma) = self.vmas.iter()
            .filter(|v| v.area_type == VMAType::Heap)
            .last()
        {
            last_vma.end
        } else {
            // Начальный адрес кучи (произвольный выбор)
            VirtAddr::new(0x0000_0000_0010_0000)
        };
        
        let page_count = (size + Size4KiB::SIZE as usize - 1) / Size4KiB::SIZE as usize;
        
        // Выделяем физические страницы
        let start_frame = allocator.allocate_frames(page_count)
            .ok_or("Failed to allocate physical frames")?;
        
        drop(allocator);
        
        // Отображаем виртуальные адреса
        for i in 0..page_count {
            let virt_addr = heap_start + i as u64 * Size4KiB::SIZE;
            let phys_addr = start_frame.start_address() + i as u64 * Size4KiB::SIZE;
            
            self.map(virt_addr, phys_addr, PageFlags::PRESENT | PageFlags::WRITABLE)?;
        }
        
        // Обновляем VMA
        let heap_end = heap_start + page_count as u64 * Size4KiB::SIZE;
        self.vmas.push(VirtualMemoryArea::new(
            heap_start,
            heap_end,
            PageFlags::PRESENT | PageFlags::WRITABLE,
            VMAType::Heap,
        ));
        
        Ok(heap_start)
    }
    
    /// Выделение стека для потока
    pub fn allocate_stack(&mut self, size: usize) -> Result<VirtAddr, &'static str> {
        const STACK_SIZE: usize = 8 * 1024; // 8KB по умолчанию
        let actual_size = if size > STACK_SIZE { size } else { STACK_SIZE };
        
        let mut allocator = PHYS_ALLOCATOR.lock();
        let page_count = (actual_size + Size4KiB::SIZE as usize - 1) / Size4KiB::SIZE as usize;
        
        // Выделяем физические страницы
        let start_frame = allocator.allocate_frames(page_count)
            .ok_or("Failed to allocate stack frames")?;
        
        drop(allocator);
        
        // Адрес стека (растет вниз, поэтому выделяем сверху)
        let stack_end = VirtAddr::new(0x0000_0000_ffff_f000); // Высокие адреса
        let stack_start = stack_end - page_count as u64 * Size4KiB::SIZE;
        
        // Отображаем страницы
        for i in 0..page_count {
            let virt_addr = stack_start + i as u64 * Size4KiB::SIZE;
            let phys_addr = start_frame.start_address() + i as u64 * Size4KiB::SIZE;
            
            self.map(virt_addr, phys_addr, PageFlags::PRESENT | PageFlags::WRITABLE | PageFlags::USER)?;
        }
        
        // Записываем VMA
        self.vmas.push(VirtualMemoryArea::new(
            stack_start,
            stack_end,
            PageFlags::PRESENT | PageFlags::WRITABLE | PageFlags::USER,
            VMAType::Stack,
        ));
        
        Ok(stack_end) // Возвращаем вершину стека
    }
    
    /// Отключение отображения страницы
    pub fn unmap(&mut self, virt_addr: VirtAddr) -> Result<(), &'static str> {
        let page = Page::containing_address(virt_addr);
        
        let mut mapper = unsafe {
            MappedPageTable::new(
                x86_64::structures::paging::page_table::Frame::from_pointer(
                    self.root_table.as_ptr()
                ).unwrap(),
                |frame| self.physical_to_virtual(frame.start_address()),
            )
        };
        
        unsafe {
            let (_, frame) = mapper
                .unmap(page)
                .map_err(|_| "Unmapping failed")?;
            
            // Освобождаем физическую страницу
            PHYS_ALLOCATOR.lock().deallocate_frame(frame);
        }
        
        // Удаляем из VMA
        self.vmas.retain(|vma| !vma.contains(virt_addr));
        
        Ok(())
    }
    
    /// Преобразование физического адреса в виртуальный
    fn physical_to_virtual(&self, phys: PhysAddr) -> VirtAddr {
        phys + self.physical_memory_offset
    }
    
    /// Преобразование наших флагов во флаги x86_64
    fn flags_to_x86(&self, flags: PageFlags) -> PageTableFlags {
        let mut x86_flags = PageTableFlags::empty();
        
        if flags.contains(PageFlags::PRESENT) {
            x86_flags |= PageTableFlags::PRESENT;
        }
        if flags.contains(PageFlags::WRITABLE) {
            x86_flags |= PageTableFlags::WRITABLE;
        }
        if flags.contains(PageFlags::USER) {
            x86_flags |= PageTableFlags::USER_ACCESSIBLE;
        }
        if flags.contains(PageFlags::NO_CACHE) {
            x86_flags |= PageTableFlags::NO_CACHE;
        }
        if flags.contains(PageFlags::WRITE_THROUGH) {
            x86_flags |= PageTableFlags::WRITE_THROUGH;
        }
        if flags.contains(PageFlags::GLOBAL) {
            x86_flags |= PageTableFlags::GLOBAL;
        }
        
        x86_flags
    }
    
    /// Поиск области виртуальной памяти по адресу
    pub fn find_vma(&self, addr: VirtAddr) -> Option<&VirtualMemoryArea> {
        self.vmas.iter().find(|vma| vma.contains(addr))
    }
    
    /// Проверка прав доступа к адресу
    pub fn check_access(&self, addr: VirtAddr, write: bool) -> bool {
        if let Some(vma) = self.find_vma(addr) {
            if write {
                vma.flags.contains(PageFlags::WRITABLE)
            } else {
                vma.flags.contains(PageFlags::PRESENT)
            }
        } else {
            false
        }
    }
}
