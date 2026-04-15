// kernel/src/allocator/linked_list.rs
//! Linked list allocator - proper malloc/free implementation

use core::alloc::{GlobalAlloc, Layout};
use core::mem;
use core::ptr;
use core::cell::UnsafeCell;

struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

pub struct LinkedListAllocator {
    head: UnsafeCell<Option<&'static mut ListNode>>,
}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self { 
            head: UnsafeCell::new(None)
        }
    }

    /// Initialize the allocator with the given heap bounds
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size);
    }

    /// Add a free region to the free list
    unsafe fn add_free_region(&self, addr: usize, size: usize) {
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());

        let mut node = ListNode::new(size);
        node.next = (*self.head.get()).take();
        let node_ptr = addr as *mut ListNode;
        node_ptr.write(node);
        *self.head.get() = Some(&mut *node_ptr)
    }

    /// Find a suitable region and remove it from the list
    unsafe fn find_region(&self, size: usize, align: usize) -> Option<(&'static mut ListNode, usize)> {
        let head = &mut *self.head.get();
        let mut current = &mut *head;

        while let Some(ref mut region) = current {
            if let Ok(alloc_start) = Self::alloc_from_region(region, size, align) {
                // Region suitable - remove from list
                let next = region.next.take();
                let ret = Some((current.take().unwrap(), alloc_start));
                *current = next;
                return ret;
            } else {
                // Not suitable - try next
                current = &mut current.as_mut().unwrap().next;
            }
        }

        None
    }

    /// Try to use the given region for an allocation
    fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            return Err(());
        }

        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            return Err(());
        }

        Ok(alloc_start)
    }

    /// Adjust layout for ListNode storage
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<ListNode>())
            .expect("adjusting alignment failed")
            .pad_to_align();
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }
}

unsafe impl GlobalAlloc for LinkedListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let (size, align) = LinkedListAllocator::size_align(layout);

        if let Some((region, alloc_start)) = self.find_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            
            if excess_size > 0 {
                self.add_free_region(alloc_end, excess_size);
            }
            
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let (size, _) = LinkedListAllocator::size_align(layout);
        self.add_free_region(ptr as usize, size)
    }
}

// IMPORTANT: Must be Sync for use as global allocator
unsafe impl Sync for LinkedListAllocator {}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
