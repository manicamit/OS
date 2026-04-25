use core::sync::atomic::{AtomicUsize, Ordering};

const KBD_DATA_PORT: u16 = 0x60;
const BUF_SIZE: usize = 256;

static mut KEY_BUF: [u8; BUF_SIZE] = [0; BUF_SIZE];
static READ_POS: AtomicUsize = AtomicUsize::new(0);
static WRITE_POS: AtomicUsize = AtomicUsize::new(0);
static mut SHIFT_HELD: bool = false;

#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    core::arch::asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack));
    val
}

static SCANCODE_TO_ASCII: [u8; 128] = {
    let mut t = [0u8; 128];
    t[0x02] = b'1'; t[0x03] = b'2'; t[0x04] = b'3'; t[0x05] = b'4'; t[0x06] = b'5';
    t[0x07] = b'6'; t[0x08] = b'7'; t[0x09] = b'8'; t[0x0A] = b'9'; t[0x0B] = b'0';
    t[0x0C] = b'-'; t[0x0D] = b'='; t[0x0E] = 0x08; t[0x0F] = b'\t';
    t[0x10] = b'q'; t[0x11] = b'w'; t[0x12] = b'e'; t[0x13] = b'r'; t[0x14] = b't';
    t[0x15] = b'y'; t[0x16] = b'u'; t[0x17] = b'i'; t[0x18] = b'o'; t[0x19] = b'p';
    t[0x1A] = b'['; t[0x1B] = b']'; t[0x1C] = b'\n';
    t[0x1E] = b'a'; t[0x1F] = b's'; t[0x20] = b'd'; t[0x21] = b'f'; t[0x22] = b'g';
    t[0x23] = b'h'; t[0x24] = b'j'; t[0x25] = b'k'; t[0x26] = b'l';
    t[0x27] = b';'; t[0x28] = b'\''; t[0x29] = b'`'; t[0x2B] = b'\\';
    t[0x2C] = b'z'; t[0x2D] = b'x'; t[0x2E] = b'c'; t[0x2F] = b'v'; t[0x30] = b'b';
    t[0x31] = b'n'; t[0x32] = b'm'; t[0x33] = b','; t[0x34] = b'.'; t[0x35] = b'/';
    t[0x39] = b' ';
    t
};

static SCANCODE_TO_ASCII_SHIFT: [u8; 128] = {
    let mut t = [0u8; 128];
    t[0x02] = b'!'; t[0x03] = b'@'; t[0x04] = b'#'; t[0x05] = b'$'; t[0x06] = b'%';
    t[0x07] = b'^'; t[0x08] = b'&'; t[0x09] = b'*'; t[0x0A] = b'('; t[0x0B] = b')';
    t[0x0C] = b'_'; t[0x0D] = b'+'; t[0x0E] = 0x08; t[0x0F] = b'\t';
    t[0x10] = b'Q'; t[0x11] = b'W'; t[0x12] = b'E'; t[0x13] = b'R'; t[0x14] = b'T';
    t[0x15] = b'Y'; t[0x16] = b'U'; t[0x17] = b'I'; t[0x18] = b'O'; t[0x19] = b'P';
    t[0x1A] = b'{'; t[0x1B] = b'}'; t[0x1C] = b'\n';
    t[0x1E] = b'A'; t[0x1F] = b'S'; t[0x20] = b'D'; t[0x21] = b'F'; t[0x22] = b'G';
    t[0x23] = b'H'; t[0x24] = b'J'; t[0x25] = b'K'; t[0x26] = b'L';
    t[0x27] = b':'; t[0x28] = b'"'; t[0x29] = b'~'; t[0x2B] = b'|';
    t[0x2C] = b'Z'; t[0x2D] = b'X'; t[0x2E] = b'C'; t[0x2F] = b'V'; t[0x30] = b'B';
    t[0x31] = b'N'; t[0x32] = b'M'; t[0x33] = b'<'; t[0x34] = b'>';  t[0x35] = b'?';
    t[0x39] = b' ';
    t
};

pub fn handle_scancode() {
    let scancode = unsafe { inb(KBD_DATA_PORT) };

    if scancode & 0x80 != 0 {
        let released = scancode & 0x7F;
        if released == 0x2A || released == 0x36 {
            unsafe { SHIFT_HELD = false; }
        }
        return;
    }

    if scancode == 0x2A || scancode == 0x36 {
        unsafe { SHIFT_HELD = true; }
        return;
    }

    if (scancode as usize) < 128 {
        let ch = unsafe {
            if SHIFT_HELD { SCANCODE_TO_ASCII_SHIFT[scancode as usize] }
            else { SCANCODE_TO_ASCII[scancode as usize] }
        };

        if ch != 0 {
            let wp = WRITE_POS.load(Ordering::Relaxed);
            let rp = READ_POS.load(Ordering::Relaxed);
            let next_wp = (wp + 1) & (BUF_SIZE - 1);
            if next_wp != rp {
                unsafe { KEY_BUF[wp] = ch; }
                WRITE_POS.store(next_wp, Ordering::Release);
            }
            crate::serial::write_byte(ch);
            crate::vga::print_char(ch as char);
        }
    }
}

pub fn read_char() -> Option<u8> {
    let rp = READ_POS.load(Ordering::Acquire);
    let wp = WRITE_POS.load(Ordering::Relaxed);
    if rp == wp { return None; }
    let ch = unsafe { KEY_BUF[rp] };
    READ_POS.store((rp + 1) & (BUF_SIZE - 1), Ordering::Release);
    Some(ch)
}

pub fn has_input() -> bool {
    READ_POS.load(Ordering::Relaxed) != WRITE_POS.load(Ordering::Relaxed)
}
