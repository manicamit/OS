use core::ptr::write_volatile;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const VGA_PHYS: u64 = 0xB8000;
const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;
pub const VGA_HIGHER_HALF: u64 = 0xFFFF_8000_000B_8000;

static VGA_BUFFER: AtomicUsize = AtomicUsize::new(0xB8000);

#[repr(u8)]
#[allow(dead_code)]
pub enum Color {
    Black = 0x0, Blue = 0x1, Green = 0x2, Cyan = 0x3,
    Red = 0x4, Magenta = 0x5, Brown = 0x6, LightGray = 0x7,
    DarkGray = 0x8, LightBlue = 0x9, LightGreen = 0xA, LightCyan = 0xB,
    LightRed = 0xC, Pink = 0xD, Yellow = 0xE, White = 0xF,
}

#[derive(Clone, Copy)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    const fn new(fg: Color, bg: Color) -> Self {
        Self((bg as u8) << 4 | (fg as u8))
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VgaChar {
    ascii: u8,
    color: ColorCode,
}

pub struct VgaWriter {
    column: usize,
    row: usize,
    color: ColorCode,
}

impl VgaWriter {
    pub const fn new() -> Self {
        Self { column: 0, row: 0, color: ColorCode::new(Color::LightGray, Color::Black) }
    }

    fn buffer_ptr(&self) -> *mut VgaChar {
        VGA_BUFFER.load(Ordering::Relaxed) as *mut VgaChar
    }

    pub fn clear_screen(&mut self) {
        for row in 0..VGA_HEIGHT {
            for col in 0..VGA_WIDTH {
                self.write_at(b' ', row, col);
            }
        }
        self.row = 0;
        self.column = 0;
    }

    fn write_at(&self, byte: u8, row: usize, col: usize) {
        let index = row * VGA_WIDTH + col;
        unsafe {
            write_volatile(self.buffer_ptr().add(index), VgaChar { ascii: byte, color: self.color });
        }
    }

    fn new_line(&mut self) {
        self.column = 0;
        if self.row < VGA_HEIGHT - 1 { self.row += 1; } else { self.scroll(); }
    }

    fn scroll(&mut self) {
        for row in 1..VGA_HEIGHT {
            for col in 0..VGA_WIDTH {
                let from = row * VGA_WIDTH + col;
                let to = (row - 1) * VGA_WIDTH + col;
                unsafe {
                    let val = *self.buffer_ptr().add(from);
                    write_volatile(self.buffer_ptr().add(to), val);
                }
            }
        }
        for col in 0..VGA_WIDTH { self.write_at(b' ', VGA_HEIGHT - 1, col); }
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column >= VGA_WIDTH { self.new_line(); }
                self.write_at(byte, self.row, self.column);
                self.column += 1;
            }
        }
    }

    pub fn write_str(&mut self, s: &str) {
        for byte in s.bytes() { self.write_byte(byte); }
    }
}

static mut WRITER: VgaWriter = VgaWriter::new();

pub fn init() {
    unsafe {
        let writer = core::ptr::addr_of_mut!(WRITER);
        (*writer).clear_screen();
    }
}

pub fn switch_to_higher_half() {
    VGA_BUFFER.store(VGA_HIGHER_HALF as usize, Ordering::Relaxed);
}

#[allow(dead_code)]
pub fn print(s: &str) {
    unsafe { let w = core::ptr::addr_of_mut!(WRITER); (*w).write_str(s); }
}

pub fn println(s: &str) {
    unsafe { let w = core::ptr::addr_of_mut!(WRITER); (*w).write_str(s); (*w).write_byte(b'\n'); }
}

pub fn print_num(n: u64) {
    if n == 0 { print("0"); return; }
    let mut num = n;
    let mut divisor = 1u64;
    while divisor <= num / 10 { divisor *= 10; }
    while divisor > 0 {
        let digit = (num / divisor) as u8;
        unsafe { let w = core::ptr::addr_of_mut!(WRITER); (*w).write_byte(b'0' + digit); }
        num %= divisor;
        divisor /= 10;
    }
}

pub fn print_hex(n: u64) {
    print("0x");
    for i in (0..16).rev() {
        let nibble = ((n >> (i * 4)) & 0xF) as u8;
        let ch = if nibble < 10 { b'0' + nibble } else { b'A' + (nibble - 10) };
        unsafe { let w = core::ptr::addr_of_mut!(WRITER); (*w).write_byte(ch); }
    }
}

pub fn print_ascii(bytes: &[u8]) {
    for &byte in bytes {
        if byte >= 0x20 && byte <= 0x7E {
            unsafe { let w = core::ptr::addr_of_mut!(WRITER); (*w).write_byte(byte); }
        }
    }
}

pub fn print_char(ch: char) {
    unsafe { let w = core::ptr::addr_of_mut!(WRITER); (*w).write_byte(ch as u8); }
}
