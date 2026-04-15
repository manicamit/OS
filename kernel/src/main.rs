#![no_std]
#![no_main]

extern crate alloc;

mod idt;
mod vga;
mod pmm;
mod vmm;
mod serial;
mod allocator;

use core::panic::PanicInfo;
use pmm::PhysicalMemoryManager;
use crate::vmm::PageTableManager;

pub const KERNEL_VIRT_BASE: u64 = 0xFFFF_8000_0000_0000;

// Store PMM state for use after stack switch
// (PMM is consumed by VMM, but we need to allocate heap frames later)
static mut PMM_CURRENT: u64 = 0;
static mut PMM_END: u64 = 0;

// Static kernel stack in .bss (16KB) - will be in higher-half after kernel mapping
#[repr(C, align(16))]
struct StaticStack {
    data: [u8; 16384],
}

static mut STATIC_KERNEL_STACK: StaticStack = StaticStack { data: [0; 16384] };

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    vga::println("KERNEL PANIC!");
    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct E820Entry {
    pub base: u64,
    pub length: u64,
    pub kind: u32,
    pub _ext: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct RsdpV1 {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_address: u32,
}

fn e820_type_str(kind: u32) -> &'static str {
    match kind {
        1 => "USABLE",
        2 => "RESERVED", 
        3 => "ACPI",
        4 => "ACPI_NVS",
        5 => "BAD",
        _ => "UNKNOWN",
    }
}

// Separate function to display E820
fn display_e820(entries: &[E820Entry]) {
    vga::println("Memory Map:");
    for (i, e) in entries.iter().enumerate() {
        if e.base == 0 && e.length == 0 && e.kind == 0 {
            continue;
        }
        vga::print("  ");
        vga::print_num(i as u64);
        vga::print(": ");
        vga::print_hex(e.base);
        vga::print(" - ");
        vga::print_hex(e.base + e.length);
        vga::print(" ");
        vga::println(e820_type_str(e.kind));
    }
}

// Separate function for RSDP search
fn find_and_display_rsdp() {
    vga::print("Searching for ACPI RSDP... ");
    
    // Check EBDA
    let ebda_seg = unsafe { *(0x40E as *const u16) } as u64;
    let ebda_addr = ebda_seg << 4;
    
    let rsdp_addr = if ebda_addr != 0 && ebda_addr < 0x100000 {
        search_rsdp_range(ebda_addr, 1024)
            .or_else(|| search_rsdp_range(0xE0000, 0x20000))
    } else {
        search_rsdp_range(0xE0000, 0x20000)
    };
    
    match rsdp_addr {
        Some(addr) => {
            vga::println("Found!");
            vga::print("  RSDP at: ");
            vga::print_hex(addr);
            vga::println("");
            
            unsafe {
                let rsdp = &*(addr as *const RsdpV1);
                vga::print("  Revision: ");
                vga::print_num(rsdp.revision as u64);
                if rsdp.revision == 0 {
                    vga::println(" (ACPI 1.0)");
                } else {
                    vga::println(" (ACPI 2.0+)");
                }
            }
        }
        None => {
            vga::println("Not found!");
        }
    }
}

fn search_rsdp_range(start: u64, length: u64) -> Option<u64> {
    let mut addr = start;
    let end = start + length;
    
    while addr < end {
        unsafe {
            let sig = (addr as *const u64).read_volatile();
            if sig == 0x2052545020445352 {
                let bytes = core::slice::from_raw_parts(addr as *const u8, 20);
                let sum: u8 = bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
                if sum == 0 {
                    return Some(addr);
                }
            }
        }
        addr += 16;
    }
    None
}

// Separate function for PMM display
fn test_pmm(pmm: &mut PhysicalMemoryManager) {
    vga::println("Allocating physical frames:");
    for i in 0..5 {
        match pmm.alloc_frame() {
            Some(frame) => {
                vga::print("  Frame ");
                vga::print_num(i);
                vga::print(": ");
                vga::print_hex(frame);
                vga::println("");
            }
            None => {
                vga::println("  OUT OF MEMORY");
            }
        }
    }
}

// Separate function for VMM tests - uses pre-allocated frames
fn test_vmm_with_frames(vmm: &mut PageTableManager, phys_low: u64, phys_high: u64) {
    // Lower-half test
    vga::println("=== VMM Lower-Half Test ===");
    let virt_low = 0x0000_0000_0040_0000;
    
    unsafe {
        vmm.map_page(virt_low, phys_low, 0x3);
        *(virt_low as *mut u64) = 0xCAFEBABEDEADBEEF;
        let val = *(virt_low as *const u64);
        
        if val == 0xCAFEBABEDEADBEEF {
            vga::println("Lower-half: SUCCESS");
        } else {
            vga::println("Lower-half: FAILED");
        }
    }
    
    // Higher-half test
    vga::println("");
    vga::println("=== VMM Higher-Half Test ===");
    let virt_high = KERNEL_VIRT_BASE;
    
    unsafe {
        vmm.map_page(virt_high, phys_high, 0x3);
        *(virt_high as *mut u64) = 0xDEADC0DEBADC0FFE;
        let val = *(virt_high as *const u64);
        
        if val == 0xDEADC0DEBADC0FFE {
            vga::println("Higher-half: SUCCESS");
        } else {
            vga::println("Higher-half: FAILED");
        }
    }
}

