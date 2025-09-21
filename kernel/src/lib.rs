#![no_std]  // no standard library
#![no_main] // no normal main entry point

use core::panic::PanicInfo;

/// This function is called on panic.
/// For now, just loop forever.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/// Our entry point (`_start`) called by the bootloader.
/// We’ll link this symbol later.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // For now, do nothing and loop forever.
    loop {}
}
