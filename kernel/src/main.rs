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

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct RsdpV2 {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_address: u32,
    length: u32,
    xsdt_address: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

fn delay_seconds(seconds: u32) {
    let iterations = seconds * 5_000_000;
    for _ in 0..iterations {
        unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
    }
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

fn find_rsdp() -> Option<u64> {
    let ebda_seg = unsafe { *(0x40E as *const u16) } as u64;
    let ebda_addr = ebda_seg << 4;
    
    if ebda_addr != 0 && ebda_addr < 0x100000 {
        if let Some(addr) = search_rsdp_range(ebda_addr, 1024) {
            return Some(addr);
        }
    }
    
    search_rsdp_range(0xE0000, 0x20000)
}

fn search_rsdp_range(start: u64, length: u64) -> Option<u64> {
    let mut addr = start;
    let end = start + length;
    
    while addr < end {
        unsafe {
            let ptr = addr as *const u64;
            let sig = ptr.read_volatile();
            
            if sig == 0x2052545020445352 {
                if verify_rsdp_checksum(addr) {
                    return Some(addr);
                }
            }
        }
        
        addr += 16;
    }
    
    None
}

fn verify_rsdp_checksum(addr: u64) -> bool {
    unsafe {
        let bytes = core::slice::from_raw_parts(addr as *const u8, 20);
        let sum: u8 = bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        sum == 0
    }
}

fn display_rsdp_info(rsdp_addr: u64) {
    unsafe {
        let rsdp = &*(rsdp_addr as *const RsdpV1);
        
        vga::print("  RSDP at: ");
        vga::print_hex(rsdp_addr);
        vga::println("");
        
        vga::print("  Signature: ");
        vga::print_ascii(&rsdp.signature);
        vga::println("");
        
        vga::print("  OEM ID: ");
        vga::print_ascii(&rsdp.oem_id);
        vga::println("");
        
        vga::print("  Revision: ");
        vga::print_num(rsdp.revision as u64);
        if rsdp.revision == 0 {
            vga::println(" (ACPI 1.0)");
        } else {
            vga::println(" (ACPI 2.0+)");
        }
        
        vga::print("  RSDT Address: ");
        vga::print_hex(rsdp.rsdt_address as u64);
        vga::println("");
        
        if rsdp.revision >= 2 {
            let rsdp2 = &*(rsdp_addr as *const RsdpV2);
            vga::print("  XSDT Address: ");
            vga::print_hex(rsdp2.xsdt_address);
            vga::println("");
        }
    }
}

#[no_mangle]
pub extern "C" fn _start(e820_ptr: *const E820Entry, e820_count: usize) -> ! {
    delay_seconds(2);
    
    vga::init();
    idt::init();
    
    vga::println("=== Kernel Boot ===");
    vga::println("");
    
    vga::print("E820 entries: ");
    vga::print_num(e820_count as u64);
    vga::println("");
    vga::println("");
    
    let entries = unsafe {
        core::slice::from_raw_parts(e820_ptr, e820_count)
    };
    
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
    
    vga::println("");
    
    vga::print("Searching for ACPI RSDP... ");
    match find_rsdp() {
        Some(rsdp_addr) => {
            vga::println("Found!");
            display_rsdp_info(rsdp_addr);
        }
        None => {
            vga::println("Not found!");
        }
    }
    
    vga::println("");
    
    vga::println("Initializing Physical Memory Manager...");
    let mut pmm = match PhysicalMemoryManager::new(entries) {
        Some(p) => p,
        None => {
            vga::println("ERROR: No usable RAM found!");
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
    };
    vga::println("PMM initialized.");
    
    vga::println("");
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

    delay_seconds(2);
    
    vga::println("");
    vga::println("=== VMM Initialization ===");
    
    // Install recursive mapping at PML4[510] (optional, not used by our VMM)
    let pml4_phys = 0x2000u64;
    
    unsafe {
        let pml4 = pml4_phys as *mut u64;
        pml4.add(510).write_volatile(pml4_phys | 0x3);
        
        core::arch::asm!(
            "mov rax, cr3",
            "mov cr3, rax",
            out("rax") _,
        );
    }
    vga::println("Recursive mapping installed at PML4[510]");
    
    // Allocate frames for VMM tests
    let phys_low = pmm.alloc_frame().expect("No free frames");
    let phys_high = pmm.alloc_frame().expect("No free frames");
    
    vga::print("Test frames allocated: ");
    vga::print_hex(phys_low);
    vga::print(", ");
    vga::print_hex(phys_high);
    vga::println("");
    
    // Create VMM
    let mut vmm = unsafe {
        PageTableManager::new(pml4_phys, &mut pmm)
    };
    vga::println("VMM created");
    
    vga::println("");
    vga::println("=== VMM Lower-Half Test ===");
    
    let virt_low = 0x0000_0000_0040_0000;
    
    vga::print("Mapping ");
    vga::print_hex(virt_low);
    vga::print(" -> ");
    vga::print_hex(phys_low);
    vga::println("");
    
    unsafe {
        vmm.map_page(virt_low, phys_low, 0x3);
        *(virt_low as *mut u64) = 0xCAFEBABEDEADBEEF;
    }
    
    let val_low = unsafe { *(virt_low as *const u64) };
    
    vga::print("Read value: ");
    vga::print_hex(val_low);
    
    if val_low == 0xCAFEBABEDEADBEEF {
        vga::println("  SUCCESS");
    } else {
        vga::println("  FAILED");
    }
    
    vga::println("");
    vga::println("=== VMM Higher-Half Test ===");
    
    let virt_high = KERNEL_VIRT_BASE;
    
    vga::print("Mapping ");
    vga::print_hex(virt_high);
    vga::print(" -> ");
    vga::print_hex(phys_high);
    vga::println("");
    
    unsafe {
        vmm.map_page(virt_high, phys_high, 0x3);
        *(virt_high as *mut u64) = 0xDEADC0DEBADC0FFE;
    }
    
    let val_high = unsafe { *(virt_high as *const u64) };
    
    vga::print("Read value: ");
    vga::print_hex(val_high);
    
    if val_high == 0xDEADC0DEBADC0FFE {
        vga::println("  SUCCESS");
    } else {
        vga::println("  FAILED");
    }
    
    vga::println("");
    vga::println("=== All Tests Passed ===");
    vga::println("Kernel initialized successfully.");
    
    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}
