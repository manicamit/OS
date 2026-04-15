// kernel/src/allocator/bump.rs
//! Simple bump allocator - allocates linearly, never frees

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use core::cell::UnsafeCell;

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: UnsafeCell<usize>,
    allocations: UnsafeCell<usize>,
}

impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            heap_start: 0,
            heap_end: 0,
            next: UnsafeCell::new(0),
            allocations: UnsafeCell::new(0),
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        *self.next.get() = heap_start;
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let next = *self.next.get();
        let alloc_start = align_up(next, layout.align());
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return ptr::null_mut(),
        };

        if alloc_end > self.heap_end {
            ptr::null_mut() // Out of memory
        } else {
            *self.next.get() = alloc_end;
            *self.allocations.get() += 1;
            
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator doesn't free
    }
}

// IMPORTANT: BumpAllocator must be Sync for use as global allocator
unsafe impl Sync for BumpAllocator {}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