// Separate function for kernel mapping
fn map_kernel(vmm: &mut PageTableManager) {
    vga::println("");
    vga::println("=== Mapping Kernel to Higher-Half ===");
    
    let kernel_phys_start = 0x100000u64;
    let kernel_phys_end = 0x200000u64;
    let kernel_size = kernel_phys_end - kernel_phys_start;
    
    vga::print("Kernel physical: ");
    vga::print_hex(kernel_phys_start);
    vga::print(" - ");
    vga::print_hex(kernel_phys_end);
    vga::println("");
    
    vga::print("Kernel size: ");
    vga::print_num(kernel_size / 1024);
    vga::println(" KB");
    
    let kernel_virt = KERNEL_VIRT_BASE + kernel_phys_start;
    
    vga::print("Mapping to: ");
    vga::print_hex(kernel_virt);
    vga::println("");
    
    unsafe {
        vmm.map_range(kernel_virt, kernel_phys_start, kernel_size, 0x3);
    }
    
    vga::println("Kernel mapped!");
    for _ in 0..(2 * 5_000_000) {
            unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
        }
    
    vga::println("");
    vga::println("=== Verification ===");
    
    let h = unsafe { *(kernel_virt as *const u64) };
    let i = unsafe { *(kernel_phys_start as *const u64) };
    
    vga::print("Higher-half: ");
    vga::print_hex(h);
    vga::println("");
    vga::print("Identity:    ");
    vga::print_hex(i);
    vga::println("");
    
    if h == i {
        vga::println("Kernel mapping: SUCCESS!");
    } else {
        vga::println("Kernel mapping: FAILED!");
    }

    for _ in 0..(2 * 5_000_000) {
            unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
        }
}

/// Map VGA physical address to higher-half
/// This directly manipulates page tables since we can't use VMM (it borrows pmm)
unsafe fn map_vga_to_higher_half() {
    let virt = vga::VGA_HIGHER_HALF;
    let phys = vga::VGA_PHYS;
    
    let pml4_i = ((virt >> 39) & 0x1FF) as usize;
    let pdpt_i = ((virt >> 30) & 0x1FF) as usize;
    let pd_i   = ((virt >> 21) & 0x1FF) as usize;
    let pt_i   = ((virt >> 12) & 0x1FF) as usize;
    
    let pml4 = 0x2000 as *mut u64;
    
    // Get or create PDPT
    let pdpt_entry_ptr = pml4.add(pml4_i);
    let pdpt_entry = core::ptr::read_volatile(pdpt_entry_ptr);
    
    let pdpt_phys = if (pdpt_entry & 1) == 0 {
        // Need to allocate - use a fixed address in low memory for simplicity
        // This is safe since we're in early boot and have identity mapping
        let frame = 0x8000u64; // Use E820 buffer area (we're done with it)
        let table_ptr = frame as *mut u64;
        for i in 0..512 {
            core::ptr::write_volatile(table_ptr.add(i), 0);
        }
        core::ptr::write_volatile(pdpt_entry_ptr, frame | 0x3);
        frame
    } else {
        pdpt_entry & 0x000F_FFFF_FFFF_F000
    };
    
    let pdpt = pdpt_phys as *mut u64;
    
    // Get or create PD
    let pd_entry_ptr = pdpt.add(pdpt_i);
    let pd_entry = core::ptr::read_volatile(pd_entry_ptr);
    
    let pd_phys = if (pd_entry & 1) == 0 {
        let frame = 0x9000u64;
        let table_ptr = frame as *mut u64;
        for i in 0..512 {
            core::ptr::write_volatile(table_ptr.add(i), 0);
        }
        core::ptr::write_volatile(pd_entry_ptr, frame | 0x3);
        frame
    } else {
        pd_entry & 0x000F_FFFF_FFFF_F000
    };
    
    let pd = pd_phys as *mut u64;
    
    // Get or create PT
    let pt_entry_ptr = pd.add(pd_i);
    let pt_entry = core::ptr::read_volatile(pt_entry_ptr);
    
    let pt_phys = if (pt_entry & 1) == 0 {
        let frame = 0xA000u64;
        let table_ptr = frame as *mut u64;
        for i in 0..512 {
            core::ptr::write_volatile(table_ptr.add(i), 0);
        }
        core::ptr::write_volatile(pt_entry_ptr, frame | 0x3);
        frame
    } else {
        pt_entry & 0x000F_FFFF_FFFF_F000
    };
    
    let pt = pt_phys as *mut u64;
    
    // Set final PT entry
    let final_entry_ptr = pt.add(pt_i);
    core::ptr::write_volatile(final_entry_ptr, phys | 0x3);
    
    // Flush TLB
    core::arch::asm!(
        "invlpg [{}]",
        in(reg) virt,
        options(nostack, preserves_flags)
    );
}

