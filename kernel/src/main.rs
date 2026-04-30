#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

mod idt;
mod vga;
mod pmm;
mod vmm;
mod serial;
mod allocator;
mod gdt;
mod pic;
mod pit;
mod keyboard;
mod pci;
mod lpc;
mod acpi;
mod task;
mod scheduler;

use core::panic::PanicInfo;
use pmm::PhysicalMemoryManager;
use crate::vmm::PageTableManager;

#[alloc_error_handler]
fn alloc_error_handler(_layout: core::alloc::Layout) -> ! {
    crate::serial::println("PANIC: Out of memory / Alloc Error!");
    loop { unsafe { core::arch::asm!("hlt"); } }
}

// LLVM intrinsics required in freestanding environment
#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n { *dest.add(i) = *src.add(i); i += 1; }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if (dest as usize) < (src as usize) {
        let mut i = 0;
        while i < n { *dest.add(i) = *src.add(i); i += 1; }
    } else {
        let mut i = n;
        while i > 0 { i -= 1; *dest.add(i) = *src.add(i); }
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut u8, c: i32, n: usize) -> *mut u8 {
    let val = c as u8;
    let mut i = 0;
    while i < n { *dest.add(i) = val; i += 1; }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b { return a as i32 - b as i32; }
        i += 1;
    }
    0
}

pub const KERNEL_VIRT_BASE: u64 = 0xFFFF_8000_0000_0000;

static mut PMM_CURRENT: u64 = 0;
static mut PMM_END: u64 = 0;

#[repr(C, align(16))]
struct StaticStack {
    data: [u8; 16384],
}

static mut STATIC_KERNEL_STACK: StaticStack = StaticStack { data: [0; 16384] };

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    vga::println("KERNEL PANIC!");
    loop { unsafe { core::arch::asm!("cli; hlt"); } }
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
        1 => "USABLE", 2 => "RESERVED", 3 => "ACPI",
        4 => "ACPI_NVS", 5 => "BAD", _ => "UNKNOWN",
    }
}

fn display_e820(entries: &[E820Entry]) {
    vga::println("Memory Map:");
    for (i, e) in entries.iter().enumerate() {
        if e.base == 0 && e.length == 0 && e.kind == 0 { continue; }
        vga::print("  "); vga::print_num(i as u64); vga::print(": ");
        vga::print_hex(e.base); vga::print(" - "); vga::print_hex(e.base + e.length);
        vga::print(" "); vga::println(e820_type_str(e.kind));
    }
}

fn find_and_display_rsdp() {
    vga::print("Searching for ACPI RSDP... ");
    let ebda_seg = unsafe { *(0x40E as *const u16) } as u64;
    let ebda_addr = ebda_seg << 4;

    let rsdp_addr = if ebda_addr != 0 && ebda_addr < 0x100000 {
        search_rsdp_range(ebda_addr, 1024).or_else(|| search_rsdp_range(0xE0000, 0x20000))
    } else {
        search_rsdp_range(0xE0000, 0x20000)
    };

    match rsdp_addr {
        Some(addr) => {
            vga::println("Found!");
            vga::print("  RSDP at: "); vga::print_hex(addr); vga::println("");
            unsafe {
                let rsdp = &*(addr as *const RsdpV1);
                vga::print("  Revision: "); vga::print_num(rsdp.revision as u64);
                if rsdp.revision == 0 { vga::println(" (ACPI 1.0)"); }
                else { vga::println(" (ACPI 2.0+)"); }
                vga::print("  RSDT at: "); vga::print_hex(rsdp.rsdt_address as u64); vga::println("");
                acpi::parse_from_rsdt(rsdp.rsdt_address);
                vga::println("  ACPI tables parsed");
            }
        }
        None => vga::println("Not found!"),
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
                if sum == 0 { return Some(addr); }
            }
        }
        addr += 16;
    }
    None
}

fn test_pmm(pmm: &mut PhysicalMemoryManager) {
    vga::println("Allocating physical frames:");
    for i in 0..5 {
        match pmm.alloc_frame() {
            Some(frame) => {
                vga::print("  Frame "); vga::print_num(i); vga::print(": ");
                vga::print_hex(frame); vga::println("");
            }
            None => vga::println("  OUT OF MEMORY"),
        }
    }
}

