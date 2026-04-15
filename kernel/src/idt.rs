#![allow(dead_code)]

use core::mem::size_of;
use core::arch::naked_asm;

/* ===============================
   IDT entry definition
   =============================== */

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_mid: u16,
    offset_high: u32,
    zero: u32,
}

impl IdtEntry {
    const fn missing() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            attributes: 0,
            offset_mid: 0,
            offset_high: 0,
            zero: 0,
        }
    }

    fn set_handler(&mut self, handler: u64) {
        self.offset_low = handler as u16;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
        self.selector = 0x18;    // 64-bit code segment from stage2 GDT
        self.ist = 0;
        self.attributes = 0x8E;  // present | ring 0 | interrupt gate
        self.zero = 0;
    }
}

/* ===============================
   IDT table
   =============================== */

#[repr(C, align(16))]
struct Idt {
    entries: [IdtEntry; 256],
}

impl Idt {
    const fn new() -> Self {
        Self {
            entries: [IdtEntry::missing(); 256],
        }
    }

    fn set_handler(&mut self, index: usize, handler: u64) {
        self.entries[index].set_handler(handler);
    }

    fn load(&'static self) {
        let ptr = IdtPointer {
            limit: (size_of::<Self>() - 1) as u16,
            base: self as *const _ as u64,
        };

        unsafe {
            core::arch::asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack));
        }
    }
}

#[repr(C, packed)]
struct IdtPointer {
    limit: u16,
    base: u64,
}

/* ===============================
   Exception handlers
   =============================== */

// VGA address constant - handlers use higher-half after remap
// Note: At boot time, VGA is at 0xB8000. After switch_to_higher_half(),
// it's at 0xFFFF_8000_000B_8000. We use a static to hold the current address.
static mut VGA_ADDR_FOR_IDT: u64 = 0xB8000;

/// Update the VGA address used by IDT exception handlers to higher-half
pub fn switch_to_higher_half_vga() {
    unsafe {
        VGA_ADDR_FOR_IDT = crate::vga::VGA_HIGHER_HALF;
    }
}

// Common exception handler that uses serial output (port I/O always works)
// and the VGA address from VGA_ADDR_FOR_IDT
fn exception_common(name: &str, has_error_code: bool) {
    // Use serial for reliable output (port I/O doesn't need mapping)
    crate::serial::println("");
    crate::serial::print("!!! EXCEPTION: ");
    crate::serial::println(name);
    
    // Also try VGA
    let vga_addr = unsafe { VGA_ADDR_FOR_IDT };
    unsafe {
        let vga = vga_addr as *mut u16;
        // Write exception name to VGA
        let msg = name.as_bytes();
        for (i, &b) in msg.iter().enumerate() {
            if i >= 80 { break; }
            core::ptr::write_volatile(vga.add(i), 0x4F00 | b as u16); // Red on white
        }
    }
}

// Divide by Zero (#DE) - no error code
#[unsafe(naked)]
extern "C" fn divide_by_zero_handler() {
    unsafe {
        naked_asm!(
            "cli",
            "push rax",
            "push rbx",
            "mov rax, 0xB8000",
            
            // Clear screen
            "mov rbx, 0",
            "1:",
            "mov word ptr [rax + rbx*2], 0x0F20",
            "inc rbx",
            "cmp rbx, 2000",
            "jl 1b",
            
            // Write "DIVIDE BY ZERO"
            "mov word ptr [rax + 0], 0x4F44",   // Red 'D'
            "mov word ptr [rax + 2], 0x4F49",   // Red 'I'
            "mov word ptr [rax + 4], 0x4F56",   // Red 'V'
            "mov word ptr [rax + 6], 0x4F49",   // Red 'I'
            "mov word ptr [rax + 8], 0x4F44",   // Red 'D'
            "mov word ptr [rax + 10], 0x4F45",  // Red 'E'
            "mov word ptr [rax + 12], 0x4F20",  // Red ' '
            "mov word ptr [rax + 14], 0x4F42",  // Red 'B'
            "mov word ptr [rax + 16], 0x4F59",  // Red 'Y'
            "mov word ptr [rax + 18], 0x4F20",  // Red ' '
            "mov word ptr [rax + 20], 0x4F5A",  // Red 'Z'
            "mov word ptr [rax + 22], 0x4F45",  // Red 'E'
            "mov word ptr [rax + 24], 0x4F52",  // Red 'R'
            "mov word ptr [rax + 26], 0x4F4F",  // Red 'O'
            
            "2:",
            "hlt",
            "jmp 2b",
        )
    }
}

