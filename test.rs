#![feature(asm_sym)]
extern "C" { static x: u8; }
fn main() {
    let mut p: usize;
    unsafe { std::arch::asm!("lea {0}(%rip), {1}", sym x, out(reg) p, options(att_syntax)); }
}