#[no_mangle]
pub extern "C" fn _start(e820_ptr: *const E820Entry, e820_count: usize) -> ! {
    // Delay 2 seconds before starting
    for _ in 0..(2 * 5_000_000) {
        unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
    }
    
    vga::init();
    serial::init();
    idt::init();
    
    vga::println("=== Kernel Boot ===");
    vga::println("");
    
    let entries = unsafe {
        core::slice::from_raw_parts(e820_ptr, e820_count)
    };
    
    vga::print("E820 entries: ");
    vga::print_num(e820_count as u64);
    vga::println("");
    vga::println("");
    
    // Display E820
    display_e820(entries);
    vga::println("");
    
    // Find ACPI
    find_and_display_rsdp();
    vga::println("");
    
    // Initialize PMM
    vga::println("Initializing PMM...");
    let mut pmm = match PhysicalMemoryManager::new(entries) {
        Some(p) => p,
        None => {
            vga::println("ERROR: No RAM!");
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
    };
    vga::println("PMM initialized");
    vga::println("");
    
    // Test PMM
    test_pmm(&mut pmm);
    vga::println("");
    
    // Setup VMM
    vga::println("=== VMM Initialization ===");
    let pml4_phys = 0x2000u64;
    
    unsafe {
        let pml4 = pml4_phys as *mut u64;
        pml4.add(510).write_volatile(pml4_phys | 0x3);
        core::arch::asm!("mov rax, cr3; mov cr3, rax", out("rax") _);
    }
    vga::println("Recursive mapping installed");
    
    // Allocate frames BEFORE creating VMM
    let phys_low = pmm.alloc_frame().expect("No frames");
    let phys_high = pmm.alloc_frame().expect("No frames");
    
    // Save PMM state before it gets consumed by VMM
    let (pmm_cur, pmm_end) = pmm.get_state();
    unsafe {
        PMM_CURRENT = pmm_cur;
        PMM_END = pmm_end;
    }
    
    let mut vmm = unsafe {
        PageTableManager::new(pml4_phys, &mut pmm)
    };
    vga::println("VMM created");
    vga::println("");
    
    // Test VMM (pass the pre-allocated frames)
    test_vmm_with_frames(&mut vmm, phys_low, phys_high);
    vga::println("");
    
    // Map kernel
    map_kernel(&mut vmm);
    
    vga::println("");
    vga::println("=== Phase 2: Jump to Higher-Half ===");
    vga::println("(Using current stack - no new stack allocated)");
    vga::println("");
    
    // Get current stack pointer
    let current_rsp: u64;
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) current_rsp);
    }
    
    vga::print("Current RSP: ");
    vga::print_hex(current_rsp);
    vga::println("");
    
    // Calculate jump address
    let target_phys = higher_half_main as u64;
    let offset = target_phys - 0x100000;
    let target_virt = KERNEL_VIRT_BASE + 0x100000 + offset;
    
    vga::print("Jump to: ");
    vga::print_hex(target_virt);
    vga::println("");
    vga::println("Jumping NOW...");
    vga::println("");
    
    unsafe {
        core::arch::asm!(
            "jmp {target}",
            target = in(reg) target_virt,
            options(noreturn)
        );
    }
}

