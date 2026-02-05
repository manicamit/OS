// kernel/src/vmm.rs

use crate::pmm::PhysicalMemoryManager;

const PAGE_SIZE: usize = 4096;

#[repr(C)]
struct PageTable {
    entries: [u64; 512],
}

pub struct PageTableManager<'a> {
    pml4_phys: u64,
    pmm: &'a mut PhysicalMemoryManager,
}

impl<'a> PageTableManager<'a> {
    pub unsafe fn new(pml4_phys: u64, pmm: &'a mut PhysicalMemoryManager) -> Self {
        Self { pml4_phys, pmm }
    }

    pub unsafe fn map_page(&mut self, virt: u64, phys: u64, flags: u64) {
        let pml4_i = (virt >> 39) & 0x1FF;
        let pdpt_i = (virt >> 30) & 0x1FF;
        let pd_i   = (virt >> 21) & 0x1FF;
        let pt_i   = (virt >> 12) & 0x1FF;

        let pdpt = self.get_or_alloc(self.pml4_phys, pml4_i);
        let pd   = self.get_or_alloc(pdpt, pdpt_i);
        let pt   = self.get_or_alloc(pd, pd_i);

        let pt_ptr = pt as *mut PageTable;
        (*pt_ptr).entries[pt_i as usize] = phys | flags | 0x1;

        core::arch::asm!("invlpg [{}]", in(reg) virt, options(nostack));
    }

    unsafe fn get_or_alloc(&mut self, parent_phys: u64, index: u64) -> u64 {
        let parent = parent_phys as *mut PageTable;
        let entry = &mut (*parent).entries[index as usize];

        if (*entry & 1) == 0 {
            let new_table = self.pmm.alloc_frame()
                .expect("Out of memory for page tables");

            core::ptr::write_bytes(new_table as *mut u8, 0, PAGE_SIZE);
            *entry = new_table | 0x3;
            new_table
        } else {
            *entry & 0x000F_FFFF_FFFF_F000
        }
    }
}
