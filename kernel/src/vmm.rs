use crate::pmm::PhysicalMemoryManager;

const PRESENT: u64 = 1;
const WRITABLE: u64 = 1 << 1;

pub struct PageTableManager<'a> {
    pml4: *mut u64,
    pmm: &'a mut PhysicalMemoryManager,
}

impl<'a> PageTableManager<'a> {
    pub unsafe fn new(cr3: u64, pmm: &'a mut PhysicalMemoryManager) -> Self {
        Self {
            pml4: cr3 as *mut u64,
            pmm,
        }
    }

    pub fn map_page(&mut self, virt: u64, phys: u64) {
        let pml4_i = (virt >> 39) & 0x1FF;
        let pdpt_i = (virt >> 30) & 0x1FF;
        let pd_i   = (virt >> 21) & 0x1FF;
        let pt_i   = (virt >> 12) & 0x1FF;

        let pdpt = self.get_or_alloc(self.pml4, pml4_i);
        let pd   = self.get_or_alloc(pdpt, pdpt_i);
        let pt   = self.get_or_alloc(pd, pd_i);

        unsafe {
            *pt.add(pt_i as usize) = phys | PRESENT | WRITABLE;
        }
    }

    fn get_or_alloc(&mut self, table: *mut u64, index: u64) -> *mut u64 {
        unsafe {
            let entry = table.add(index as usize);
            if *entry & PRESENT == 0 {
                let frame = self.pmm.alloc_frame().expect("Out of physical memory");
                core::ptr::write_bytes(frame as *mut u8, 0, 4096);
                *entry = frame | PRESENT | WRITABLE;
            }
            (*entry & 0x000F_FFFF_FFFF_F000) as *mut u64
        }
    }
}