// This function runs in higher-half
#[no_mangle]
extern "C" fn higher_half_main() -> ! {
    vga::println("================================");
    vga::println("=== HIGHER-HALF SUCCESS! ===");
    vga::println("================================");
    vga::println("");
    
    let rip: u64;
    unsafe {
        core::arch::asm!("lea {}, [rip]", out(reg) rip);
    }
    
    vga::print("RIP: ");
    vga::print_hex(rip);
    vga::println("");
    
    if rip >= KERNEL_VIRT_BASE {
        vga::println("Running in higher-half: SUCCESS!");
    }
    
    vga::println("");
    vga::println("=== Phase 3: Switch Stack & Remove Identity ===");
    
    // Get the address of the static stack
    // Rust will give us the higher-half address since we're running in higher-half
   // Get the address of the static stack (NO &T!)
    let stack_higher = unsafe {
        core::ptr::addr_of_mut!(STATIC_KERNEL_STACK.data) as u64
    };
    let new_stack_top = stack_higher + 16384;
    
    vga::print("Stack at: ");
    vga::print_hex(stack_higher);
    vga::println("");
    for _ in 0..(2 * 5_000_000) {
            unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
        }
    
    vga::print("New stack top: ");
    vga::print_hex(new_stack_top);
    vga::println("");
    for _ in 0..(2 * 5_000_000) {
            unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
        }
    
    // Verify it's in higher-half
    if stack_higher < KERNEL_VIRT_BASE {
        vga::println("ERROR: Stack not in higher-half!");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }
    for _ in 0..(2 * 5_000_000) {
            unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
        }
    
    // Verify we can access it
    vga::println("Verifying stack is accessible...");
    unsafe {
        let test_ptr = stack_higher as *mut u64;
        core::ptr::write_volatile(test_ptr, 0xDEADBEEF);
        let test = core::ptr::read_volatile(test_ptr);
        if test == 0xDEADBEEF {
            vga::println("Stack is accessible: OK");
        } else {
            vga::println("Stack test FAILED!");
            loop { core::arch::asm!("hlt"); }
        }
    }
    for _ in 0..(2 * 5_000_000) {
            unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
        }
    
    vga::println("");
    vga::println("Switching to higher-half stack...");

     for _ in 0..(2 * 5_000_000) {
            unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
        }
    
    // Switch stack and continue
    unsafe {
        core::arch::asm!(
            "mov rsp, {stack}",
            "call {func}",
            stack = in(reg) new_stack_top,
            func = sym phase3_with_new_stack,
            options(noreturn)
        );
    }
}

#[no_mangle]
extern "C" fn phase3_with_new_stack() -> ! {
    vga::println("Stack switched to higher-half!");
    
    let new_rsp: u64;
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) new_rsp);
    }
    
    vga::print("New RSP: ");
    vga::print_hex(new_rsp);
    vga::println("");
    
    vga::println("");
    vga::println("Removing identity mapping...");

    for _ in 0..(2 * 5_000_000) {
            unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
        }
    
    // Map VGA to higher-half BEFORE removing identity mapping
    // We need to directly map since we can't use vmm here (it borrows pmm)
    unsafe {
        map_vga_to_higher_half();
    }
    
    // Switch VGA module to use higher-half address
    vga::switch_to_higher_half();
    
    vga::println("VGA mapped to higher-half");
    
    let pml4_phys = 0x2000u64;
    let pml4 = pml4_phys as *mut u64;
    
    unsafe {
        // Unmap PML4[0-3]
        core::ptr::write_volatile(pml4.add(0), 0);
        core::ptr::write_volatile(pml4.add(1), 0);
        core::ptr::write_volatile(pml4.add(2), 0);
        core::ptr::write_volatile(pml4.add(3), 0);
        
        // Flush TLB
        core::arch::asm!(
            "mov rax, cr3",
            "mov cr3, rax",
            out("rax") _,
        );
    }
    
    vga::println("Identity mapping removed!");
    
    // Reload IDT with higher-half addresses
    // Handler function pointers now resolve to higher-half via RIP-relative addressing
    idt::reload_idt();
    vga::println("IDT reloaded for higher-half");
    serial::println("IDT reloaded for higher-half");
    
    vga::println("");
    vga::println("========================================");
    vga::println("=== PHASE 3 COMPLETE! ===");
    vga::println("========================================");
    vga::println("");
    vga::println("Kernel Status:");
    vga::println("  RIP: Higher-half OK");
    vga::println("  RSP: Higher-half OK");
    vga::println("  Identity mapping: REMOVED OK");
    vga::println("  Kernel isolated: YES OK");
    serial::println(" Serial Works");
    vga::println("");
    vga::println("=== All 3 Phases Complete! ===");
    
    // === Phase 4: Heap Allocator ===
    vga::println("");
    
    let pmm_cur = unsafe { PMM_CURRENT };
    let pmm_end = unsafe { PMM_END };
    
    match allocator::init_heap(pmm_cur, pmm_end) {
        Ok(new_pmm_cur) => {
            // Update stored PMM state
            unsafe { PMM_CURRENT = new_pmm_cur; }
        }
        Err(e) => {
            vga::print("HEAP INIT FAILED: ");
            vga::println(e);
            serial::print("HEAP INIT FAILED: ");
            serial::println(e);
            loop { unsafe { core::arch::asm!("cli; hlt"); } }
        }
    }
    
    // Run allocator tests
    allocator::test_allocator();
    
    vga::println("");
    vga::println("========================================");
    vga::println("=== All 4 Phases Complete! ===");
    vga::println("========================================");
    
    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}
