use alloc::vec::Vec;
use core::arch::asm;

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

fn outl(port: u16, val: u32) {
    unsafe { asm!("out dx, eax", in("dx") port, in("eax") val, options(nomem, nostack)); }
}

fn inl(port: u16) -> u32 {
    let val: u32;
    unsafe { asm!("in eax, dx", in("dx") port, out("eax") val, options(nomem, nostack)); }
    val
}

fn config_address(bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
    (1u32 << 31)
        | ((bus as u32) << 16)
        | ((dev as u32 & 0x1F) << 11)
        | ((func as u32 & 0x07) << 8)
        | ((offset as u32) & 0xFC)
}

pub fn config_read32(bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
    outl(CONFIG_ADDRESS, config_address(bus, dev, func, offset));
    inl(CONFIG_DATA)
}

pub fn config_read16(bus: u8, dev: u8, func: u8, offset: u8) -> u16 {
    let dword = config_read32(bus, dev, func, offset & 0xFC);
    ((dword >> ((offset & 2) * 8)) & 0xFFFF) as u16
}

pub fn config_read8(bus: u8, dev: u8, func: u8, offset: u8) -> u8 {
    let dword = config_read32(bus, dev, func, offset & 0xFC);
    ((dword >> ((offset & 3) * 8)) & 0xFF) as u8
}

pub fn config_write32(bus: u8, dev: u8, func: u8, offset: u8, val: u32) {
    outl(CONFIG_ADDRESS, config_address(bus, dev, func, offset));
    outl(CONFIG_DATA, val);
}

#[derive(Clone)]
pub struct PciDevice {
    pub bus: u8,
    pub dev: u8,
    pub func: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub revision: u8,
    pub header_type: u8,
    pub irq_line: u8,
    pub irq_pin: u8,
    pub bars: [u32; 6],
}

#[derive(Clone, Copy)]
pub enum BarInfo {
    None,
    Io { port: u16 },
    Memory32 { base: u32, size: u32 },
    Memory64 { base: u64, size: u64 },
}

impl PciDevice {
    fn read(bus: u8, dev: u8, func: u8) -> Option<Self> {
        let vendor_id = config_read16(bus, dev, func, 0x00);
        if vendor_id == 0xFFFF { return None; }

        let device_id = config_read16(bus, dev, func, 0x02);
        let class = config_read8(bus, dev, func, 0x0B);
        let subclass = config_read8(bus, dev, func, 0x0A);
        let prog_if = config_read8(bus, dev, func, 0x09);
        let revision = config_read8(bus, dev, func, 0x08);
        let header_type = config_read8(bus, dev, func, 0x0E);
        let irq_line = config_read8(bus, dev, func, 0x3C);
        let irq_pin = config_read8(bus, dev, func, 0x3D);

        let mut bars = [0u32; 6];
        if (header_type & 0x7F) == 0 {
            for i in 0..6 {
                bars[i] = config_read32(bus, dev, func, 0x10 + (i as u8) * 4);
            }
        }

        Some(PciDevice {
            bus, dev, func, vendor_id, device_id,
            class, subclass, prog_if, revision,
            header_type, irq_line, irq_pin, bars,
        })
    }

    pub fn bar_info(&self, index: usize) -> BarInfo {
        if index >= 6 { return BarInfo::None; }
        let raw = self.bars[index];
        if raw == 0 { return BarInfo::None; }

        if (raw & 1) != 0 {
            return BarInfo::Io { port: (raw & 0xFFFC) as u16 };
        }

        let mem_type = (raw >> 1) & 0x3;
        match mem_type {
            0x00 => {
                let base = raw & 0xFFFFFFF0;
                let size = bar_size_32(self.bus, self.dev, self.func, 0x10 + (index as u8) * 4);
                BarInfo::Memory32 { base, size }
            }
            0x02 => {
                if index + 1 >= 6 { return BarInfo::None; }
                let low = raw & 0xFFFFFFF0;
                let high = self.bars[index + 1];
                let base = ((high as u64) << 32) | (low as u64);
                let size = bar_size_64(
                    self.bus, self.dev, self.func,
                    0x10 + (index as u8) * 4,
                    0x10 + (index as u8 + 1) * 4,
                );
                BarInfo::Memory64 { base, size }
            }
            _ => BarInfo::None,
        }
    }

    pub fn is_multifunction(&self) -> bool { (self.header_type & 0x80) != 0 }
}

fn bar_size_32(bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
    let original = config_read32(bus, dev, func, offset);
    config_write32(bus, dev, func, offset, 0xFFFFFFFF);
    let readback = config_read32(bus, dev, func, offset);
    config_write32(bus, dev, func, offset, original);
    if readback == 0 { return 0; }
    let mask = readback & 0xFFFFFFF0;
    (!mask).wrapping_add(1)
}

fn bar_size_64(bus: u8, dev: u8, func: u8, off_lo: u8, off_hi: u8) -> u64 {
    let orig_lo = config_read32(bus, dev, func, off_lo);
    let orig_hi = config_read32(bus, dev, func, off_hi);
    config_write32(bus, dev, func, off_lo, 0xFFFFFFFF);
    config_write32(bus, dev, func, off_hi, 0xFFFFFFFF);
    let rb_lo = config_read32(bus, dev, func, off_lo);
    let rb_hi = config_read32(bus, dev, func, off_hi);
    config_write32(bus, dev, func, off_lo, orig_lo);
    config_write32(bus, dev, func, off_hi, orig_hi);
    let mask = ((rb_hi as u64) << 32) | ((rb_lo & 0xFFFFFFF0) as u64);
    if mask == 0 { return 0; }
    (!mask).wrapping_add(1)
}

