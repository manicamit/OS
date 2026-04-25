#![allow(dead_code)]

use core::mem::size_of;
use core::arch::naked_asm;

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
        Self { offset_low: 0, selector: 0, ist: 0, attributes: 0, offset_mid: 0, offset_high: 0, zero: 0 }
    }

    fn set_handler(&mut self, handler: u64) {
        self.offset_low = handler as u16;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
        self.selector = 0x18;
        self.ist = 0;
        self.attributes = 0x8E;
        self.zero = 0;
    }
}

#[repr(C, align(16))]
struct Idt {
    entries: [IdtEntry; 256],
}

impl Idt {
    const fn new() -> Self {
        Self { entries: [IdtEntry::missing(); 256] }
    }

    fn set_handler(&mut self, index: usize, handler: u64) {
        self.entries[index].set_handler(handler);
    }

    fn load(&'static self) {
        let ptr = IdtPointer {
            limit: (size_of::<Self>() - 1) as u16,
            base: self as *const _ as u64,
        };
        unsafe { core::arch::asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack)); }
    }
}

#[repr(C, packed)]
struct IdtPointer {
    limit: u16,
    base: u64,
}

static mut VGA_ADDR_FOR_IDT: u64 = 0xB8000;

pub fn switch_to_higher_half_vga() {
    unsafe { VGA_ADDR_FOR_IDT = crate::vga::VGA_HIGHER_HALF; }
}

fn exception_common(name: &str, _has_error_code: bool) {
    crate::serial::println("");
    crate::serial::print("!!! EXCEPTION: ");
    crate::serial::println(name);

    let vga_addr = unsafe { VGA_ADDR_FOR_IDT };
    unsafe {
        let vga = vga_addr as *mut u16;
        let msg = name.as_bytes();
        for (i, &b) in msg.iter().enumerate() {
            if i >= 80 { break; }
            core::ptr::write_volatile(vga.add(i), 0x4F00 | b as u16);
        }
    }
}

#[unsafe(naked)]
extern "C" fn divide_by_zero_handler() {
    unsafe {
        naked_asm!(
            "cli",
            "mov word ptr [rax + 22], 0x4F45",
            "mov word ptr [rax + 24], 0x4F52",
            "mov word ptr [rax + 26], 0x4F4F",
            "2:", "hlt", "jmp 2b",
        )
    }
}

#[unsafe(naked)]
extern "C" fn general_protection_fault_handler() {
    unsafe {
        naked_asm!(
            "cli",
            "mov dx, 0x3FD",
            "1:", "in al, dx", "test al, 0x20", "jz 1b",
            "mov dx, 0x3F8", "mov al, 0x47", "out dx, al",
            "mov dx, 0x3FD",
            "2:", "in al, dx", "test al, 0x20", "jz 2b",
            "mov dx, 0x3F8", "mov al, 0x50", "out dx, al",
            "mov dx, 0x3FD",
            "3:", "in al, dx", "test al, 0x20", "jz 3b",
            "mov dx, 0x3F8", "mov al, 0x21", "out dx, al",
            "mov dx, 0x3FD",
            "4:", "in al, dx", "test al, 0x20", "jz 4b",
            "mov dx, 0x3F8", "mov al, 0x0A", "out dx, al",
            "5:", "hlt", "jmp 5b",
        )
    }
}

#[unsafe(naked)]
extern "C" fn page_fault_handler() {
    unsafe {
        naked_asm!(
            "cli",
            "pop r8",
            "mov r9, cr2",
            "mov dx, 0x3FD",
            "2:", "in al, dx", "test al, 0x20", "jz 2b",
            "mov dx, 0x3F8", "mov al, 0x50", "out dx, al",
            "mov dx, 0x3FD",
            "3:", "in al, dx", "test al, 0x20", "jz 3b",
            "mov dx, 0x3F8", "mov al, 0x46", "out dx, al",
            "mov dx, 0x3FD",
            "4:", "in al, dx", "test al, 0x20", "jz 4b",
            "mov dx, 0x3F8", "mov al, 0x20", "out dx, al",
            "mov dx, 0x3FD",
            "44:", "in al, dx", "test al, 0x20", "jz 44b",
            "mov dx, 0x3F8", "mov al, 0x40", "out dx, al",
            "mov rcx, r9",
            "mov rbx, 60",
            "10:",
            "mov rax, rcx", "mov cl, bl", "shr rax, cl", "and rax, 0xF",
            "cmp al, 10", "jl 11f", "add al, 55", "jmp 12f",
            "11:", "add al, 48",
            "12:",
            "mov dx, 0x3FD",
            "13:", "in al, dx", "test al, 0x20", "jz 13b",
            "mov rax, r9", "push rcx", "mov cl, bl", "shr rax, cl", "pop rcx",
            "and rax, 0xF", "cmp al, 10", "jl 14f", "add al, 55", "jmp 15f",
            "14:", "add al, 48",
            "15:",
            "mov dx, 0x3F8", "out dx, al",
            "sub bl, 4", "cmp bl, -4", "jg 10b",
            "mov dx, 0x3FD",
            "20:", "in al, dx", "test al, 0x20", "jz 20b",
            "mov dx, 0x3F8", "mov al, 0x0A", "out dx, al",
            "16:", "hlt", "jmp 16b",
        )
    }
}

