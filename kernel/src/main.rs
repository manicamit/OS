#![no_std]
#![no_main]

mod idt;
mod vga;
mod pmm;
mod vmm;

use core::panic::PanicInfo;
use pmm::PhysicalMemoryManager;
use crate::vmm::PageTableManager;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    vga::println("KERNEL PANIC!");
    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct E820Entry {
    pub base: u64,
    pub length: u64,
    pub kind: u32,
    pub _ext: u32,
}

#[no_mangle]
pub extern "C" fn _start(e820_ptr: *const E820Entry, e820_count: usize) -> ! {
    vga::init();
    idt::init();
    
    vga::println("=== VMM Test ===");
    
    let entries = unsafe {
        core::slice::from_raw_parts(e820_ptr, e820_count)
    };
    
    let mut pmm = match PhysicalMemoryManager::new(entries) {
        Some(p) => p,
        None => {
            vga::println("ERROR: No RAM!");
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
    };
    vga::println("PMM initialized");
    
    // Setup recursive paging
    let pml4_phys = 0x2000u64;
    
    vga::println("Installing recursive mapping...");
    unsafe {
        let pml4 = pml4_phys as *mut u64;
        pml4.add(510).write_volatile(pml4_phys | 0x3);
        
        core::arch::asm!(
            "mov rax, cr3",
            "mov cr3, rax",
            out("rax") _,
        );
    }
    vga::println("Recursive mapping installed");
    
    // Allocate frames
    let phys_low = pmm.alloc_frame().expect("No frames");
    let phys_high = pmm.alloc_frame().expect("No frames");
    
    vga::print("Frames: ");
    vga::print_hex(phys_low);
    vga::print(", ");
    vga::print_hex(phys_high);
    vga::println("");
    
    // Create VMM
    let mut vmm = unsafe {
        PageTableManager::new(pml4_phys, &mut pmm)
    };
    vga::println("VMM created");
    
    // Test 1: Lower-half
    vga::println("");
    vga::println("=== Lower-half test ===");
    let virt_low = 0x0000_0000_0040_0000;
    
    vga::print("Mapping ");
    vga::print_hex(virt_low);
    vga::println("...");
    
    unsafe {
        vmm.map_page(virt_low, phys_low, 0x3);
    }
    vga::println("Mapped");
    
    unsafe {
        *(virt_low as *mut u64) = 0xCAFEBABEDEADBEEF;
        let val = *(virt_low as *const u64);
        
        if val == 0xCAFEBABEDEADBEEF {
            vga::println("Lower-half: SUCCESS");
        } else {
            vga::println("Lower-half: FAILED");
        }
    }
    
    // Delay before higher-half test
    vga::println("Waiting 2 seconds...");
    let iterations = 2 * 5_000_000;
    for _ in 0..iterations {
        unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
    }
    
    // Test 2: Higher-half with SMALLER address first
    vga::println("");
    vga::println("=== Higher-half test (simple) ===");
    
    // Try a simpler higher-half address in PML4[256]
    let virt_high_simple = 0xFFFF_8000_0000_0000u64;
    
    vga::print("Virtual: ");
    vga::print_hex(virt_high_simple);
    vga::println("");
    
    vga::print("Physical: ");
    vga::print_hex(phys_high);
    vga::println("");
    
    // Delay
    for _ in 0..(2 * 5_000_000) {
        unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
    }
    
    vga::println("Calling map_page...");
    
    unsafe {
        vmm.map_page(virt_high_simple, phys_high, 0x3);
    }
    
    vga::println("map_page returned successfully!");
    
    // Delay
    for _ in 0..(2 * 5_000_000) {
        unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
    }
    
    vga::println("About to write to mapped address...");
    
    unsafe {
        *(virt_high_simple as *mut u64) = 0xDEADC0DE;
    }
    
    vga::println("Write successful!");
    
    // Delay
    for _ in 0..(2 * 5_000_000) {
        unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
    }
    
    vga::println("About to read from mapped address...");
    
    let val = unsafe { *(virt_high_simple as *const u64) };
    
    vga::println("Read successful!");
    
    // Delay
    for _ in 0..(2 * 5_000_000) {
        unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
    }
    
    vga::print("Value: ");
    vga::print_hex(val);
    vga::println("");
    
    if val == 0xDEADC0DE {
        vga::println("Higher-half (simple): SUCCESS");
    } else {
        vga::println("Higher-half (simple): FAILED");
    }
    
    vga::println("");
    vga::println("=== ALL TESTS COMPLETE ===");
    
    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}