pub fn enumerate() -> Vec<PciDevice> {
    let mut devices = Vec::new();

    let host = config_read8(0, 0, 0, 0x0E);
    let max_bus: u8 = if (host & 0x80) != 0 { 8 } else { 1 };

    for bus in 0..max_bus {
        for dev in 0..32u8 {
            if let Some(d) = PciDevice::read(bus, dev, 0) {
                let multifunction = d.is_multifunction();
                devices.push(d);
                if multifunction {
                    for func in 1..8u8 {
                        if let Some(d) = PciDevice::read(bus, dev, func) {
                            devices.push(d);
                        }
                    }
                }
            }
        }
    }

    devices
}

pub fn class_name(class: u8, subclass: u8) -> &'static str {
    match (class, subclass) {
        (0x00, 0x00) => "Non-VGA Unclassified",
        (0x00, 0x01) => "VGA-Compatible Unclassified",
        (0x01, 0x00) => "SCSI Bus Controller",
        (0x01, 0x01) => "IDE Controller",
        (0x01, 0x02) => "Floppy Disk Controller",
        (0x01, 0x05) => "ATA Controller",
        (0x01, 0x06) => "SATA Controller",
        (0x01, 0x08) => "NVMe Controller",
        (0x02, 0x00) => "Ethernet Controller",
        (0x02, 0x80) => "Other Network Controller",
        (0x03, 0x00) => "VGA Controller",
        (0x03, 0x01) => "XGA Controller",
        (0x04, 0x00) => "Video Controller",
        (0x04, 0x01) => "Audio Controller",
        (0x04, 0x03) => "HD Audio Controller",
        (0x05, 0x00) => "RAM Controller",
        (0x06, 0x00) => "Host Bridge",
        (0x06, 0x01) => "ISA Bridge",
        (0x06, 0x02) => "EISA Bridge",
        (0x06, 0x04) => "PCI-to-PCI Bridge",
        (0x06, 0x80) => "Other Bridge",
        (0x07, 0x00) => "Serial Controller",
        (0x07, 0x01) => "Parallel Controller",
        (0x08, 0x00) => "PIC",
        (0x08, 0x01) => "DMA Controller",
        (0x08, 0x02) => "Timer",
        (0x08, 0x03) => "RTC Controller",
        (0x0C, 0x00) => "FireWire Controller",
        (0x0C, 0x03) => "USB Controller",
        (0x0C, 0x05) => "SMBus Controller",
        _ => "Unknown",
    }
}

pub fn display_devices(devices: &[PciDevice]) {
    use crate::{vga, serial};

    vga::print("  Found ");
    vga::print_num(devices.len() as u64);
    vga::println(" PCI devices:");
    vga::println("");

    serial::print("PCI: ");
    serial::print_num(devices.len() as u64);
    serial::println(" devices found");

    vga::println("  Bus Dev Fn  Vendor:Device  Class  Description");
    vga::println("  --- --- --  -------------  -----  -----------");

    for d in devices {
        vga::print("  ");
        print_padded_num(d.bus as u64, 3);
        print_padded_num(d.dev as u64, 4);
        print_padded_num(d.func as u64, 3);
        vga::print("  ");
        print_hex16(d.vendor_id);
        vga::print(":");
        print_hex16(d.device_id);
        vga::print("  ");
        print_hex8(d.class);
        print_hex8(d.subclass);
        vga::print("  ");
        vga::println(class_name(d.class, d.subclass));

        serial::print("  ");
        serial::print_num(d.bus as u64); serial::print(":");
        serial::print_num(d.dev as u64); serial::print(".");
        serial::print_num(d.func as u64); serial::print(" ");
        serial::print_hex(((d.vendor_id as u64) << 16) | d.device_id as u64);
        serial::print(" ");
        serial::println(class_name(d.class, d.subclass));

        for i in 0..6 {
            match d.bar_info(i) {
                BarInfo::Io { port } => {
                    serial::print("    BAR");
                    serial::print_num(i as u64);
                    serial::print(": I/O port ");
                    serial::print_hex(port as u64);
                    serial::println("");
                }
                BarInfo::Memory32 { base, size } if size > 0 => {
                    serial::print("    BAR");
                    serial::print_num(i as u64);
                    serial::print(": MMIO32 ");
                    serial::print_hex(base as u64);
                    serial::print(" size=");
                    serial::print_hex(size as u64);
                    serial::println("");
                }
                BarInfo::Memory64 { base, size } if size > 0 => {
                    serial::print("    BAR");
                    serial::print_num(i as u64);
                    serial::print(": MMIO64 ");
                    serial::print_hex(base);
                    serial::print(" size=");
                    serial::print_hex(size);
                    serial::println("");
                }
                _ => {}
            }
        }
    }
}

fn print_hex16(val: u16) {
    for i in (0..4).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as u8;
        let ch = if nibble < 10 { b'0' + nibble } else { b'A' + (nibble - 10) };
        crate::vga::print_char(ch as char);
    }
}

fn print_hex8(val: u8) {
    for i in (0..2).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as u8;
        let ch = if nibble < 10 { b'0' + nibble } else { b'A' + (nibble - 10) };
        crate::vga::print_char(ch as char);
    }
}

fn print_padded_num(n: u64, width: usize) {
    let mut buf = [b' '; 8];
    let mut val = n;
    let mut pos = buf.len();
    if val == 0 {
        pos -= 1;
        buf[pos] = b'0';
    } else {
        while val > 0 && pos > 0 {
            pos -= 1;
            buf[pos] = b'0' + (val % 10) as u8;
            val /= 10;
        }
    }
    let digits = buf.len() - pos;
    let padding = if width > digits { width - digits } else { 0 };
    for _ in 0..padding { crate::vga::print_char(' '); }
    for i in pos..buf.len() { crate::vga::print_char(buf[i] as char); }
}