#[unsafe(naked)]
extern "C" fn double_fault_handler() {
    unsafe {
        naked_asm!(
            "cli",
            "mov dx, 0x3FD",
            "1:", "in al, dx", "test al, 0x20", "jz 1b",
            "mov dx, 0x3F8", "mov al, 0x44", "out dx, al",
            "mov dx, 0x3FD",
            "2:", "in al, dx", "test al, 0x20", "jz 2b",
            "mov dx, 0x3F8", "mov al, 0x46", "out dx, al",
            "mov dx, 0x3FD",
            "3:", "in al, dx", "test al, 0x20", "jz 3b",
            "mov dx, 0x3F8", "mov al, 0x21", "out dx, al",
            "mov dx, 0x3FD",
            "4:", "in al, dx", "test al, 0x20", "jz 4b",
            "mov dx, 0x3F8", "mov al, 0x0A", "out dx, al",
            "5:", "hlt", "jmp 5b",
        )
    }
}

#[unsafe(naked)]
extern "C" fn generic_exception_handler() {
    unsafe {
        naked_asm!(
            "cli",
            "mov dx, 0x3FD",
            "1:", "in al, dx", "test al, 0x20", "jz 1b",
            "mov dx, 0x3F8", "mov al, 0x45", "out dx, al",
            "mov dx, 0x3FD",
            "2:", "in al, dx", "test al, 0x20", "jz 2b",
            "mov dx, 0x3F8", "mov al, 0x58", "out dx, al",
            "mov dx, 0x3FD",
            "3:", "in al, dx", "test al, 0x20", "jz 3b",
            "mov dx, 0x3F8", "mov al, 0x21", "out dx, al",
            "mov dx, 0x3FD",
            "4:", "in al, dx", "test al, 0x20", "jz 4b",
            "mov dx, 0x3F8", "mov al, 0x0A", "out dx, al",
            "5:", "hlt", "jmp 5b",
        )
    }
}

#[unsafe(naked)]
extern "C" fn irq0_timer_handler() {
    unsafe {
        naked_asm!(
            "push rax", "push rcx", "push rdx", "push rsi", "push rdi",
            "push r8", "push r9", "push r10", "push r11",
            "call {tick}",
            "mov al, 0x20", "out 0x20, al",
            "pop r11", "pop r10", "pop r9", "pop r8", "pop rdi",
            "pop rsi", "pop rdx", "pop rcx", "pop rax",
            "iretq",
            tick = sym crate::pit::tick,
        )
    }
}

#[unsafe(naked)]
extern "C" fn irq1_keyboard_handler() {
    unsafe {
        naked_asm!(
            "push rax", "push rcx", "push rdx", "push rsi", "push rdi",
            "push r8", "push r9", "push r10", "push r11",
            "call {kbd}",
            "mov al, 0x20", "out 0x20, al",
            "pop r11", "pop r10", "pop r9", "pop r8", "pop rdi",
            "pop rsi", "pop rdx", "pop rcx", "pop rax",
            "iretq",
            kbd = sym crate::keyboard::handle_scancode,
        )
    }
}

static mut IDT: Idt = Idt::new();

fn register_all_handlers() {
    unsafe {
        let idt_ptr = core::ptr::addr_of_mut!(IDT);
        for i in 0..32 {
            (*idt_ptr).set_handler(i, generic_exception_handler as u64);
        }
        (*idt_ptr).set_handler(0, divide_by_zero_handler as u64);
        (*idt_ptr).set_handler(8, double_fault_handler as u64);
        (*idt_ptr).set_handler(13, general_protection_fault_handler as u64);
        (*idt_ptr).set_handler(14, page_fault_handler as u64);
        (*idt_ptr).set_handler(32, irq0_timer_handler as u64);
        (*idt_ptr).set_handler(33, irq1_keyboard_handler as u64);
    }
}

pub fn init() {
    register_all_handlers();
    unsafe { (*core::ptr::addr_of!(IDT)).load(); }
}

pub fn reload_idt() {
    register_all_handlers();
    unsafe { (*core::ptr::addr_of!(IDT)).load(); }
}

pub fn get_pf_handler_addr() -> u64 {
    page_fault_handler as u64
}
