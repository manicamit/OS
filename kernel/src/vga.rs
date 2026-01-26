use core::ptr::write_volatile;

const VGA_BUFFER: usize = 0xB8000;
const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;

#[repr(u8)]
#[allow(dead_code)]
pub enum Color {
    Black = 0x0,
    Blue = 0x1,
    Green = 0x2,
    Cyan = 0x3,
    Red = 0x4,
    Magenta = 0x5,
    Brown = 0x6,
    LightGray = 0x7,
    DarkGray = 0x8,
    LightBlue = 0x9,
    LightGreen = 0xA,
    LightCyan = 0xB,
    LightRed = 0xC,
    Pink = 0xD,
    Yellow = 0xE,
    White = 0xF,
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
        Self {
            column: 0,
            row: 0,
            color: ColorCode::new(Color::LightGray, Color::Black),
        }
    }

    fn buffer_ptr(&self) -> *mut VgaChar {
        VGA_BUFFER as *mut VgaChar
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
            write_volatile(
                self.buffer_ptr().add(index),
                VgaChar {
                    ascii: byte,
                    color: self.color,
                },
            );
        }
    }

    fn new_line(&mut self) {
        self.column = 0;
        if self.row < VGA_HEIGHT - 1 {
            self.row += 1;
        } else {
            self.scroll();
        }
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
        // clear last line
        for col in 0..VGA_WIDTH {
            self.write_at(b' ', VGA_HEIGHT - 1, col);
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column >= VGA_WIDTH {
                    self.new_line();
                }
                self.write_at(byte, self.row, self.column);
                self.column += 1;
            }
        }
    }

    pub fn write_str(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
    }
}

/* ===============================
   Global writer (early kernel)
   =============================== */

static mut WRITER: VgaWriter = VgaWriter::new();

pub fn init() {
    unsafe {
        let writer = core::ptr::addr_of_mut!(WRITER);
        (*writer).clear_screen();
    }
}

#[allow(dead_code)]
pub fn print(s: &str) {
    unsafe {
        let writer = core::ptr::addr_of_mut!(WRITER);
        (*writer).write_str(s);
    }
}

pub fn println(s: &str) {
    unsafe {
        let writer = core::ptr::addr_of_mut!(WRITER);
        (*writer).write_str(s);
        (*writer).write_byte(b'\n');
    }
}