// General Protection Fault (#GP) - has error code
#[unsafe(naked)]
extern "C" fn general_protection_fault_handler() {
    unsafe {
        naked_asm!(
            "cli",
            "push rax",
            "push rbx",
            "mov rax, 0xB8000",
            
            // Clear screen
            "mov rbx, 0",
            "1:",
            "mov word ptr [rax + rbx*2], 0x0F20",
            "inc rbx",
            "cmp rbx, 2000",
            "jl 1b",
            
            // Write "GP FAULT"
            "mov word ptr [rax + 0], 0x4F47",   // Red 'G'
            "mov word ptr [rax + 2], 0x4F50",   // Red 'P'
            "mov word ptr [rax + 4], 0x4F20",   // Red ' '
            "mov word ptr [rax + 6], 0x4F46",   // Red 'F'
            "mov word ptr [rax + 8], 0x4F41",   // Red 'A'
            "mov word ptr [rax + 10], 0x4F55",  // Red 'U'
            "mov word ptr [rax + 12], 0x4F4C",  // Red 'L'
            "mov word ptr [rax + 14], 0x4F54",  // Red 'T'
            
            "2:",
            "hlt",
            "jmp 2b",
        )
    }
}

// Page Fault (#PF) - has error code
// This handler uses serial port output (port I/O) which works regardless of page mappings
#[unsafe(naked)]
extern "C" fn page_fault_handler() {
    unsafe {
        naked_asm!(
            "cli",
            
            // Save error code from stack
            "pop r8",           // Error code pushed by CPU
            
            // Get CR2 (fault address)
            "mov r9, cr2",
            
            // === Output via serial port (COM1 = 0x3F8) ===
            // Print "PF:" prefix
            "mov dx, 0x3FD",    // Line status register

            // Wait for TX ready, then send 'P'
            "2:", "in al, dx", "test al, 0x20", "jz 2b",
            "mov dx, 0x3F8", "mov al, 0x50", "out dx, al",  // 'P'
            
            "mov dx, 0x3FD",
            "3:", "in al, dx", "test al, 0x20", "jz 3b",
            "mov dx, 0x3F8", "mov al, 0x46", "out dx, al",  // 'F'
            
            "mov dx, 0x3FD",
            "4:", "in al, dx", "test al, 0x20", "jz 4b",
            "mov dx, 0x3F8", "mov al, 0x20", "out dx, al",  // ' '

            "mov dx, 0x3FD",
            "44:", "in al, dx", "test al, 0x20", "jz 44b",
            "mov dx, 0x3F8", "mov al, 0x40", "out dx, al",  // '@'

            // Print CR2 (r9) as hex via serial
            "mov rcx, r9",
            "mov rbx, 60",      // Start from bit 60 (top nibble)
            "10:",
            "mov rax, rcx",
            "mov cl, bl",
            "shr rax, cl",
            "and rax, 0xF",
            "cmp al, 10",
            "jl 11f",
            "add al, 55",       // 'A'-10
            "jmp 12f",
            "11:",
            "add al, 48",       // '0'
            "12:",
            "mov dx, 0x3FD",
            "13:", "in al, dx", "test al, 0x20", "jz 13b",  // wait
            // Restore the character
            "mov rax, r9",
            "push rcx",
            "mov cl, bl",
            "shr rax, cl",
            "pop rcx",
            "and rax, 0xF",
            "cmp al, 10",
            "jl 14f",
            "add al, 55",
            "jmp 15f",
            "14:",
            "add al, 48",
            "15:",
            "mov dx, 0x3F8",
            "out dx, al",
            "sub bl, 4",
            "cmp bl, -4",       // We go 60,56,...,0 then stop
            "jg 10b",          // Changed from jge to jg to include 0

            // Print newline
            "mov dx, 0x3FD",
            "20:", "in al, dx", "test al, 0x20", "jz 20b",
            "mov dx, 0x3F8", "mov al, 0x0A", "out dx, al",
            
            "16:",
            "hlt",
            "jmp 16b",
        )
    }
}

