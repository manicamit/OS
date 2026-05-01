use core::alloc::{GlobalAlloc, Layout};
use core::mem;
use core::ptr;
use core::cell::UnsafeCell;

struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    const fn new(size: usize) -> Self { ListNode { size, next: None } }
    fn start_addr(&self) -> usize { self as *const Self as usize }
    fn end_addr(&self) -> usize { self.start_addr() + self.size }
}

pub struct LinkedListAllocator {
    head: UnsafeCell<ListNode>,
}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self { head: UnsafeCell::new(ListNode::new(0)) }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        let aligned = align_up(heap_start, mem::align_of::<ListNode>());
        let size = heap_size - (aligned - heap_start);
        let node_ptr = aligned as *mut ListNode;
        ptr::write(node_ptr, ListNode::new(size));
        (*self.head.get()).next = Some(&mut *node_ptr);
    }

    unsafe fn insert_free_region(&self, addr: usize, size: usize) {
        let aligned = align_up(addr, mem::align_of::<ListNode>());
        if size < mem::size_of::<ListNode>() { return; }

        let mut prev = &mut *self.head.get() as *mut ListNode;

        // Find insertion point: prev -> [NEW] -> next, sorted by address
        while let Some(ref mut next_node) = (*prev).next {
            if next_node.start_addr() >= addr { break; }
            prev = *next_node as *mut ListNode;
        }

        let next_opt = (*prev).next.take();
        let new_addr = aligned;
        let mut new_size = size;

        // Try merge with next block
        let mut new_next = next_opt;
        if let Some(next_node) = new_next.as_mut() {
            if new_addr + new_size == next_node.start_addr() {
                let mut absorbed = new_next.take().unwrap();
                new_size += absorbed.size;
                new_next = absorbed.next.take();
            }
        }

        // Try merge with prev block (if prev is a real region, not the dummy head)
        let prev_end = (*prev).start_addr() + (*prev).size;
        if (*prev).size > 0 && prev_end == new_addr {
            (*prev).size += new_size;
            (*prev).next = new_next;
        } else {
            let node_ptr = new_addr as *mut ListNode;
            ptr::write(node_ptr, ListNode {
                size: new_size,
                next: new_next,
            });
            (*prev).next = Some(&mut *node_ptr);
        }
    }

    unsafe fn find_region(&self, size: usize, align: usize) -> Option<(&'static mut ListNode, usize)> {
        let mut current = &mut (*self.head.get()).next;
        while let Some(ref mut region) = current {
            if let Ok(alloc_start) = Self::alloc_from_region(region, size, align) {
                let next = region.next.take();
                let ret = Some((current.take().unwrap(), alloc_start));
                *current = next;
                return ret;
            } else {
                current = &mut current.as_mut().unwrap().next;
            }
        }
        None
    }

    fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;
        if alloc_end > region.end_addr() { return Err(()); }
        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() { return Err(()); }
        Ok(alloc_start)
    }

    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout.align_to(mem::align_of::<ListNode>()).expect("align failed").pad_to_align();
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }
}

unsafe impl GlobalAlloc for LinkedListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let flags: u64;
        core::arch::asm!("pushfq; pop {}", out(reg) flags);
        core::arch::asm!("cli", options(nomem, nostack));

        let (size, align) = LinkedListAllocator::size_align(layout);
        let result = if let Some((region, alloc_start)) = self.find_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            if excess_size > 0 { self.insert_free_region(alloc_end, excess_size); }
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        };

        if flags & 0x200 != 0 { core::arch::asm!("sti", options(nomem, nostack)); }
        result
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let flags: u64;
        core::arch::asm!("pushfq; pop {}", out(reg) flags);
        core::arch::asm!("cli", options(nomem, nostack));

        let (size, _) = LinkedListAllocator::size_align(layout);
        self.insert_free_region(ptr as usize, size);

        if flags & 0x200 != 0 { core::arch::asm!("sti", options(nomem, nostack)); }
    }
}

unsafe impl Sync for LinkedListAllocator {}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
