#![no_std]
#![no_main]

mod idt;
mod vga;
mod pmm;
mod vmm;

use core::panic::PanicInfo;
use pmm::PhysicalMemoryManager;
use crate::vmm::PageTableManager;

pub const KERNEL_VIRT_BASE: u64 = 0xFFFF_8000_0000_0000;

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
}

#[no_mangle]
pub extern "C" fn _start(e820_ptr: *const E820Entry, e820_count: usize) -> ! {
    // Delay 2 seconds before starting
    for _ in 0..(2 * 5_000_000) {
        unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
    }
    
    vga::init();
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

     for _ in 0..(2 * 5_000_000) {
        unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
    }
    
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
    vga::println("=== Kernel Ready for Higher-Half Execution ===");
    vga::println("Kernel initialized successfully.");
    
    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}
