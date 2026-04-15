#![no_std]
#![feature(asm_sym)]
extern "C" { static x: u8; }
fn get_x() -> usize {
    let mut p: usize;
    unsafe { core::arch::asm!("lea {0}(%rip), {1}", sym x, out(reg) p, options(att_syntax)); }
    p
}
