// kernel/src/vmm.rs

use crate::pmm::PhysicalMemoryManager;

const PAGE_SIZE: usize = 4096;
const RECURSIVE_SLOT: usize = 510;

#[repr(C)]
struct PageTable {
    entries: [u64; 512],
}

pub struct PageTableManager<'a> {
    pml4_phys: u64,
    pmm: &'a mut PhysicalMemoryManager,
}

impl<'a> PageTableManager<'a> {
    /// Create VMM - recursive mapping must already be installed at PML4[510]
    pub unsafe fn new(pml4_phys: u64, pmm: &'a mut PhysicalMemoryManager) -> Self {
        Self { pml4_phys, pmm }
    }

    /// Map a single 4 KiB page (works for both lower and higher half)
    pub unsafe fn map_page(&mut self, virt: u64, phys: u64, flags: u64) {
        use crate::vga;
        
        vga::println("  [map_page] Entered");
        
        // Delay
        for _ in 0..(2 * 5_000_000) {
            core::arch::asm!("pause", options(nomem, nostack));
        }
        
        let pml4_i = ((virt >> 39) & 0x1FF) as usize;
        let pdpt_i = ((virt >> 30) & 0x1FF) as usize;
        let pd_i   = ((virt >> 21) & 0x1FF) as usize;
        let pt_i   = ((virt >> 12) & 0x1FF) as usize;

        vga::print("  [map_page] Indices: PML4[");
        vga::print_num(pml4_i as u64);
        vga::print("] PDPT[");
        vga::print_num(pdpt_i as u64);
        vga::print("] PD[");
        vga::print_num(pd_i as u64);
        vga::print("] PT[");
        vga::print_num(pt_i as u64);
        vga::println("]");

        // Delay
        for _ in 0..(2 * 5_000_000) {
            core::arch::asm!("pause", options(nomem, nostack));
        }

        // Access PML4 directly through identity mapping (bootloader mapped first 4GB)
        // The PML4 is at physical 0x2000, which is identity-mapped
        let pml4 = self.pml4_phys as *mut PageTable;
        
        vga::println("  [map_page] Accessing PML4...");
        
        // Delay
        for _ in 0..(2 * 5_000_000) {
            core::arch::asm!("pause", options(nomem, nostack));
        }
        
        // Ensure PDPT exists for this PML4 entry
        let pdpt_entry = &mut (*pml4).entries[pml4_i];
        
        vga::println("  [map_page] Got PML4 entry");
        
        // Delay
        for _ in 0..(2 * 5_000_000) {
            core::arch::asm!("pause", options(nomem, nostack));
        }
        
        if (*pdpt_entry & 1) == 0 {
            vga::println("    Allocating PDPT...");
            
            // Delay
            for _ in 0..(2 * 5_000_000) {
                core::arch::asm!("pause", options(nomem, nostack));
            }
            
            // Allocate new PDPT
            let frame = self.pmm.alloc_frame()
                .expect("Out of memory for page tables");
                
            vga::print("    PDPT frame: ");
            vga::print_hex(frame);
            vga::println("");
            
            // Delay
            for _ in 0..(2 * 5_000_000) {
                core::arch::asm!("pause", options(nomem, nostack));
            }
            
            vga::println("    Zeroing PDPT...");
            
            // Delay
            for _ in 0..(2 * 5_000_000) {
                core::arch::asm!("pause", options(nomem, nostack));
            }
            
            // Zero the new table - use volatile writes
            let table_ptr = frame as *mut u64;
            for i in 0..512 {
                core::ptr::write_volatile(table_ptr.add(i), 0);
            }
            
            vga::println("    PDPT zeroed");
            
            // Delay
            for _ in 0..(2 * 5_000_000) {
                core::arch::asm!("pause", options(nomem, nostack));
            }
            
            *pdpt_entry = frame | 0x3;
            
            vga::println("    PDPT entry set");
            
            // Delay
            for _ in 0..(2 * 5_000_000) {
                core::arch::asm!("pause", options(nomem, nostack));
            }
            
            // Flush TLB for the PML4 entry itself
            core::arch::asm!(
                "mov rax, cr3",
                "mov cr3, rax",
                out("rax") _,
            );
            
            vga::println("    TLB flushed");
            
            // Delay
            for _ in 0..(2 * 5_000_000) {
                core::arch::asm!("pause", options(nomem, nostack));
            }
        }
        
        let pdpt_phys = *pdpt_entry & 0x000F_FFFF_FFFF_F000;
        
        vga::print("  [map_page] PDPT physical: ");
        vga::print_hex(pdpt_phys);
        vga::println("");
        
        // Delay
        for _ in 0..(2 * 5_000_000) {
            core::arch::asm!("pause", options(nomem, nostack));
        }
        
        let pdpt = pdpt_phys as *mut PageTable;
        
        vga::println("  [map_page] Accessing PDPT...");
        
        // Delay
        for _ in 0..(2 * 5_000_000) {
            core::arch::asm!("pause", options(nomem, nostack));
        }
        
        // Ensure PD exists
        let pd_entry = &mut (*pdpt).entries[pdpt_i];
        if (*pd_entry & 1) == 0 {
            let frame = self.pmm.alloc_frame()
                .expect("Out of memory for page tables");
            core::ptr::write_bytes(frame as *mut u8, 0, PAGE_SIZE);
            *pd_entry = frame | 0x3;
        }
        
        let pd_phys = *pd_entry & 0x000F_FFFF_FFFF_F000;
        let pd = pd_phys as *mut PageTable;
        
        // Ensure PT exists
        let pt_entry = &mut (*pd).entries[pd_i];
        if (*pt_entry & 1) == 0 {
            let frame = self.pmm.alloc_frame()
                .expect("Out of memory for page tables");
            core::ptr::write_bytes(frame as *mut u8, 0, PAGE_SIZE);
            *pt_entry = frame | 0x3;
        }
        
        let pt_phys = *pt_entry & 0x000F_FFFF_FFFF_F000;
        let pt = pt_phys as *mut PageTable;

        // Set the final page table entry
        (*pt).entries[pt_i] = phys | flags | 0x1;

        // Flush TLB entry
        core::arch::asm!(
            "invlpg [{}]",
            in(reg) virt,
            options(nostack, preserves_flags)
        );
    }
}
