use core::fmt;
use core::sync::atomic::{AtomicBool, Ordering};

const SERIAL_PORT: u16 = 0x3F8;

static SERIAL_LOCK: AtomicBool = AtomicBool::new(false);

fn lock() -> bool {
    let flags: u64;
    unsafe { core::arch::asm!("pushfq; pop {}", out(reg) flags); }
    let was_enabled = flags & 0x200 != 0;
    unsafe { core::arch::asm!("cli", options(nomem, nostack)); }
    while SERIAL_LOCK.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        core::hint::spin_loop();
    }
    was_enabled
}

fn unlock(restore_interrupts: bool) {
    SERIAL_LOCK.store(false, Ordering::Release);
    if restore_interrupts {
        unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
    }
}

pub fn init() {
    unsafe {
        outb(SERIAL_PORT + 1, 0x00);
        outb(SERIAL_PORT + 3, 0x80);
        outb(SERIAL_PORT + 0, 0x03);
        outb(SERIAL_PORT + 1, 0x00);
        outb(SERIAL_PORT + 3, 0x03);
        outb(SERIAL_PORT + 2, 0xC7);
        outb(SERIAL_PORT + 4, 0x0B);
    }
}

pub fn write_byte(byte: u8) {
    unsafe {
        while (inb(SERIAL_PORT + 5) & 0x20) == 0 {}
        outb(SERIAL_PORT, byte);
    }
}

fn write_str_raw(s: &str) {
    for byte in s.bytes() { write_byte(byte); }
}

unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack));
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack));
    value
}

pub struct SerialWriter;

impl fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_str_raw(s);
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
        $crate::serial::write_byte(b'\n');
    }};
}

pub fn print(s: &str) {
    let ie = lock();
    write_str_raw(s);
    unlock(ie);
}

pub fn println(s: &str) {
    let ie = lock();
    write_str_raw(s);
    write_str_raw("\n");
    unlock(ie);
}

pub fn print_hex(num: u64) {
    let ie = lock();
    write_str_raw("0x");
    for i in (0..16).rev() {
        let nibble = ((num >> (i * 4)) & 0xF) as u8;
        let ch = if nibble < 10 { b'0' + nibble } else { b'A' + (nibble - 10) };
        write_byte(ch);
    }
    unlock(ie);
}

pub fn print_num(mut num: u64) {
    let ie = lock();
    if num == 0 { write_byte(b'0'); unlock(ie); return; }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while num > 0 {
        buf[i] = b'0' + (num % 10) as u8;
        num /= 10;
        i += 1;
    }
    while i > 0 { i -= 1; write_byte(buf[i]); }
    unlock(ie);
}

pub fn locked_begin() -> bool { lock() }
pub fn locked_end(ie: bool) { unlock(ie); }

pub fn write_str_unlocked(s: &str) {
    write_str_raw(s);
}

pub fn write_num_unlocked(mut num: u64) {
    if num == 0 { write_byte(b'0'); return; }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while num > 0 {
        buf[i] = b'0' + (num % 10) as u8;
        num /= 10;
        i += 1;
    }
    while i > 0 { i -= 1; write_byte(buf[i]); }
}
