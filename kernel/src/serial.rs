// kernel/src/serial.rs
//! Simple serial port driver for debugging

use core::fmt;

const SERIAL_PORT: u16 = 0x3F8; // COM1

/// Initialize serial port
pub fn init() {
    unsafe {
        // Disable interrupts
        outb(SERIAL_PORT + 1, 0x00);
        // Enable DLAB
        outb(SERIAL_PORT + 3, 0x80);
        // Set divisor to 3 (38400 baud)
        outb(SERIAL_PORT + 0, 0x03);
        outb(SERIAL_PORT + 1, 0x00);
        // 8 bits, no parity, one stop bit
        outb(SERIAL_PORT + 3, 0x03);
        // Enable FIFO
        outb(SERIAL_PORT + 2, 0xC7);
        // Mark data terminal ready
        outb(SERIAL_PORT + 4, 0x0B);
    }
}

/// Write a byte to serial port
fn write_byte(byte: u8) {
    unsafe {
        // Wait for transmit buffer to be empty
        while (inb(SERIAL_PORT + 5) & 0x20) == 0 {}
        outb(SERIAL_PORT, byte);
    }
}

/// Write a string to serial
pub fn write_str(s: &str) {
    for byte in s.bytes() {
        write_byte(byte);
    }
}

/// Write with newline
pub fn writeln(s: &str) {
    write_str(s);
    write_str("\n");
}

unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack));
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack));
    value
}

// Formatter for easy printing
pub struct SerialWriter;

impl fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_str(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::serial::SerialWriter, $($arg)*);
    }};
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::serial::SerialWriter, $($arg)*);
        $crate::serial::write_str("\n");
    }};
}
