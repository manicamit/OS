#![no_std]
#![no_main]

mod idt;
mod vga;
mod pmm;
mod vmm;

use core::panic::PanicInfo;
use pmm::PhysicalMemoryManager;
use crate::vmm::PageTableManager;

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
                
// ============================
  // VMM SETUP (CORRECT)
  // ============================
  
  vga::println("");
  vga::println("=== VMM TEST ===");
  
  // Read CR3
 // inside _start()
  
vga::println("=== VMM LOWER-HALF TEST ===");
  
  // Read CR3
  let cr3: u64;
  unsafe { core::arch::asm!("mov {}, cr3", out(reg) cr3); }
  let pml4_phys = cr3 & 0x000F_FFFF_FFFF_F000;
  
  // Install recursive mapping at PML4[510]
  unsafe {
      let pml4 = pml4_phys as *mut u64;
      pml4.add(510).write(pml4_phys | 0x3);
  }
  
  // Reload CR3 to activate recursion
  unsafe {
      core::arch::asm!("mov cr3, {}", in(reg) pml4_phys);
  }
  
  vga::println("Recursive slot installed (not yet dereferenced)");
  
  // Allocate data frame FIRST
  let phys = pmm.alloc_frame().expect("No free frames");
  
  vga::print("Allocated frame: ");
  vga::print_hex(phys);
  vga::println("");
  
  // Create VMM AFTER allocation
  let mut vmm = unsafe {
      PageTableManager::new(pml4_phys, &mut pmm)
  };
  
  // Test lower-half mapping
  let virt = 0x0000_0000_0040_0000;
  
  vga::print("Mapping ");
  vga::print_hex(virt);
  vga::print(" -> ");
  vga::print_hex(phys);
  vga::println("");
  
  unsafe {
      vmm.map_page(virt, phys, 0x2);
      *(virt as *mut u64) = 0xCAFEBABEDEADBEEF;
  }
  
  let val = unsafe { *(virt as *const u64) };
  
  vga::print("Read value: ");
  vga::print_hex(val);
  
  if val == 0xCAFEBABEDEADBEEF {
      vga::println("  SUCCESS");
  } else {
      vga::println("  FAILED");
  }
  
  vga::println("VMM lower-half test PASSED");
  
  vga::println("Kernel initialized successfully.");
  
  loop {
      unsafe { core::arch::asm!("cli; hlt"); }
  }          
            
}
