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
#[unsafe(naked)]
extern "C" fn page_fault_handler() {
    unsafe {
        naked_asm!(
            "cli",
            "push rax",
            "push rbx",
            "push rcx",
            "push rdx",
            
            // Get error code (on stack after our pushes)
            "mov rdx, [rsp + 32]",  // Error code
            
            // Get CR2 (fault address)
            "mov rcx, cr2",
            
            "mov rax, 0xB8000",
            
            // Clear screen
            "mov rbx, 0",
            "1:",
            "mov word ptr [rax + rbx*2], 0x0F20",
            "inc rbx",
            "cmp rbx, 2000",
            "jl 1b",
            
            // Line 0: "PAGE FAULT"
            "mov word ptr [rax + 0], 0x4F50",   // Red 'P'
            "mov word ptr [rax + 2], 0x4F41",   // Red 'A'
            "mov word ptr [rax + 4], 0x4F47",   // Red 'G'
            "mov word ptr [rax + 6], 0x4F45",   // Red 'E'
            "mov word ptr [rax + 8], 0x4F20",   // Red ' '
            "mov word ptr [rax + 10], 0x4F46",  // Red 'F'
            "mov word ptr [rax + 12], 0x4F41",  // Red 'A'
            "mov word ptr [rax + 14], 0x4F55",  // Red 'U'
            "mov word ptr [rax + 16], 0x4F4C",  // Red 'L'
            "mov word ptr [rax + 18], 0x4F54",  // Red 'T'
            
            // Line 1: "ERR: " + error code in hex
            "add rax, 160",  // Next line
            "mov word ptr [rax + 0], 0x0F45",   // White 'E'
            "mov word ptr [rax + 2], 0x0F52",   // White 'R'
            "mov word ptr [rax + 4], 0x0F52",   // White 'R'
            "mov word ptr [rax + 6], 0x0F3A",   // White ':'
            "mov word ptr [rax + 8], 0x0F20",   // White ' '
            
            // Print error code (rdx) as hex
            "mov rbx, 10",  // Start at offset 10
            "mov rcx, rdx",
            "call print_hex",
            
            // Line 2: "ADDR: " + CR2 address
            "add rax, 160",  // Next line
            "mov word ptr [rax + 0], 0x0F41",   // White 'A'
            "mov word ptr [rax + 2], 0x0F44",   // White 'D'
            "mov word ptr [rax + 4], 0x0F44",   // White 'D'
            "mov word ptr [rax + 6], 0x0F52",   // White 'R'
            "mov word ptr [rax + 8], 0x0F3A",   // White ':'
            "mov word ptr [rax + 10], 0x0F20",  // White ' '
            
            // Get CR2 value (already in rcx)
            "mov rbx, 12",
            "call print_hex",
            
            "3:",
            "hlt",
            "jmp 3b",
            
            // Helper: print 64-bit hex value in rcx at [rax + rbx*2]
            "print_hex:",
            "push rdi",
            "push rsi",
            "mov rdi, 16",  // 16 hex digits
            "4:",
            "mov rsi, rcx",
            "shr rsi, 60",  // Get top nibble
            "and rsi, 0xF",
            "cmp rsi, 10",
            "jl 5f",
            "add rsi, 55",  // 'A'-10
            "jmp 6f",
            "5:",
            "add rsi, 48",  // '0'
            "6:",
            "mov word ptr [rax + rbx*2], si",
            "or word ptr [rax + rbx*2], 0x0F00",
            "inc rbx",
            "shl rcx, 4",
            "dec rdi",
            "jnz 4b",
            "pop rsi",
            "pop rdi",
            "ret",
        )
    }
}

// Double Fault (#DF) - has error code (always 0)
#[unsafe(naked)]
extern "C" fn double_fault_handler() {
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
            
            // Write "DOUBLE FAULT"
            "mov word ptr [rax + 0], 0x4F44",   // Red 'D'
            "mov word ptr [rax + 2], 0x4F4F",   // Red 'O'
            "mov word ptr [rax + 4], 0x4F55",   // Red 'U'
            "mov word ptr [rax + 6], 0x4F42",   // Red 'B'
            "mov word ptr [rax + 8], 0x4F4C",   // Red 'L'
            "mov word ptr [rax + 10], 0x4F45",  // Red 'E'
            "mov word ptr [rax + 12], 0x4F20",  // Red ' '
            "mov word ptr [rax + 14], 0x4F46",  // Red 'F'
            "mov word ptr [rax + 16], 0x4F41",  // Red 'A'
            "mov word ptr [rax + 18], 0x4F55",  // Red 'U'
            "mov word ptr [rax + 20], 0x4F4C",  // Red 'L'
            "mov word ptr [rax + 22], 0x4F54",  // Red 'T'
            
            "2:",
            "hlt",
            "jmp 2b",
        )
    }
}

// Generic Exception Handler
#[unsafe(naked)]
extern "C" fn generic_exception_handler() {
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
            
            // Write "EXCEPTION"
            "mov word ptr [rax + 0], 0x4F45",   // Red 'E'
            "mov word ptr [rax + 2], 0x4F58",   // Red 'X'
            "mov word ptr [rax + 4], 0x4F43",   // Red 'C'
            "mov word ptr [rax + 6], 0x4F45",   // Red 'E'
            "mov word ptr [rax + 8], 0x4F50",   // Red 'P'
            "mov word ptr [rax + 10], 0x4F54",  // Red 'T'
            "mov word ptr [rax + 12], 0x4F49",  // Red 'I'
            "mov word ptr [rax + 14], 0x4F4F",  // Red 'O'
            "mov word ptr [rax + 16], 0x4F4E",  // Red 'N'
            
            "2:",
            "hlt",
            "jmp 2b",
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