// Double Fault (#DF) - has error code (always 0)
#[unsafe(naked)]
extern "C" fn double_fault_handler() {
    unsafe {
        naked_asm!(
            "cli",
            // Use serial output - doesn't need page mappings
            "mov dx, 0x3FD",
            "1:", "in al, dx", "test al, 0x20", "jz 1b",
            "mov dx, 0x3F8", "mov al, 0x44", "out dx, al",  // 'D'
            "mov dx, 0x3FD",
            "2:", "in al, dx", "test al, 0x20", "jz 2b",
            "mov dx, 0x3F8", "mov al, 0x46", "out dx, al",  // 'F'
            "mov dx, 0x3FD",
            "3:", "in al, dx", "test al, 0x20", "jz 3b",
            "mov dx, 0x3F8", "mov al, 0x21", "out dx, al",  // '!'
            "mov dx, 0x3FD",
            "4:", "in al, dx", "test al, 0x20", "jz 4b",
            "mov dx, 0x3F8", "mov al, 0x0A", "out dx, al",  // '\n'
            
            "5:",
            "hlt",
            "jmp 5b",
        )
    }
}

// Generic Exception Handler  
#[unsafe(naked)]
extern "C" fn generic_exception_handler() {
    unsafe {
        naked_asm!(
            "cli",
            // Use serial output
            "mov dx, 0x3FD",
            "1:", "in al, dx", "test al, 0x20", "jz 1b",
            "mov dx, 0x3F8", "mov al, 0x45", "out dx, al",  // 'E'
            "mov dx, 0x3FD",
            "2:", "in al, dx", "test al, 0x20", "jz 2b",
            "mov dx, 0x3F8", "mov al, 0x58", "out dx, al",  // 'X'
            "mov dx, 0x3FD",
            "3:", "in al, dx", "test al, 0x20", "jz 3b",
            "mov dx, 0x3F8", "mov al, 0x21", "out dx, al",  // '!'
            "mov dx, 0x3FD",
            "4:", "in al, dx", "test al, 0x20", "jz 4b",
            "mov dx, 0x3F8", "mov al, 0x0A", "out dx, al",  // '\n'
            
            "5:",
            "hlt",
            "jmp 5b",
        )
    }
}

/* ===============================
   Public init
   =============================== */

static mut IDT: Idt = Idt::new();

pub fn init() {
    unsafe {
        let idt_ptr = core::ptr::addr_of_mut!(IDT);
        
        // Set generic handler for all exceptions
        for i in 0..32 {
            (*idt_ptr).set_handler(i, generic_exception_handler as u64);
        }
        
        // Override with specific handlers
        (*idt_ptr).set_handler(0, divide_by_zero_handler as u64);              // #DE
        (*idt_ptr).set_handler(8, double_fault_handler as u64);                // #DF
        (*idt_ptr).set_handler(13, general_protection_fault_handler as u64);   // #GP
        (*idt_ptr).set_handler(14, page_fault_handler as u64);                 // #PF
        
        (*idt_ptr).load();
    }
}

/// Reload the IDT with higher-half addresses.
/// Must be called after jumping to higher-half so that function pointers
/// resolve to higher-half addresses (via RIP-relative addressing).
pub fn reload_idt() {
    unsafe {
        let idt_ptr = core::ptr::addr_of_mut!(IDT);
        
        // Re-register all handlers - now their addresses will be higher-half
        // because the compiler uses RIP-relative addressing and RIP is in higher-half
        for i in 0..32 {
            (*idt_ptr).set_handler(i, generic_exception_handler as u64);
        }
        
        (*idt_ptr).set_handler(0, divide_by_zero_handler as u64);
        (*idt_ptr).set_handler(8, double_fault_handler as u64);
        (*idt_ptr).set_handler(13, general_protection_fault_handler as u64);
        (*idt_ptr).set_handler(14, page_fault_handler as u64);
        
        // The IDT static is also at a higher-half address (RIP-relative)
        (*idt_ptr).load();
    }
}
