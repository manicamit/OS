// kernel/src/allocator/mod.rs
//! Kernel heap allocator

pub mod bump;
pub mod linked_list;

use linked_list::LinkedListAllocator;
use core::alloc::{GlobalAlloc, Layout};

#[global_allocator]
static ALLOCATOR: Locked<LinkedListAllocator> = Locked::new(LinkedListAllocator::new());

pub const HEAP_START: usize = 0xFFFF_C444_4444_0000;
pub const HEAP_SIZE: usize = 1024 * 1024; // 1 MiB

/// Simple physical frame allocator for heap init
struct HeapFrameAllocator {
    current: u64,
    end: u64,
}

impl HeapFrameAllocator {
    fn alloc_frame(&mut self) -> Option<u64> {
        if self.current + 4096 > self.end {
            return None;
        }
        let frame = self.current;
        self.current += 4096;
        Some(frame)
    }
}

// ============================================================================
// Recursive Page Table Access (PML4[510] = self-reference)
// ============================================================================

const RECURSIVE_INDEX: u64 = 510;

/// Build canonical virtual address from four 9-bit page-table indices
#[inline(always)]
fn make_recursive_va(i3: u64, i2: u64, i1: u64, i0: u64) -> u64 {
    let raw = (i3 << 39) | (i2 << 30) | (i1 << 21) | (i0 << 12);
    // Sign-extend from bit 47
    if raw & (1u64 << 47) != 0 {
        raw | 0xFFFF_0000_0000_0000
    } else {
        raw
    }
}

/// Pointer to PML4 entry [pml4_i]
#[inline(always)]
fn pml4_entry_ptr(pml4_i: u64) -> *mut u64 {
    let base = make_recursive_va(RECURSIVE_INDEX, RECURSIVE_INDEX, RECURSIVE_INDEX, RECURSIVE_INDEX);
    (base + pml4_i * 8) as *mut u64
}

/// Pointer to PDPT entry [pml4_i][pdpt_i]
#[inline(always)]
fn pdpt_entry_ptr(pml4_i: u64, pdpt_i: u64) -> *mut u64 {
    let base = make_recursive_va(RECURSIVE_INDEX, RECURSIVE_INDEX, RECURSIVE_INDEX, pml4_i);
    (base + pdpt_i * 8) as *mut u64
}

/// Pointer to PD entry [pml4_i][pdpt_i][pd_i]
#[inline(always)]
fn pd_entry_ptr(pml4_i: u64, pdpt_i: u64, pd_i: u64) -> *mut u64 {
    let base = make_recursive_va(RECURSIVE_INDEX, RECURSIVE_INDEX, pml4_i, pdpt_i);
    (base + pd_i * 8) as *mut u64
}

/// Pointer to PT entry [pml4_i][pdpt_i][pd_i][pt_i]
#[inline(always)]
fn pt_entry_ptr(pml4_i: u64, pdpt_i: u64, pd_i: u64, pt_i: u64) -> *mut u64 {
    let base = make_recursive_va(RECURSIVE_INDEX, pml4_i, pdpt_i, pd_i);
    (base + pt_i * 8) as *mut u64
}

/// Flush the entire TLB by reloading CR3
#[inline(always)]
unsafe fn flush_tlb_all() {
    core::arch::asm!("mov rax, cr3; mov cr3, rax", out("rax") _, options(nostack));
}

/// Flush a single TLB entry
#[inline(always)]
unsafe fn flush_tlb_page(virt: u64) {
    core::arch::asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags));
}

