#![no_std]
#![no_main]

pub mod serial;
pub mod vga;
pub mod vmm;
pub mod idt;
pub mod gdt;

use core::panic::PanicInfo;
use vga::{Color, Writer};


/// Panic handler
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}


/// Kernel entry point (called from stage2)
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // Create a VGA writer: white text on black background
    let mut writer = Writer::new(Color::White, Color::Black);

    // Clear the screen
    writer.clear_screen();

    // Write some text
    writer.write_str("Hello from Rust kernel!\n");
    writer.write_str("Long mode + paging + VGA works.\n");

    // Halt forever
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