fn test_vmm_with_frames(vmm: &mut PageTableManager, phys_low: u64, phys_high: u64) {
    vga::println("=== VMM Lower-Half Test ===");
    let virt_low = 0x0000_0000_0040_0000;
    unsafe {
        vmm.map_page(virt_low, phys_low, 0x3);
        *(virt_low as *mut u64) = 0xCAFEBABEDEADBEEF;
        let val = *(virt_low as *const u64);
        if val == 0xCAFEBABEDEADBEEF { vga::println("Lower-half: SUCCESS"); }
        else { vga::println("Lower-half: FAILED"); }
    }

    vga::println("");
    vga::println("=== VMM Higher-Half Test ===");
    let virt_high = KERNEL_VIRT_BASE;
    unsafe {
        vmm.map_page(virt_high, phys_high, 0x3);
        *(virt_high as *mut u64) = 0xDEADC0DEBADC0FFE;
        let val = *(virt_high as *const u64);
        if val == 0xDEADC0DEBADC0FFE { vga::println("Higher-half: SUCCESS"); }
        else { vga::println("Higher-half: FAILED"); }
    }
}

fn map_kernel(vmm: &mut PageTableManager) {
    vga::println("");
    vga::println("=== Mapping Kernel to Higher-Half ===");
    let kernel_phys_start = 0x100000u64;
    let kernel_phys_end = 0x200000u64;
    let kernel_size = kernel_phys_end - kernel_phys_start;

    vga::print("Kernel physical: ");
    vga::print_hex(kernel_phys_start); vga::print(" - "); vga::print_hex(kernel_phys_end);
    vga::println("");
    vga::print("Kernel size: "); vga::print_num(kernel_size / 1024); vga::println(" KB");

    let kernel_virt = KERNEL_VIRT_BASE + kernel_phys_start;
    vga::print("Mapping to: "); vga::print_hex(kernel_virt); vga::println("");

    unsafe { vmm.map_range(kernel_virt, kernel_phys_start, kernel_size, 0x3); }
    vga::println("Kernel mapped!");

    for _ in 0..(2 * 5_000_000) {
        unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
    }

    vga::println("");
    vga::println("=== Verification ===");
    let h = unsafe { *(kernel_virt as *const u64) };
    let i = unsafe { *(kernel_phys_start as *const u64) };
    vga::print("Higher-half: "); vga::print_hex(h); vga::println("");
    vga::print("Identity:    "); vga::print_hex(i); vga::println("");

    if h == i { vga::println("Kernel mapping: SUCCESS!"); }
    else { vga::println("Kernel mapping: FAILED!"); }

    for _ in 0..(2 * 5_000_000) {
        unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
    }
}

unsafe fn map_vga_to_higher_half() {
    let virt = vga::VGA_HIGHER_HALF;
    let phys = vga::VGA_PHYS;

    let pml4_i = ((virt >> 39) & 0x1FF) as usize;
    let pdpt_i = ((virt >> 30) & 0x1FF) as usize;
    let pd_i   = ((virt >> 21) & 0x1FF) as usize;
    let pt_i   = ((virt >> 12) & 0x1FF) as usize;

    let pml4 = 0x2000 as *mut u64;

    let pdpt_entry_ptr = pml4.add(pml4_i);
    let pdpt_entry = core::ptr::read_volatile(pdpt_entry_ptr);
    let pdpt_phys = if (pdpt_entry & 1) == 0 {
        let frame = 0x8000u64;
        let table_ptr = frame as *mut u64;
        for i in 0..512 { core::ptr::write_volatile(table_ptr.add(i), 0); }
        core::ptr::write_volatile(pdpt_entry_ptr, frame | 0x3);
        frame
    } else { pdpt_entry & 0x000F_FFFF_FFFF_F000 };

    let pd_entry_ptr = (pdpt_phys as *mut u64).add(pdpt_i);
    let pd_entry = core::ptr::read_volatile(pd_entry_ptr);
    let pd_phys = if (pd_entry & 1) == 0 {
        let frame = 0x9000u64;
        let table_ptr = frame as *mut u64;
        for i in 0..512 { core::ptr::write_volatile(table_ptr.add(i), 0); }
        core::ptr::write_volatile(pd_entry_ptr, frame | 0x3);
        frame
    } else { pd_entry & 0x000F_FFFF_FFFF_F000 };

    let pt_entry_ptr = (pd_phys as *mut u64).add(pd_i);
    let pt_entry = core::ptr::read_volatile(pt_entry_ptr);
    let pt_phys = if (pt_entry & 1) == 0 {
        let frame = 0xA000u64;
        let table_ptr = frame as *mut u64;
        for i in 0..512 { core::ptr::write_volatile(table_ptr.add(i), 0); }
        core::ptr::write_volatile(pt_entry_ptr, frame | 0x3);
        frame
    } else { pt_entry & 0x000F_FFFF_FFFF_F000 };

    let final_entry_ptr = (pt_phys as *mut u64).add(pt_i);
    core::ptr::write_volatile(final_entry_ptr, phys | 0x3);
    core::arch::asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags));
}