pub unsafe fn map_heap_page(virt: u64, phys: u64, flags: u64, frame_alloc: &mut HeapFrameAllocator) -> bool {
    let pml4_i = (virt >> 39) & 0x1FF;
    let pdpt_i = (virt >> 30) & 0x1FF;
    let pd_i   = (virt >> 21) & 0x1FF;
    let pt_i   = (virt >> 12) & 0x1FF;

    crate::serial::print("map_heap virt:"); crate::serial::print_hex(virt);
    crate::serial::print(" phys:"); crate::serial::print_hex(phys);
    crate::serial::println("");

    let pml4_e = pml4_entry_ptr(pml4_i);
    let mut pml4_val = core::ptr::read_volatile(pml4_e);
    if (pml4_val & 1) == 0 {
        crate::serial::println("  > Alloc PDPT");
        let frame = match frame_alloc.alloc_frame() {
            Some(f) => f,
            None => { crate::serial::println("  > FAIL PDPT"); return false; },
        };
        crate::serial::print("  > Frame: "); crate::serial::print_hex(frame); crate::serial::println("");
        core::ptr::write_volatile(pml4_e, frame | 0x3);
        
        let pdpt_base = make_recursive_va(RECURSIVE_INDEX, RECURSIVE_INDEX, RECURSIVE_INDEX, pml4_i) as *mut u64;
        crate::serial::print("  > Zeroing "); crate::serial::print_hex(pdpt_base as u64); crate::serial::println("");
        for j in 0..512u64 {
            core::ptr::write_volatile(pdpt_base.add(j as usize), 0u64);
        }
        crate::serial::println("  > Zeroed PDPT");
    }

    let pdpt_e = pdpt_entry_ptr(pml4_i, pdpt_i);
    // crate::serial::print("  > Read PDPT_e "); 
    //crate::serial::print_hex(pdpt_e as u64); crate::serial::println("");
    let mut pdpt_val = core::ptr::read_volatile(pdpt_e);
    if (pdpt_val & 1) == 0 {
        crate::serial::println("  > Alloc PD");
        let frame = match frame_alloc.alloc_frame() {
            Some(f) => f,
            None => return false,
        };
        crate::serial::print("  > Frame: "); crate::serial::print_hex(frame); crate::serial::println("");
        core::ptr::write_volatile(pdpt_e, frame | 0x3);

        let pd_base = make_recursive_va(RECURSIVE_INDEX, RECURSIVE_INDEX, pml4_i, pdpt_i) as *mut u64;
        crate::serial::print("  > Zeroing "); crate::serial::print_hex(pd_base as u64); crate::serial::println("");
        for j in 0..512u64 {
            core::ptr::write_volatile(pd_base.add(j as usize), 0u64);
        }
        crate::serial::println("  > Zeroed PD");
    }

    let pd_e = pd_entry_ptr(pml4_i, pdpt_i, pd_i);
    //crate::serial::print("  > Read PD_e "); 
    //crate::serial::print_hex(pd_e as u64); crate::serial::println("");
    let mut pd_val = core::ptr::read_volatile(pd_e);
    if (pd_val & 1) == 0 {
        crate::serial::println("  > Alloc PT");
        let frame = match frame_alloc.alloc_frame() {
            Some(f) => f,
            None => return false,
        };
        crate::serial::print("  > Frame: "); crate::serial::print_hex(frame); crate::serial::println("");
        core::ptr::write_volatile(pd_e, frame | 0x3);

        let pt_base = make_recursive_va(RECURSIVE_INDEX, pml4_i, pdpt_i, pd_i) as *mut u64;
        crate::serial::print("  > Zeroing "); crate::serial::print_hex(pt_base as u64); crate::serial::println("");
        for j in 0..512u64 {
            core::ptr::write_volatile(pt_base.add(j as usize), 0u64);
        }
        crate::serial::println("  > Zeroed PT");
    }

    let pt_e = pt_entry_ptr(pml4_i, pdpt_i, pd_i, pt_i);
    //crate::serial::print("  > Write PT_e "); 
    //crate::serial::print_hex(pt_e as u64); crate::serial::println("");
    core::ptr::write_volatile(pt_e, phys | flags | 0x1);
    
    // Minimal TLB flush
    core::arch::asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags));

    true
}

/// Initialize the heap allocator.
/// Uses recursive page table mapping (PML4[510]) — works after identity mapping removal.
pub fn init_heap(pmm_current: u64, pmm_end: u64) -> Result<u64, &'static str> {
    use crate::serial;
    use crate::vga;
    
    vga::println("");
    vga::println("=== Heap Allocator Init ===");
    serial::println("Initializing kernel heap...");
    
    vga::print("Heap region: ");
    vga::print_hex(HEAP_START as u64);
    vga::print(" - ");
    vga::print_hex((HEAP_START + HEAP_SIZE) as u64);
    vga::println("");

    let mut frame_alloc = HeapFrameAllocator {
        current: pmm_current,
        end: pmm_end,
    };

    let num_pages = HEAP_SIZE / 4096;
    vga::print("Mapping ");
    vga::print_num(num_pages as u64);
    vga::println(" heap pages...");

    serial::print("Pages to map: ");
    serial::print_num(num_pages as u64);
    serial::println("");

    for i in 0..num_pages {
        let virt_addr = (HEAP_START + i * 4096) as u64;
        
        // crate::serial::print("L: ");
        // crate::serial::print_num(i as u64);
        // crate::serial::println("");

        let phys_frame = match frame_alloc.alloc_frame() {
            Some(f) => f,
            None => {
                crate::serial::println("ERR: OOM");
                return Err("Out of physical memory for heap");
            }
        };

        // crate::serial::println(" > Doing map...");

        let ok = unsafe {
            map_heap_page(virt_addr, phys_frame, 0x2, &mut frame_alloc)
        };

        // crate::serial::println(" > Map done");

        if !ok {
            crate::serial::println("ERR: map_heap_page false");
            return Err("Failed to map heap page");
        }

        if i < 3 || i == num_pages - 1 {
            crate::serial::print("  page ");
            crate::serial::print_num(i as u64);
            crate::serial::print(" virt=");
            crate::serial::print_hex(virt_addr);
            crate::serial::print(" -> phys=");
            crate::serial::print_hex(phys_frame);
            crate::serial::println("");
        }
    }
    
    vga::println("Heap pages mapped!");
    serial::println("Heap pages mapped!");
    
    // Verify heap memory access
    serial::println("Verifying heap memory access...");
    unsafe {
        let test_ptr = HEAP_START as *mut u64;
        core::ptr::write_volatile(test_ptr, 0xDEAD_BEEF_CAFE_1234);
        let test_val = core::ptr::read_volatile(test_ptr);
        if test_val != 0xDEAD_BEEF_CAFE_1234 {
            return Err("Heap memory verification failed");
        }
        core::ptr::write_volatile(test_ptr, 0);
    }
    vga::println("Heap memory verified!");
    serial::println("Heap memory access: OK");
    
    // Initialize the linked list allocator
    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }
    
    vga::println("Heap allocator initialized!");
    serial::println("Heap allocator initialized!");
    
    Ok(frame_alloc.current)
}

