// kernel/src/gdt.rs
//! Global Descriptor Table setup

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
        
        // Entry 0: Null descriptor
        entries[0] = 0;
        
        // Entry 1: 16-bit code (not used in long mode but keeping layout)
        entries[1] = 0;
        // Entry 2: 16-bit data
        entries[2] = 0;
        
        // Entry 3: 64-bit Code segment (Selector 0x18)
        // DPL=0, Present=1, L=1 (64-bit), D=0, Readable code segment
        entries[3] = (1 << 43) | (1 << 44) | (1 << 47) | (1 << 53);
        
        // Entry 4: 64-bit Data segment (Selector 0x20)
        // DPL=0, Present=1, Writable data segment
        entries[4] = (1 << 41) | (1 << 44) | (1 << 47);
        
        Gdt { entries }
    }
    
    pub fn load(&'static self) {
        let descriptor = GdtDescriptor {
            size: (size_of::<Gdt>() - 1) as u16,
            offset: self as *const _ as u64,
        };
        
        unsafe {
            // Load the new GDT
            core::arch::asm!("lgdt [{}]", in(reg) &descriptor, options(readonly, nostack));
            
            // Reload CS register using a far return trick
            core::arch::asm!(
                "push 0x18",      // New CS selector (entry 3 * 8 = 0x18)
                "lea rax, [rip + 2f]",
                "push rax",
                "retfq",          // equivalent to lretq in some assemblers
                "2:",
                "mov ax, 0x20",   // New DS selector (entry 4 * 8 = 0x20)
                "mov ds, ax",
                "mov es, ax",
                "mov fs, ax",
                "mov gs, ax",
                "mov ss, ax",
                out("rax") _
            );
        }
    }
}

pub static mut GDT: Gdt = Gdt::new();

pub fn init() {
    unsafe {
        (*core::ptr::addr_of!(GDT)).load();
    }
}
