use core::mem::size_of;

#[repr(C, packed)]
struct GdtDescriptor {
    size: u16,
    offset: u64,
}

const GDT_ENTRIES: usize = 9;

#[repr(C, align(8))]
pub struct Gdt {
    entries: [u64; GDT_ENTRIES],
}

#[repr(C, packed)]
pub struct Tss {
    _reserved0: u32,
    pub rsp0: u64,
    pub rsp1: u64,
    pub rsp2: u64,
    _reserved1: u64,
    pub ist1: u64,
    pub ist2: u64,
    pub ist3: u64,
    pub ist4: u64,
    pub ist5: u64,
    pub ist6: u64,
    pub ist7: u64,
    _reserved2: u64,
    _reserved3: u16,
    pub iomap_base: u16,
}

impl Tss {
    pub const fn new() -> Self {
        Tss {
            _reserved0: 0, rsp0: 0, rsp1: 0, rsp2: 0,
            _reserved1: 0,
            ist1: 0, ist2: 0, ist3: 0, ist4: 0,
            ist5: 0, ist6: 0, ist7: 0,
            _reserved2: 0, _reserved3: 0,
            iomap_base: size_of::<Tss>() as u16,
        }
    }
}

pub static mut TSS: Tss = Tss::new();

impl Gdt {
    pub const fn new() -> Self {
        let mut entries = [0u64; GDT_ENTRIES];
        entries[3] = (1 << 43) | (1 << 44) | (1 << 47) | (1 << 53);
        entries[4] = (1 << 41) | (1 << 44) | (1 << 47);
        Gdt { entries }
    }

    fn install_tss(&mut self, tss: &Tss) {
        let tss_addr = tss as *const Tss as u64;
        let tss_len = (size_of::<Tss>() - 1) as u64;

        let low: u64 =
            (tss_len & 0xFFFF)
            | ((tss_addr & 0xFFFF) << 16)
            | (((tss_addr >> 16) & 0xFF) << 32)
            | (0x89u64 << 40)
            | (((tss_len >> 16) & 0xF) << 48)
            | (((tss_addr >> 24) & 0xFF) << 56);

        let high: u64 = tss_addr >> 32;

        self.entries[5] = low;
        self.entries[6] = high;
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
            core::arch::asm!("ltr {0:x}", in(reg) 0x28u16, options(nostack, nomem));
        }
    }
}

pub static mut GDT: Gdt = Gdt::new();

pub fn init() {
    unsafe {
        let gdt = &mut *core::ptr::addr_of_mut!(GDT);
        let tss = &*core::ptr::addr_of!(TSS);
        gdt.install_tss(tss);
        (*core::ptr::addr_of!(GDT)).load();
    }
}
