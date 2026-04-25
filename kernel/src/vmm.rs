use crate::pmm::PhysicalMemoryManager;

const PAGE_SIZE: usize = 4096;

pub struct PageTableManager<'a> {
    pml4_phys: u64,
    pmm: &'a mut PhysicalMemoryManager,
}

impl<'a> PageTableManager<'a> {
    pub unsafe fn new(pml4_phys: u64, pmm: &'a mut PhysicalMemoryManager) -> Self {
        Self { pml4_phys, pmm }
    }

    pub unsafe fn map_page(&mut self, virt: u64, phys: u64, flags: u64) {
        let pml4_i = ((virt >> 39) & 0x1FF) as usize;
        let pdpt_i = ((virt >> 30) & 0x1FF) as usize;
        let pd_i   = ((virt >> 21) & 0x1FF) as usize;
        let pt_i   = ((virt >> 12) & 0x1FF) as usize;

        let pml4_entries = self.pml4_phys as *mut u64;

        let pdpt_entry_ptr = pml4_entries.add(pml4_i);
        let mut pdpt_entry_val = core::ptr::read_volatile(pdpt_entry_ptr);
        if (pdpt_entry_val & 1) == 0 {
            let frame = self.pmm.alloc_frame().expect("OOM for page tables");
            let table_ptr = frame as *mut u64;
            for i in 0..512 { core::ptr::write_volatile(table_ptr.add(i), 0); }
            pdpt_entry_val = frame | 0x3;
            core::ptr::write_volatile(pdpt_entry_ptr, pdpt_entry_val);
            core::arch::asm!("mov rax, cr3", "mov cr3, rax", out("rax") _);
        }

        let pdpt_phys = pdpt_entry_val & 0x000F_FFFF_FFFF_F000;
        let pd_entry_ptr = (pdpt_phys as *mut u64).add(pdpt_i);
        let mut pd_entry_val = core::ptr::read_volatile(pd_entry_ptr);
        if (pd_entry_val & 1) == 0 {
            let frame = self.pmm.alloc_frame().expect("OOM for page tables");
            let table_ptr = frame as *mut u64;
            for i in 0..512 { core::ptr::write_volatile(table_ptr.add(i), 0); }
            pd_entry_val = frame | 0x3;
            core::ptr::write_volatile(pd_entry_ptr, pd_entry_val);
        }

        let pd_phys = pd_entry_val & 0x000F_FFFF_FFFF_F000;
        let pt_entry_ptr = (pd_phys as *mut u64).add(pd_i);
        let mut pt_entry_val = core::ptr::read_volatile(pt_entry_ptr);
        if (pt_entry_val & 1) == 0 {
            let frame = self.pmm.alloc_frame().expect("OOM for page tables");
            let table_ptr = frame as *mut u64;
            for i in 0..512 { core::ptr::write_volatile(table_ptr.add(i), 0); }
            pt_entry_val = frame | 0x3;
            core::ptr::write_volatile(pt_entry_ptr, pt_entry_val);
        }

        let pt_phys = pt_entry_val & 0x000F_FFFF_FFFF_F000;
        let final_entry_ptr = (pt_phys as *mut u64).add(pt_i);
        core::ptr::write_volatile(final_entry_ptr, phys | flags | 0x1);
        core::arch::asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags));
    }

    pub unsafe fn map_range(&mut self, virt_start: u64, phys_start: u64, size: u64, flags: u64) {
        let virt_aligned = virt_start & !0xFFF;
        let phys_aligned = phys_start & !0xFFF;
        let size_aligned = (size + 0xFFF) & !0xFFF;
        let num_pages = size_aligned / PAGE_SIZE as u64;
        for i in 0..num_pages {
            self.map_page(virt_aligned + i * PAGE_SIZE as u64, phys_aligned + i * PAGE_SIZE as u64, flags);
        }
    }
}
