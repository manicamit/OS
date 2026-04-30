use crate::{vga, serial, pci};

fn inb(port: u16) -> u8 {
    let val: u8;
    unsafe { core::arch::asm!("in al, dx", in("dx") port, out("al") val, options(nomem, nostack)); }
    val
}

fn probe_port(port: u16) -> bool {
    let val = inb(port);
    val != 0xFF
}

fn probe_serial(base: u16) -> bool {
    let iir = inb(base + 2);
    let lsr = inb(base + 5);
    iir != 0xFF && lsr != 0xFF && (lsr & 0x60) != 0
}

struct LpcDevice {
    name: &'static str,
    port: u16,
    present: bool,
    detail: &'static str,
}

pub fn scan_and_display() {
    vga::println("  LPC/ISA Legacy Device Probe:");
    vga::println("");

    let devices = [
        probe_device("PIC (Master)",     0x20, |_| { let mask = inb(0x21); outb_safe(0x21, mask); inb(0x21) == mask }, "IRQ controller"),
        probe_device("PIC (Slave)",      0xA0, |_| { let mask = inb(0xA1); outb_safe(0xA1, mask); inb(0xA1) == mask }, "IRQ controller (cascade)"),
        probe_device("PIT 8253",         0x40, |_| { let _ = inb(0x40); probe_port(0x43) }, "System timer"),
        probe_device("PS/2 Controller",  0x64, |_| { let status = inb(0x64); status != 0xFF }, "Keyboard/mouse"),
        probe_device("COM1",             0x3F8, |p| probe_serial(p), "Serial port"),
        probe_device("COM2",             0x2F8, |p| probe_serial(p), "Serial port"),
        probe_device("COM3",             0x3E8, |p| probe_serial(p), "Serial port"),
        probe_device("COM4",             0x2E8, |p| probe_serial(p), "Serial port"),
        probe_device("LPT1",             0x378, |p| probe_port(p), "Parallel port"),
        probe_device("RTC/CMOS",         0x70, |_| { outb_safe(0x70, 0x0D); let v = inb(0x71); v != 0xFF }, "Real-time clock"),
        probe_device("DMA Controller 1", 0x00, |_| probe_port(0x08), "8237 DMA channels 0-3"),
        probe_device("DMA Controller 2", 0xC0, |_| probe_port(0xD0), "8237 DMA channels 4-7"),
        probe_device("PC Speaker",       0x61, |p| probe_port(p), "System speaker control"),
    ];

    let mut found = 0u32;
    for d in &devices {
        let status = if d.present { "FOUND" } else { "-----" };
        vga::print("    ");
        vga::print(d.name);
        pad_to(d.name.len(), 20);
        vga::print("0x");
        print_hex16(d.port);
        vga::print("  ");
        vga::print(status);
        vga::print("  ");
        vga::println(d.detail);

        serial::print("  LPC ");
        serial::print(d.name);
        serial::print(" @ 0x");
        serial_hex16(d.port);
        serial::print(": ");
        serial::println(status);

        if d.present { found += 1; }
    }

    vga::println("");
    vga::print("  ");
    vga::print_num(found as u64);
    vga::print("/");
    vga::print_num(devices.len() as u64);
    vga::println(" legacy devices detected");

    read_lpc_bridge_config();
}

fn probe_device(name: &'static str, port: u16, test: impl Fn(u16) -> bool, detail: &'static str) -> LpcDevice {
    LpcDevice { name, port, present: test(port), detail }
}

fn read_lpc_bridge_config() {
    let vendor = pci::config_read16(0, 31, 0, 0x00);
    if vendor == 0xFFFF { return; }

    vga::println("");
    vga::println("  ICH9 LPC Bridge Config:");

    let lpc_io_dec = pci::config_read16(0, 31, 0, 0x80);
    serial::print("  LPC IO Decode: ");
    serial::print_hex(lpc_io_dec as u64);
    serial::println("");

    let com_decode = (lpc_io_dec >> 0) & 0x07;
    let lpt_decode = (lpc_io_dec >> 4) & 0x03;

    let com_str = match com_decode {
        0 => "0x3F8 (COM1)", 1 => "0x2F8 (COM2)",
        2 => "0x220", 3 => "0x228",
        4 => "0x238", 5 => "0x2E8 (COM3)",
        6 => "0x338", _ => "0x3E8 (COM4)",
    };
    vga::print("    COM decode: "); vga::println(com_str);

    let lpt_str = match lpt_decode {
        0 => "0x378 (LPT1)", 1 => "0x278 (LPT2)",
        2 => "0x3BC", _ => "Reserved",
    };
    vga::print("    LPT decode: "); vga::println(lpt_str);

    let acpi_base = pci::config_read16(0, 31, 0, 0x40) & 0xFFF8;
    if acpi_base != 0 {
        vga::print("    ACPI base:  0x");
        print_hex16(acpi_base);
        vga::println("");
    }

    let gen_dec1 = pci::config_read32(0, 31, 0, 0x84);
    let gen_dec2 = pci::config_read32(0, 31, 0, 0x88);
    if gen_dec1 & 1 != 0 {
        let base = (gen_dec1 >> 16) & 0xFFFC;
        vga::print("    GenDec1:    0x");
        print_hex16(base as u16);
        vga::println(" (enabled)");
    }
    if gen_dec2 & 1 != 0 {
        let base = (gen_dec2 >> 16) & 0xFFFC;
        vga::print("    GenDec2:    0x");
        print_hex16(base as u16);
        vga::println(" (enabled)");
    }
}

fn outb_safe(port: u16, val: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack)); }
}

fn pad_to(current: usize, target: usize) {
    if current < target {
        for _ in 0..(target - current) { vga::print_char(' '); }
    }
}

fn print_hex16(val: u16) {
    for i in (0..4).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as u8;
        let ch = if nibble < 10 { b'0' + nibble } else { b'A' + (nibble - 10) };
        vga::print_char(ch as char);
    }
}

fn serial_hex16(val: u16) {
    for i in (0..4).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as u8;
        let ch = if nibble < 10 { b'0' + nibble } else { b'A' + (nibble - 10) };
        serial::write_byte(ch);
    }
}
