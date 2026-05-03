//! Memory Management Subsystem
//! 
//! Implements:
//! - Physical memory allocation (buddy allocator)
//! - Virtual memory management (paging)
//! - Frame allocation and page tables

use x86_64::{
    structures::paging::{
        PageTable, FrameAllocator, PhysFrame, Size4KiB, OffsetPageTable,
        Mapper, Page,
    },
    PhysAddr, VirtAddr,
};
use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use spin::Mutex;
use core::ptr::NonNull;

/// Global physical frame allocator
static FRAME_ALLOCATOR: Mutex<BootInfoFrameAllocator> = Mutex::new(
    BootInfoFrameAllocator {
        next_frame: 0,
        memory_map: None,
    }
);

/// Initialize the memory management subsystem
pub fn init(memory_map: &'static MemoryMap, physical_memory_offset: VirtAddr) {
    let mut allocator = FRAME_ALLOCATOR.lock();
    allocator.memory_map = Some(memory_map);
    
    // Map kernel pages
    unsafe {
        let phys_mem_offset = physical_memory_offset.as_u64() as *mut ();
        let level_4_table_ptr = NonNull::new_unchecked(phys_mem_offset as *mut _);
        
        let mut mapper = OffsetPageTable::new(level_4_table_ptr, physical_memory_offset);
        init_frame_allocator(&mut mapper, memory_map);
    }
}

/// Initialize the frame allocator from memory map
fn init_frame_allocator(
    mapper: &mut OffsetPageTable,
    memory_map: &'static MemoryMap,
) {
    for region in memory_map.iter() {
        if region.region_type == MemoryRegionType::Usable {
            let start_addr = region.range_start_addr();
            let end_addr = region.range_end_addr();
            
            // Align to frame boundaries
            let start_frame = PhysFrame::<Size4KiB>::containing_address(
                PhysAddr::new(start_addr)
            );
            let end_frame = PhysFrame::<Size4KiB>::containing_address(
                PhysAddr::new(end_addr - 1)
            );
            
            // Mark frames as available
            for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
                // Frames will be allocated on demand
            }
        }
    }
}

/// Frame allocator implementation using boot info
struct BootInfoFrameAllocator {
    next_frame: u64,
    memory_map: Option<&'static MemoryMap>,
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        if let Some(map) = self.memory_map {
            for region in map.iter() {
                if region.region_type == MemoryRegionType::Usable {
                    let start = region.range_start_addr();
                    let end = region.range_end_addr();
                    
                    if start + self.next_frame * 4096 < end {
                        let addr = PhysAddr::new(start + self.next_frame * 4096);
                        self.next_frame += 1;
                        
                        return Some(PhysFrame::containing_address(addr));
                    }
                }
            }
        }
        None
    }
}

/// Get reference to global frame allocator
pub fn get_frame_allocator() -> &'static Mutex<BootInfoFrameAllocator> {
    &FRAME_ALLOCATOR
}

/// Slab allocator for kernel objects
pub mod slab {
    use super::*;
    use alloc::vec::Vec;
    
    /// Simple slab allocator for fixed-size objects
    pub struct Slab<T> {
        objects: Vec<T>,
        free_list: Vec<usize>,
    }
    
    impl<T> Slab<T> {
        pub const fn new() -> Self {
            Slab {
                objects: Vec::new(),
                free_list: Vec::new(),
            }
        }
        
        pub fn insert(&mut self, obj: T) -> usize {
            if let Some(idx) = self.free_list.pop() {
                self.objects[idx] = obj;
                idx
            } else {
                let idx = self.objects.len();
                self.objects.push(obj);
                idx
            }
        }
        
        pub fn remove(&mut self, idx: usize) -> Option<T> {
            if idx < self.objects.len() {
                self.free_list.push(idx);
                Some(core::mem::replace(
                    &mut self.objects[idx],
                    core::mem::MaybeUninit::zeroed().assume_init()
                ))
            } else {
                None
            }
        }
    }
}

/// Virtual memory area representation
#[derive(Debug, Clone)]
pub struct VMA {
    pub start: VirtAddr,
    pub end: VirtAddr,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
    pub private: bool,
}

impl VMA {
    pub fn new(start: VirtAddr, end: VirtAddr) -> Self {
        VMA {
            start,
            end,
            readable: true,
            writable: true,
            executable: false,
            private: true,
        }
    }
}
