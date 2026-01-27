#![no_std]
#![no_main]

mod idt;
mod vga;

use core::panic::PanicInfo;

/// Panic handler — halt forever
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    vga::println("KERNEL PANIC!");
    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}

/// BIOS E820 memory map entry
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct E820Entry {
    pub base: u64,
    pub length: u64,
    pub kind: u32,
    pub _ext: u32,
}

fn delay_seconds(seconds: u32) {
    let iterations = seconds * 5_000_000;
    for _ in 0..iterations {
        unsafe {
            core::arch::asm!("pause", options(nomem, nostack));
        }
    }
}

fn e820_type_str(kind: u32) -> &'static str {
    match kind {
        1 => "USABLE",
        2 => "RESERVED",
        3 => "ACPI",
        4 => "ACPI_NVS",
        5 => "BAD",
        _ => "UNKNOWN",
    }
}

#[no_mangle]
pub extern "C" fn _start(e820_ptr: *const E820Entry, e820_count: usize) -> ! {
    delay_seconds(2);
    
    vga::init();
    idt::init();
    
    vga::println("Hello from kernel!");
    vga::println("VGA driver initialized.");
    vga::println("IDT initialized.");
    vga::println("");
    
    // Debug: print the pointer and count
    vga::print("E820 pointer: ");
    vga::print_hex(e820_ptr as u64);
    vga::println("");
    
    vga::print("E820 count: ");
    vga::print_num(e820_count as u64);
    vga::println("");
    vga::println("");
    
    // Read the raw memory at the E820 buffer address
    vga::println("First 64 bytes at E820 buffer:");
    let raw_ptr = 0x8000 as *const u64;
    for i in 0..8 {
        unsafe {
            vga::print("  ");
            vga::print_hex((0x8000 + i * 8) as u64);
            vga::print(": ");
            vga::print_hex(*raw_ptr.add(i));
            vga::println("");
        }
    }
    vga::println("");
    
    if e820_count == 0 {
        vga::println("ERROR: E820 count is 0!");
        loop {
            unsafe { core::arch::asm!("cli; hlt"); }
        }
    }
    
    vga::println("E820 Memory Map:");
    
    let count = core::cmp::min(e820_count, 128);
    let entries = unsafe {
        core::slice::from_raw_parts(e820_ptr, count)
    };
    
    for (i, e) in entries.iter().enumerate() {
        // Skip entries that are clearly invalid
        if e.base == 0 && e.length == 0 && e.kind == 0 {
            continue;
        }
        
        vga::print("  ");
        vga::print_num(i as u64);
        vga::print(": ");
        vga::print_hex(e.base);
        vga::print(" - ");
        vga::print_hex(e.base + e.length);
        vga::print(" (");
        vga::print_hex(e.length);
        vga::print(") ");
        vga::println(e820_type_str(e.kind));
        
        // Only show first 10 entries for readability
        if i >= 9 {
            vga::print("  ... (");
            vga::print_num((count - 10) as u64);
            vga::println(" more entries)");
            break;
        }
    }
    
    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}