/// A wrapper around a type that provides interior mutability
pub struct Locked<T> {
    inner: core::cell::UnsafeCell<T>,
}

impl<T> Locked<T> {
    pub const fn new(inner: T) -> Self {
        Locked {
            inner: core::cell::UnsafeCell::new(inner),
        }
    }

    pub fn lock(&self) -> &mut T {
        unsafe { &mut *self.inner.get() }
    }
}

unsafe impl<T: GlobalAlloc> GlobalAlloc for Locked<T> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.lock().alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.lock().dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let new_ptr = self.lock().alloc(new_layout);
        if !new_ptr.is_null() {
            let copy_size = core::cmp::min(layout.size(), new_size);
            core::ptr::copy_nonoverlapping(ptr, new_ptr, copy_size);
            self.lock().dealloc(ptr, layout);
        }
        new_ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let ptr = self.alloc(layout);
        if !ptr.is_null() {
            core::ptr::write_bytes(ptr, 0, size);
        }
        ptr
    }
}

unsafe impl<T> Sync for Locked<T> {}

/// Test the allocator with Box, Vec, String, and stress tests
pub fn test_allocator() {
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    use crate::serial;
    use crate::vga;

    // Test panic
    //panic!("Testing panic handler!");

    vga::println("");
    vga::println("=== Testing Heap Allocator ===");
    serial::println("");
    serial::println("=== Testing Heap Allocator ===");

    // Test 1: Box allocation
    vga::println("Test 1: Box<u64>");
    serial::println("Test 1: Box<u64>");
    let heap_value_1 = Box::new(41u64);
    let heap_value_2 = Box::new(13u64);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
    vga::println("  Box allocation: OK");
    serial::println("  Box allocation: OK");

    // Test 2: Vec allocation
    vga::println("Test 2: Vec<u32>");
    serial::println("Test 2: Vec<u32>");
    let mut vec = Vec::new();
    serial::println("  Vec::new OK");
    for i in 0..500u32 {
        serial::print_num(i as u64);
        vec.push(i);
        if i % 100 == 0 {
            serial::print("    push ");
            serial::print_num(i as u64);
            serial::println("");
        }
    }
    serial::println("  push loop OK");
    assert_eq!(vec.iter().sum::<u32>(), (499 * 500) / 2);
    vga::println("  Vec allocation: OK");
    serial::println("  Vec allocation: OK");

    // Test 3: Many alloc+dealloc (tests free list reuse)
    vga::println("Test 3: Many alloc+dealloc cycles");
    serial::println("Test 3: Many alloc+dealloc cycles");
    for i in 0..1000u64 {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    vga::println("  Many allocations: OK");
    serial::println("  Many allocations: OK");

    // Test 4: Large allocation
    vga::println("Test 4: Large Vec (10000 elements)");
    serial::println("Test 4: Large Vec (10000 elements)");
    let mut large_vec = Vec::new();
    for i in 0..10000u64 {
        large_vec.push(i);
    }
    assert_eq!(large_vec.len(), 10000);
    vga::println("  Large Vec: OK");
    serial::println("  Large Vec: OK");

    // Test 5: String allocation
    vga::println("Test 5: alloc::string::String");
    serial::println("Test 5: alloc::string::String");
    let mut s = alloc::string::String::new();
    for _ in 0..100 {
        s.push_str("hello ");
    }
    assert_eq!(s.len(), 600);
    vga::println("  String allocation: OK");
    serial::println("  String allocation: OK");

    vga::println("");
    vga::println("=== All Allocator Tests Passed! ===");
    serial::println("");
    serial::println("=== All Allocator Tests Passed! ===");
}
