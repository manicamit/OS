use core::mem::size_of;

#[repr(C, packed)]
struct GdtDescriptor {
    size: u16,
    offset: u64,
}

const GDT_ENTRIES: usize = 7;

#[repr(C, align(8))]
pub struct Gdt {
    entries: [u64; GDT_ENTRIES],
}

impl Gdt {
    pub const fn new() -> Self {
        let mut entries = [0; GDT_ENTRIES];
        entries[0] = 0;
        entries[1] = 0;
        entries[2] = 0;
        entries[3] = (1 << 43) | (1 << 44) | (1 << 47) | (1 << 53); // 64-bit code (0x18)
        entries[4] = (1 << 41) | (1 << 44) | (1 << 47);              // 64-bit data (0x20)
        Gdt { entries }
    }

    pub fn load(&'static self) {
        let descriptor = GdtDescriptor {
            size: (size_of::<Gdt>() - 1) as u16,
            offset: self as *const _ as u64,
        };
        unsafe {
            core::arch::asm!("lgdt [{}]", in(reg) &descriptor, options(readonly, nostack));
            core::arch::asm!(
                "push 0x18",
                "lea rax, [rip + 2f]",
                "push rax",
                "retfq",
                "2:",
                "mov ax, 0x20",
                "mov ds, ax", "mov es, ax", "mov fs, ax", "mov gs, ax", "mov ss, ax",
                out("rax") _
            );
        }
    }
}

pub static mut GDT: Gdt = Gdt::new();

pub fn init() {
    unsafe { (*core::ptr::addr_of!(GDT)).load(); }
}
