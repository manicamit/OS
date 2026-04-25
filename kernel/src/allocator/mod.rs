pub mod bump;
pub mod linked_list;

use linked_list::LinkedListAllocator;
use core::alloc::{GlobalAlloc, Layout};

#[global_allocator]
static ALLOCATOR: Locked<LinkedListAllocator> = Locked::new(LinkedListAllocator::new());

pub const HEAP_START: usize = 0xFFFF_C444_4444_0000;
pub const HEAP_SIZE: usize = 1024 * 1024;

struct HeapFrameAllocator {
    current: u64,
    end: u64,
}

impl HeapFrameAllocator {
    fn alloc_frame(&mut self) -> Option<u64> {
        if self.current + 4096 > self.end { return None; }
        let frame = self.current;
        self.current += 4096;
        Some(frame)
    }
}

const RECURSIVE_INDEX: u64 = 510;

#[inline(always)]
fn make_recursive_va(i3: u64, i2: u64, i1: u64, i0: u64) -> u64 {
    let raw = (i3 << 39) | (i2 << 30) | (i1 << 21) | (i0 << 12);
    if raw & (1u64 << 47) != 0 { raw | 0xFFFF_0000_0000_0000 } else { raw }
}

#[inline(always)]
fn pml4_entry_ptr(pml4_i: u64) -> *mut u64 {
    let base = make_recursive_va(RECURSIVE_INDEX, RECURSIVE_INDEX, RECURSIVE_INDEX, RECURSIVE_INDEX);
    (base + pml4_i * 8) as *mut u64
}

#[inline(always)]
fn pdpt_entry_ptr(pml4_i: u64, pdpt_i: u64) -> *mut u64 {
    let base = make_recursive_va(RECURSIVE_INDEX, RECURSIVE_INDEX, RECURSIVE_INDEX, pml4_i);
    (base + pdpt_i * 8) as *mut u64
}

#[inline(always)]
fn pd_entry_ptr(pml4_i: u64, pdpt_i: u64, pd_i: u64) -> *mut u64 {
    let base = make_recursive_va(RECURSIVE_INDEX, RECURSIVE_INDEX, pml4_i, pdpt_i);
    (base + pd_i * 8) as *mut u64
}

#[inline(always)]
fn pt_entry_ptr(pml4_i: u64, pdpt_i: u64, pd_i: u64, pt_i: u64) -> *mut u64 {
    let base = make_recursive_va(RECURSIVE_INDEX, pml4_i, pdpt_i, pd_i);
    (base + pt_i * 8) as *mut u64
}

#[inline(always)]
unsafe fn flush_tlb_all() {
    core::arch::asm!("mov rax, cr3; mov cr3, rax", out("rax") _, options(nostack));
}

#[inline(always)]
unsafe fn flush_tlb_page(virt: u64) {
    core::arch::asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags));
}

pub unsafe fn map_heap_page(virt: u64, phys: u64, flags: u64, frame_alloc: &mut HeapFrameAllocator) -> bool {
    let pml4_i = (virt >> 39) & 0x1FF;
    let pdpt_i = (virt >> 30) & 0x1FF;
    let pd_i   = (virt >> 21) & 0x1FF;
    let pt_i   = (virt >> 12) & 0x1FF;


    let pml4_e = pml4_entry_ptr(pml4_i);
    let pml4_val = core::ptr::read_volatile(pml4_e);
    if (pml4_val & 1) == 0 {
        let frame = match frame_alloc.alloc_frame() { Some(f) => f, None => return false };
        core::ptr::write_volatile(pml4_e, frame | 0x3);
        let pdpt_base = make_recursive_va(RECURSIVE_INDEX, RECURSIVE_INDEX, RECURSIVE_INDEX, pml4_i) as *mut u64;
        for j in 0..512u64 { core::ptr::write_volatile(pdpt_base.add(j as usize), 0u64); }
    }

    let pdpt_e = pdpt_entry_ptr(pml4_i, pdpt_i);
    let pdpt_val = core::ptr::read_volatile(pdpt_e);
    if (pdpt_val & 1) == 0 {
        let frame = match frame_alloc.alloc_frame() { Some(f) => f, None => return false };
        core::ptr::write_volatile(pdpt_e, frame | 0x3);
        let pd_base = make_recursive_va(RECURSIVE_INDEX, RECURSIVE_INDEX, pml4_i, pdpt_i) as *mut u64;
        for j in 0..512u64 { core::ptr::write_volatile(pd_base.add(j as usize), 0u64); }
    }

    let pd_e = pd_entry_ptr(pml4_i, pdpt_i, pd_i);
    let pd_val = core::ptr::read_volatile(pd_e);
    if (pd_val & 1) == 0 {
        let frame = match frame_alloc.alloc_frame() { Some(f) => f, None => return false };
        core::ptr::write_volatile(pd_e, frame | 0x3);
        let pt_base = make_recursive_va(RECURSIVE_INDEX, pml4_i, pdpt_i, pd_i) as *mut u64;
        for j in 0..512u64 { core::ptr::write_volatile(pt_base.add(j as usize), 0u64); }
    }

    let pt_e = pt_entry_ptr(pml4_i, pdpt_i, pd_i, pt_i);
    core::ptr::write_volatile(pt_e, phys | flags | 0x1);
    core::arch::asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags));
    true
}

