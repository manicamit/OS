const PIT_CHANNEL0: u16 = 0x40;
const PIT_COMMAND: u16  = 0x43;
const PIT_BASE_FREQ: u32 = 1_193_182;
const TICK_HZ: u32 = 100;

static mut TICKS: u64 = 0;

#[inline(always)]
unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
}

pub fn init() {
    let divisor = (PIT_BASE_FREQ / TICK_HZ) as u16;
    unsafe {
        outb(PIT_COMMAND, 0x36);
        outb(PIT_CHANNEL0, (divisor & 0xFF) as u8);
        outb(PIT_CHANNEL0, (divisor >> 8) as u8);
    }
    crate::serial::print("PIT: ");
    crate::serial::print_num(TICK_HZ as u64);
    crate::serial::println(" Hz");
}

#[inline(never)]
pub fn tick() {
    unsafe {
        let t = core::ptr::read_volatile(core::ptr::addr_of!(TICKS));
        core::ptr::write_volatile(core::ptr::addr_of_mut!(TICKS), t + 1);
    }
}

pub fn get_ticks() -> u64 {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(TICKS)) }
}

pub fn sleep_ms(ms: u64) {
    let target_ticks = ms / (1000 / TICK_HZ as u64);
    let start = get_ticks();
    while get_ticks() - start < target_ticks {
        unsafe { core::arch::asm!("hlt"); }
    }
}
