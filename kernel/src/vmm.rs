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
            
            // Invalidate the entire higher-half range for this PML4 entry
            // This forces the CPU to re-walk the page tables
            let test_addr = (pml4_i as u64) << 39;
            let canonical_addr = if test_addr & (1 << 47) != 0 {
                test_addr | 0xFFFF_0000_0000_0000
            } else {
                test_addr
            };
            
            core::arch::asm!(
                "invlpg [{}]",
                in(reg) canonical_addr,
                options(nostack, preserves_flags)
            );
            
            vga::println("    Invalidated higher-half TLB entry");
            
            // Delay
            for _ in 0..(2 * 5_000_000) {
                core::arch::asm!("pause", options(nomem, nostack));
            }
            
            // Also do a full CR3 reload to be absolutely sure
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
        
        // Add memory fence to ensure all writes are complete
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        
        vga::println("  [map_page] Memory fence complete");
        
        // Delay
        for _ in 0..(2 * 5_000_000) {
            core::arch::asm!("pause", options(nomem, nostack));
        }
        
        let pdpt = pdpt_phys as *mut PageTable;
        
        vga::println("  [map_page] Created PDPT pointer");
        
        // Delay
        for _ in 0..(2 * 5_000_000) {
            core::arch::asm!("pause", options(nomem, nostack));
        }
        
        vga::println("  [map_page] About to access PDPT...");
        
        // Delay
        for _ in 0..(2 * 5_000_000) {
            core::arch::asm!("pause", options(nomem, nostack));
        }
        
        // TEST: Try to read the first entry before using it
        vga::println("  [map_page] Test reading PDPT[0]...");
        
        // Delay
        for _ in 0..(2 * 5_000_000) {
            core::arch::asm!("pause", options(nomem, nostack));
        }
        
        let test_val = unsafe {
            let test_ptr = pdpt_phys as *const u64;
            core::ptr::read_volatile(test_ptr)
        };
        
        vga::print("  [map_page] PDPT[0] value: ");
        vga::print_hex(test_val);
        vga::println("");
        
        // Delay
        for _ in 0..(2 * 5_000_000) {
            core::arch::asm!("pause", options(nomem, nostack));
        }
        
        // Ensure PD exists - use raw pointer arithmetic instead of struct dereference
        vga::println("  [map_page] Getting PD entry...");
        
        // Delay
        for _ in 0..(2 * 5_000_000) {
            core::arch::asm!("pause", options(nomem, nostack));
        }
        
        // Access PDPT as u64 array, not as struct
        let pdpt_entries = pdpt_phys as *mut u64;
        let pd_entry_ptr = pdpt_entries.add(pdpt_i);
        let mut pd_entry_val = core::ptr::read_volatile(pd_entry_ptr);
        
        vga::print("  [map_page] PDPT[");
        vga::print_num(pdpt_i as u64);
        vga::print("] = ");
        vga::print_hex(pd_entry_val);
        vga::println("");
        
        // Delay
        for _ in 0..(2 * 5_000_000) {
            core::arch::asm!("pause", options(nomem, nostack));
        }
        
        if (pd_entry_val & 1) == 0 {
            vga::println("    Allocating PD...");
            
            let frame = self.pmm.alloc_frame()
                .expect("Out of memory for page tables");
            
            // Zero it  
            let table_ptr = frame as *mut u64;
            for i in 0..512 {
                core::ptr::write_volatile(table_ptr.add(i), 0);
            }
            
            pd_entry_val = frame | 0x3;
            core::ptr::write_volatile(pd_entry_ptr, pd_entry_val);
            
            vga::println("    PD allocated");
        }
        
        let pd_phys = pd_entry_val & 0x000F_FFFF_FFFF_F000;
        let pd_entries = pd_phys as *mut u64;
        
        // Ensure PT exists - use raw pointer arithmetic
        vga::println("  [map_page] Getting PT entry...");
        
        let pt_entry_ptr = pd_entries.add(pd_i);
        let mut pt_entry_val = core::ptr::read_volatile(pt_entry_ptr);
        
        if (pt_entry_val & 1) == 0 {
            vga::println("    Allocating PT...");
            
            let frame = self.pmm.alloc_frame()
                .expect("Out of memory for page tables");
                
            // Zero it
            let table_ptr = frame as *mut u64;
            for i in 0..512 {
                core::ptr::write_volatile(table_ptr.add(i), 0);
            }
            
            pt_entry_val = frame | 0x3;
            core::ptr::write_volatile(pt_entry_ptr, pt_entry_val);
            
            vga::println("    PT allocated");
        }
        
        let pt_phys = pt_entry_val & 0x000F_FFFF_FFFF_F000;
        let pt_entries = pt_phys as *mut u64;

        // Set the final page table entry
        vga::println("  [map_page] Setting final PT entry...");
        
        let final_entry_ptr = pt_entries.add(pt_i);
        core::ptr::write_volatile(final_entry_ptr, phys | flags | 0x1);
        
        vga::println("  [map_page] PT entry set");

        // Flush TLB entry
        core::arch::asm!(
            "invlpg [{}]",
            in(reg) virt,
            options(nostack, preserves_flags)
        );
    }
}
