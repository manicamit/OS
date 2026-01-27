#![no_std]
#![no_main]

mod idt;
mod vga;
mod pmm;

use core::panic::PanicInfo;
use pmm::PhysicalMemoryManager;

/// Panic handler
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    vga::println("KERNEL PANIC!");
    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}

/// BIOS E820 memory map entry (matches stage2 layout)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct E820Entry {
    pub base: u64,
    pub length: u64,
    pub kind: u32,
    pub _ext: u32,
}

fn delay_seconds(seconds: u32) {
    let iterations = seconds * 5_000_000;
    for _ in 0..iterations {
        unsafe { core::arch::asm!("pause"); }
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

    vga::println("VGA driver initialized.");
    vga::println("IDT initialized.");
    vga::println("");

    vga::print("E820 pointer: ");
    vga::print_hex(e820_ptr as u64);
    vga::println("");

    vga::print("E820 count: ");
    vga::print_num(e820_count as u64);
    vga::println("");
    vga::println("");

    let entries = unsafe {
        core::slice::from_raw_parts(e820_ptr, e820_count)
    };

    vga::println("E820 Memory Map:");
    for (i, e) in entries.iter().enumerate() {
        if e.base == 0 && e.length == 0 {
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
    }

    vga::println("");
    vga::println("Initializing Physical Memory Manager...");

    let mut pmm = match PhysicalMemoryManager::new(entries) {
        Some(p) => p,
        None => {
            vga::println("ERROR: No usable RAM found!");
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
    };

    vga::println("PMM initialized.");
    vga::println("");
    vga::println("Allocating physical frames:");

    for i in 0..5 {
        match pmm.alloc_frame() {
            Some(frame) => {
                vga::print("  Frame ");
                vga::print_num(i);
                vga::print(": ");
                vga::print_hex(frame);
                vga::println("");
            }
            None => {
                vga::println("  OUT OF MEMORY");
            }
        }
    }

    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}