pub fn init_heap(pmm_current: u64, pmm_end: u64) -> Result<u64, &'static str> {
    use crate::{serial, vga};

    vga::println("");
    vga::println("=== Heap Allocator Init ===");
    vga::print("Heap region: ");
    vga::print_hex(HEAP_START as u64);
    vga::print(" - ");
    vga::print_hex((HEAP_START + HEAP_SIZE) as u64);
    vga::println("");

    let mut frame_alloc = HeapFrameAllocator { current: pmm_current, end: pmm_end };
    let num_pages = HEAP_SIZE / 4096;

    vga::print("Mapping ");
    vga::print_num(num_pages as u64);
    vga::println(" heap pages...");

    for i in 0..num_pages {
        let virt_addr = (HEAP_START + i * 4096) as u64;
        let phys_frame = match frame_alloc.alloc_frame() {
            Some(f) => f,
            None => return Err("Out of physical memory for heap"),
        };
        let ok = unsafe { map_heap_page(virt_addr, phys_frame, 0x2, &mut frame_alloc) };
        if !ok { return Err("Failed to map heap page"); }
    }

    vga::println("Heap pages mapped!");

    // Verify access
    unsafe {
        let test_ptr = HEAP_START as *mut u64;
        core::ptr::write_volatile(test_ptr, 0xDEAD_BEEF_CAFE_1234);
        let test_val = core::ptr::read_volatile(test_ptr);
        if test_val != 0xDEAD_BEEF_CAFE_1234 { return Err("Heap memory verification failed"); }
        core::ptr::write_volatile(test_ptr, 0);
    }
    vga::println("Verifying heap memory access...");
    vga::println("Heap memory access: OK");

    unsafe { ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE); }
    vga::println("Heap allocator initialized!");
    Ok(frame_alloc.current)
}

pub struct Locked<T> {
    inner: core::cell::UnsafeCell<T>,
}

impl<T> Locked<T> {
    pub const fn new(inner: T) -> Self {
        Locked { inner: core::cell::UnsafeCell::new(inner) }
    }
    pub fn lock(&self) -> &mut T {
        unsafe { &mut *self.inner.get() }
    }
}

unsafe impl<T: GlobalAlloc> GlobalAlloc for Locked<T> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 { self.lock().alloc(layout) }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) { self.lock().dealloc(ptr, layout) }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let new_ptr = self.lock().alloc(new_layout);
        if !new_ptr.is_null() {
            core::ptr::copy_nonoverlapping(ptr, new_ptr, core::cmp::min(layout.size(), new_size));
            self.lock().dealloc(ptr, layout);
        }
        new_ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let ptr = self.alloc(layout);
        if !ptr.is_null() { core::ptr::write_bytes(ptr, 0, size); }
        ptr
    }
}

unsafe impl<T> Sync for Locked<T> {}

pub fn test_allocator() {
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    use crate::vga;

    vga::println("");
    vga::println("=== Testing Heap Allocator ===");

    vga::println("Test 1: Box<u64>");
    let heap_value_1 = Box::new(41u64);
    let heap_value_2 = Box::new(13u64);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
    vga::println("  Box allocation: OK");

    vga::println("Test 2: Vec<u32>");
    let mut vec = Vec::new();
    for i in 0..500u32 { vec.push(i); }
    assert_eq!(vec.iter().sum::<u32>(), (499 * 500) / 2);
    vga::println("  Vec allocation: OK");

    vga::println("Test 3: Many alloc+dealloc cycles");
    for i in 0..1000u64 {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    vga::println("  Many allocations: OK");

    vga::println("Test 4: Large Vec (10000 elements)");
    let mut large_vec = Vec::new();
    for i in 0..10000u64 { large_vec.push(i); }
    assert_eq!(large_vec.len(), 10000);
    vga::println("  Large Vec: OK");

    vga::println("Test 5: alloc::string::String");
    let mut s = alloc::string::String::new();
    for _ in 0..100 { s.push_str("hello "); }
    assert_eq!(s.len(), 600);
    vga::println("  String allocation: OK");

    vga::println("");
    vga::println("=== All Allocator Tests Passed! ===");
}