extern "C" {
    static __rela_start: u8;
    static __rela_end: u8;
    static __bss_start: u8;
    static __bss_end: u8;
}

#[no_mangle]
pub extern "C" fn _start(e820_ptr: *const E820Entry, e820_count: usize) -> ! {
    // Phase 1 relocation: patch GOT with raw physical addends
    unsafe {
        let mut rela_start: usize;
        let mut rela_end: usize;
        core::arch::asm!("lea {0}(%rip), {1}", sym __rela_start, out(reg) rela_start, options(att_syntax, nomem, nostack));
        core::arch::asm!("lea {0}(%rip), {1}", sym __rela_end, out(reg) rela_end, options(att_syntax, nomem, nostack));

        let mut pos = rela_start;
        while pos + 24 <= rela_end {
            let offset = *(pos as *const u64);
            let info = *((pos + 8) as *const u64);
            let addend = *((pos + 16) as *const u64);
            let rela_type = (info & 0xFFFF_FFFF) as u32;
            if rela_type == 8 {
                core::ptr::write_volatile(offset as *mut u64, addend);
            }
            pos += 24;
        }
    }

    // Zero .bss (NOBITS in ELF, not initialized by flat binary loader)
    unsafe {
        let mut bss_start: usize;
        let mut bss_end: usize;
        core::arch::asm!("lea {0}(%rip), {1}", sym __bss_start, out(reg) bss_start, options(att_syntax, nomem, nostack));
        core::arch::asm!("lea {0}(%rip), {1}", sym __bss_end, out(reg) bss_end, options(att_syntax, nomem, nostack));
        let mut ptr = bss_start as *mut u8;
        let end = bss_end as *mut u8;
        while ptr < end { core::ptr::write_volatile(ptr, 0); ptr = ptr.add(1); }
    }

    for _ in 0..(2 * 5_000_000) {
        unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
    }

    vga::init();
    serial::init();
    idt::init();

    vga::println("=== Kernel Boot ===");
    vga::println("");

    let entries = unsafe { core::slice::from_raw_parts(e820_ptr, e820_count) };
    vga::print("E820 entries: "); vga::print_num(e820_count as u64); vga::println("");
    vga::println("");

    display_e820(entries);
    vga::println("");

    find_and_display_rsdp();
    vga::println("");

    vga::println("Initializing PMM...");
    let mut pmm = match PhysicalMemoryManager::new(entries) {
        Some(p) => p,
        None => { vga::println("ERROR: No RAM!"); loop { unsafe { core::arch::asm!("hlt"); } } }
    };
    vga::println("PMM initialized");
    vga::println("");

    test_pmm(&mut pmm);
    vga::println("");

    vga::println("=== VMM Initialization ===");
    let pml4_phys = 0x2000u64;
    unsafe {
        let pml4 = pml4_phys as *mut u64;
        pml4.add(510).write_volatile(pml4_phys | 0x3);
        core::arch::asm!("mov rax, cr3; mov cr3, rax", out("rax") _);
    }
    vga::println("Recursive mapping installed");

    let phys_low = pmm.alloc_frame().expect("No frames");
    let phys_high = pmm.alloc_frame().expect("No frames");

    let mut vmm = unsafe { PageTableManager::new(pml4_phys, &mut pmm) };
    vga::println("VMM created");
    vga::println("");

    test_vmm_with_frames(&mut vmm, phys_low, phys_high);
    vga::println("");
    map_kernel(&mut vmm);

    let (pmm_cur, pmm_end) = pmm.get_state();
    unsafe { PMM_CURRENT = pmm_cur; PMM_END = pmm_end; }

    vga::println("");
    vga::println("=== Phase 2: Jump to Higher-Half ===");
    vga::println("");

    let current_rsp: u64;
    unsafe { core::arch::asm!("mov {}, rsp", out(reg) current_rsp); }
    vga::print("Current RSP: "); vga::print_hex(current_rsp); vga::println("");

    let target_phys = higher_half_main as u64;
    let offset = target_phys - 0x100000;
    let target_virt = KERNEL_VIRT_BASE + 0x100000 + offset;

    vga::print("Jump to: "); vga::print_hex(target_virt); vga::println("");
    vga::println("Jumping NOW...");
    vga::println("");

    unsafe {
        core::arch::asm!("jmp {target}", target = in(reg) target_virt, options(noreturn));
    }
}

