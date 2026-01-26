#![no_std]
#![no_main]

mod idt;
mod vga;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    vga::println("KERNEL PANIC!");
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

/// CPU cycle based delay using PAUSE instruction
fn delay_seconds(seconds: u32) {
    let iterations = seconds * 5_000_000;
    
    for _ in 0..iterations {
        unsafe {
            core::arch::asm!("pause", options(nomem, nostack));
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Wait 2 seconds to see bootloader output
    delay_seconds(2);
    
    // Initialize VGA and IDT
    vga::init();
    idt::init();
    
    // Display kernel info
    vga::println("Hello from kernel!");
    vga::println("VGA driver initialized.");
    vga::println("IDT initialized.");
    vga::println("Page fault handler ready.");
    vga::println("");
    vga::println("Testing page fault in 3 seconds...");
    
    // Wait 3 seconds
    delay_seconds(3);
    
    vga::println("Triggering page fault NOW!");
    
    // Small delay to ensure message is visible
    delay_seconds(1);
    
    // Trigger page fault by accessing unmapped memory
    // Use address above 4GB (our identity map only covers first 4GB)
    unsafe {
        core::arch::asm!(
            "mov rax, 0x0000500000000000",  // 5TB - definitely not mapped
            "mov qword ptr [rax], 0x1234",   // This will cause page fault
            out("rax") _,
        );
    }
    
    // Should never reach here
    vga::println("ERROR: Page fault handler failed!");
    
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}
