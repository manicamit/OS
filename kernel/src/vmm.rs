// kernel/src/vmm.rs

use crate::pmm::PhysicalMemoryManager;

const PAGE_SIZE: usize = 4096;

pub struct PageTableManager<'a> {
    pml4_phys: u64,
    pmm: &'a mut PhysicalMemoryManager,
}

impl<'a> PageTableManager<'a> {
    /// Create VMM - PML4 should be identity-mapped
    pub unsafe fn new(pml4_phys: u64, pmm: &'a mut PhysicalMemoryManager) -> Self {
        Self { pml4_phys, pmm }
    }

    /// Map a single 4 KiB page (works for both lower and higher half)
    pub unsafe fn map_page(&mut self, virt: u64, phys: u64, flags: u64) {
        let pml4_i = ((virt >> 39) & 0x1FF) as usize;
        let pdpt_i = ((virt >> 30) & 0x1FF) as usize;
        let pd_i   = ((virt >> 21) & 0x1FF) as usize;
        let pt_i   = ((virt >> 12) & 0x1FF) as usize;

        // Access PML4 through identity mapping
        let pml4_entries = self.pml4_phys as *mut u64;
        
        // Ensure PDPT exists
        let pdpt_entry_ptr = pml4_entries.add(pml4_i);
        let mut pdpt_entry_val = core::ptr::read_volatile(pdpt_entry_ptr);
        
        if (pdpt_entry_val & 1) == 0 {
            // Allocate and zero new PDPT
            let frame = self.pmm.alloc_frame()
                .expect("Out of memory for page tables");
            
            let table_ptr = frame as *mut u64;
            for i in 0..512 {
                core::ptr::write_volatile(table_ptr.add(i), 0);
            }
            
            pdpt_entry_val = frame | 0x3;
            core::ptr::write_volatile(pdpt_entry_ptr, pdpt_entry_val);
            
            // Flush TLB
            core::arch::asm!(
                "mov rax, cr3",
                "mov cr3, rax",
                out("rax") _,
            );
        }
        
        let pdpt_phys = pdpt_entry_val & 0x000F_FFFF_FFFF_F000;
        let pdpt_entries = pdpt_phys as *mut u64;
        
        // Ensure PD exists
        let pd_entry_ptr = pdpt_entries.add(pdpt_i);
        let mut pd_entry_val = core::ptr::read_volatile(pd_entry_ptr);
        
        if (pd_entry_val & 1) == 0 {
            let frame = self.pmm.alloc_frame()
                .expect("Out of memory for page tables");
            
            let table_ptr = frame as *mut u64;
            for i in 0..512 {
                core::ptr::write_volatile(table_ptr.add(i), 0);
            }
            
            pd_entry_val = frame | 0x3;
            core::ptr::write_volatile(pd_entry_ptr, pd_entry_val);
        }
        
        let pd_phys = pd_entry_val & 0x000F_FFFF_FFFF_F000;
        let pd_entries = pd_phys as *mut u64;
        
        // Ensure PT exists
        let pt_entry_ptr = pd_entries.add(pd_i);
        let mut pt_entry_val = core::ptr::read_volatile(pt_entry_ptr);
        
        if (pt_entry_val & 1) == 0 {
            let frame = self.pmm.alloc_frame()
                .expect("Out of memory for page tables");
            
            let table_ptr = frame as *mut u64;
            for i in 0..512 {
                core::ptr::write_volatile(table_ptr.add(i), 0);
            }
            
            pt_entry_val = frame | 0x3;
            core::ptr::write_volatile(pt_entry_ptr, pt_entry_val);
        }
        
        let pt_phys = pt_entry_val & 0x000F_FFFF_FFFF_F000;
        let pt_entries = pt_phys as *mut u64;

        // Set the final page table entry
        let final_entry_ptr = pt_entries.add(pt_i);
        core::ptr::write_volatile(final_entry_ptr, phys | flags | 0x1);

        // Flush TLB for this specific address
        core::arch::asm!(
            "invlpg [{}]",
            in(reg) virt,
            options(nostack, preserves_flags)
        );
    }

    /// Map a contiguous range of physical memory to virtual memory
    /// All addresses and size must be 4K-aligned
    pub unsafe fn map_range(&mut self, virt_start: u64, phys_start: u64, size: u64, flags: u64) {
        // Align to page boundaries
        let virt_aligned = virt_start & !0xFFF;
        let phys_aligned = phys_start & !0xFFF;
        let size_aligned = ((size + 0xFFF) & !0xFFF);
        
        let num_pages = size_aligned / PAGE_SIZE as u64;
        
        for i in 0..num_pages {
            let virt = virt_aligned + (i * PAGE_SIZE as u64);
            let phys = phys_aligned + (i * PAGE_SIZE as u64);
            self.map_page(virt, phys, flags);
        }
    }
}