#[no_mangle]
extern "C" fn higher_half_main() -> ! {
    vga::println("================================");
    vga::println("=== HIGHER-HALF SUCCESS! ===");
    vga::println("================================");
    vga::println("");

    let rip: u64;
    unsafe { core::arch::asm!("lea {}, [rip]", out(reg) rip); }
    vga::print("RIP: "); vga::print_hex(rip); vga::println("");
    if rip >= KERNEL_VIRT_BASE { vga::println("Running in higher-half: SUCCESS!"); }

    vga::println("");
    vga::println("=== Phase 3: Switch Stack & Remove Identity ===");

    let stack_higher = unsafe { core::ptr::addr_of_mut!(STATIC_KERNEL_STACK.data) as u64 };
    let new_stack_top = stack_higher + 16384;

    vga::print("Stack at: "); vga::print_hex(stack_higher); vga::println("");
    vga::print("New stack top: "); vga::print_hex(new_stack_top); vga::println("");

    if stack_higher < KERNEL_VIRT_BASE {
        vga::println("ERROR: Stack not in higher-half!");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    unsafe {
        let test_ptr = stack_higher as *mut u64;
        core::ptr::write_volatile(test_ptr, 0xDEADBEEF);
        let test = core::ptr::read_volatile(test_ptr);
        if test == 0xDEADBEEF { vga::println("Stack is accessible: OK"); }
        else { vga::println("Stack test FAILED!"); loop { core::arch::asm!("hlt"); } }
    }

    vga::println("");
    vga::println("Switching to higher-half stack...");

    unsafe {
        core::arch::asm!(
            "mov rsp, {stack}", "call {func}",
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
    unsafe { core::arch::asm!("mov {}, rsp", out(reg) new_rsp); }
    vga::print("New RSP: "); vga::print_hex(new_rsp); vga::println("");

    vga::println("");
    vga::println("Removing identity mapping...");

    for _ in 0..(2 * 5_000_000) {
        unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
    }

    unsafe { map_vga_to_higher_half(); }
    vga::switch_to_higher_half();
    vga::println("VGA mapped to higher-half");

    let pml4 = 0x2000u64 as *mut u64;
    unsafe {
        core::ptr::write_volatile(pml4.add(0), 0);
        core::ptr::write_volatile(pml4.add(1), 0);
        core::ptr::write_volatile(pml4.add(2), 0);
        core::ptr::write_volatile(pml4.add(3), 0);
        core::arch::asm!("mov rax, cr3", "mov cr3, rax", out("rax") _);
    }
    vga::println("Identity mapping removed!");

    idt::reload_idt();
    gdt::init();
    vga::println("IDT + GDT reloaded for higher-half");

    // Phase 2 relocation: re-apply GOT entries with higher-half bias
    unsafe {
        let mut rela_start: usize;
        let mut rela_end: usize;
        core::arch::asm!("lea {0}(%rip), {1}", sym __rela_start, out(reg) rela_start, options(att_syntax, nomem, nostack));
        core::arch::asm!("lea {0}(%rip), {1}", sym __rela_end, out(reg) rela_end, options(att_syntax, nomem, nostack));
        let mut pos = rela_start;
        while pos + 24 <= rela_end {
            let offset = *(pos as *const u64);
            let info = *((pos + 8) as *const u64);
            let addend = *((pos + 16) as *const u64);
            let rela_type = (info & 0xFFFF_FFFF) as u32;
            if rela_type == 8 {
                let target = (KERNEL_VIRT_BASE + offset) as *mut u64;
                core::ptr::write_volatile(target, addend + KERNEL_VIRT_BASE);
            }
            pos += 24;
        }
    }

    vga::println("");
    vga::println("========================================");
    vga::println("=== PHASE 3 COMPLETE! ===");
    vga::println("========================================");
    vga::println("");

    // Phase 4: Heap
    let pmm_cur = unsafe { PMM_CURRENT };
    let pmm_end = unsafe { PMM_END };
    match allocator::init_heap(pmm_cur, pmm_end) {
        Ok(new_pmm_cur) => unsafe { PMM_CURRENT = new_pmm_cur; },
        Err(e) => {
            vga::print("HEAP INIT FAILED: "); vga::println(e);
            loop { unsafe { core::arch::asm!("cli; hlt"); } }
        }
    }
    allocator::test_allocator();

    vga::println("");
    vga::println("========================================");
    vga::println("=== Phase 4 Complete: Heap OK ===");
    vga::println("========================================");

    // Phase 5: Interrupts & Timer
    vga::println("");
    vga::println("=== Phase 5: Interrupts & Timer ===");
    vga::println("");

    pic::init();
    vga::println("  PIC: Initialized");
    pit::init();
    vga::println("  PIT: Initialized (100 Hz)");
    idt::reload_idt();
    vga::println("  IDT: Reloaded with IRQ handlers");

    pic::unmask_irq(0);
    pic::unmask_irq(1);
    vga::println("  IRQ0 (Timer): Unmasked");
    vga::println("  IRQ1 (Keyboard): Unmasked");

    unsafe { core::arch::asm!("sti"); }
    vga::println("  Interrupts: ENABLED");

    pit::sleep_ms(500);
    let ticks = pit::get_ticks();
    vga::print("  Timer test: "); vga::print_num(ticks); vga::println(" ticks in ~500ms");
    if ticks >= 40 { vga::println("  Timer: OK"); }
    else { vga::println("  Timer: UNEXPECTED"); }

    // Phase 6: PCI Bus Enumeration
    vga::println("");
    vga::println("=== Phase 6: PCI Bus Scan ===");
    vga::println("");

    let pci_devices = pci::enumerate();
    pci::display_devices(&pci_devices);

    vga::println("");
    vga::println("=== LPC/ISA Legacy Probe ===");
    vga::println("");
    lpc::scan_and_display();

    vga::println("");
    vga::println("=== ACPI System Info ===");
    vga::println("");
    acpi::display_info();

    // Phase 7: Process Management
    vga::println("");
    vga::println("=== Phase 7: Process Management ===");
    vga::println("");

    scheduler::init();
    scheduler::spawn(task_heartbeat);
    scheduler::spawn(task_prime_compute);
    scheduler::spawn(task_counter);
    scheduler::spawn(task_memory_stress);
    scheduler::spawn(task_fast_ticker);
    scheduler::spawn(task_serial_logger);
    scheduler::spawn(task_fibonacci);
    scheduler::spawn(task_watchdog);

    vga::println("  8 kernel threads spawned");
    vga::println("");
    vga::println("========================================");
    vga::println("=== All 7 Phases Complete! ===");
    vga::println("========================================");
    vga::println("");

    loop { unsafe { core::arch::asm!("hlt"); } }
}

// Task 1: Periodic heartbeat — prints a dot every 2 seconds
fn task_heartbeat() -> ! {
    let mut beat: u64 = 0;
    loop {
        beat += 1;
        serial::print("[HEARTBEAT] beat #");
        serial::print_num(beat);
        serial::println("");
        vga::print(".");
        pit::sleep_ms(2000);
    }
}

// Task 2: CPU-bound — find prime numbers (gets preempted mid-computation)
fn task_prime_compute() -> ! {
    let mut n: u64 = 2;
    let mut found: u64 = 0;
    loop {
        if is_prime(n) {
            found += 1;
            if found % 50 == 0 {
                serial::print("[PRIME] found ");
                serial::print_num(found);
                serial::print(" primes (latest: ");
                serial::print_num(n);
                serial::println(")");
                vga::print("P");
            }
        }
        n += 1;
        if n > 100_000 { n = 2; found = 0; }
    }
}

fn is_prime(n: u64) -> bool {
    if n < 2 { return false; }
    if n == 2 || n == 3 { return true; }
    if n % 2 == 0 || n % 3 == 0 { return false; }
    let mut i = 5u64;
    while i * i <= n {
        if n % i == 0 || n % (i + 2) == 0 { return false; }
        i += 6;
    }
    true
}

// Task 3: Counter — tracks how many ticks it's been scheduled across
fn task_counter() -> ! {
    let mut count: u64 = 0;
    let mut last_report = pit::get_ticks();
    loop {
        count += 1;
        let now = pit::get_ticks();
        if now - last_report >= 300 { // every ~3 seconds
            serial::print("[COUNTER] iterations=");
            serial::print_num(count);
            serial::print(" at tick ");
            serial::print_num(now);
            serial::println("");
            vga::print("C");
            last_report = now;
        }
    }
}

// Task 4: Memory stress — allocates and frees Vec objects
fn task_memory_stress() -> ! {
    use alloc::vec::Vec;
    let mut cycle: u64 = 0;
    loop {
        cycle += 1;
        let mut v: Vec<u64> = Vec::new();
        for i in 0..64 {
            v.push(i * cycle);
        }
        let sum: u64 = v.iter().sum();
        drop(v);

        if cycle % 100 == 0 {
            serial::print("[MALLOC] cycle ");
            serial::print_num(cycle);
            serial::print(" sum=");
            serial::print_num(sum);
            serial::println("");
            vga::print("M");
        }
    }
}

// Task 5: Fast ticker — runs every 100ms to test rapid context switching
fn task_fast_ticker() -> ! {
    let mut tick_count: u64 = 0;
    loop {
        tick_count += 1;
        if tick_count % 50 == 0 {
            serial::print("[FAST] ");
            serial::print_num(tick_count);
            serial::print(" rapid cycles at tick ");
            serial::print_num(pit::get_ticks());
            serial::println("");
            vga::print("F");
        }
        pit::sleep_ms(100);
    }
}

// Task 6: Serial I/O bound — writes longer messages simulating log output
fn task_serial_logger() -> ! {
    let mut log_id: u64 = 0;
    loop {
        log_id += 1;
        serial::print("[LOG #");
        serial::print_num(log_id);
        serial::print("] System uptime: ");
        serial::print_num(pit::get_ticks() / 100);
        serial::print("s | Tasks active | Heap OK | IRQs: ");
        serial::print_num(pit::get_ticks());
        serial::println("");
        pit::sleep_ms(3000);
    }
}

// Task 7: Fibonacci — CPU intensive with periodic output
fn task_fibonacci() -> ! {
    let mut round: u64 = 0;
    loop {
        round += 1;
        let result = fib(30 + (round % 5));
        serial::print("[FIB] fib(");
        serial::print_num(30 + (round % 5));
        serial::print(")=");
        serial::print_num(result);
        serial::println("");
        vga::print("B");
        pit::sleep_ms(500);
    }
}

fn fib(n: u64) -> u64 {
    if n <= 1 { return n; }
    let mut a: u64 = 0;
    let mut b: u64 = 1;
    for _ in 2..=n {
        let tmp = a + b;
        a = b;
        b = tmp;
    }
    b
}

// Task 8: Watchdog — monitors tick count and reports scheduling health
fn task_watchdog() -> ! {
    let mut last_tick = pit::get_ticks();
    let mut checks: u64 = 0;
    loop {
        pit::sleep_ms(5000);
        checks += 1;
        let now = pit::get_ticks();
        let elapsed = now - last_tick;
        serial::print("[WATCHDOG #");
        serial::print_num(checks);
        serial::print("] ");
        serial::print_num(elapsed);
        serial::print(" ticks elapsed (~");
        serial::print_num(elapsed / 100);
        serial::print("s) | uptime ");
        serial::print_num(now / 100);
        serial::println("s");
        vga::print("W");
        last_tick = now;
    }
}
