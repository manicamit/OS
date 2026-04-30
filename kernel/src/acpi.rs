use crate::{vga, serial};

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct AcpiTableHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct Fadt {
    header: AcpiTableHeader,
    firmware_ctrl: u32,
    dsdt: u32,
    _reserved: u8,
    preferred_pm_profile: u8,
    sci_interrupt: u16,
    smi_command_port: u32,
    acpi_enable: u8,
    acpi_disable: u8,
    s4bios_req: u8,
    pstate_cnt: u8,
    pm1a_evt_blk: u32,
    pm1b_evt_blk: u32,
    pm1a_cnt_blk: u32,
    pm1b_cnt_blk: u32,
}

#[repr(C, packed)]
struct MadtEntryHeader {
    entry_type: u8,
    length: u8,
}

#[repr(C, packed)]
struct MadtLocalApic {
    _header: MadtEntryHeader,
    processor_id: u8,
    apic_id: u8,
    flags: u32,
}

#[repr(C, packed)]
struct MadtIoApic {
    _header: MadtEntryHeader,
    io_apic_id: u8,
    _reserved: u8,
    io_apic_address: u32,
    global_irq_base: u32,
}

#[derive(Clone, Copy)]
pub struct AcpiInfo {
    pub valid: bool,
    pub cpu_count: u8,
    pub io_apic_addr: u32,
    pub local_apic_addr: u32,
    pub pm1a_cnt_blk: u16,
    pub sci_interrupt: u16,
    pub pm_profile: u8,
    pub rsdt_addr: u32,
    pub table_count: u8,
}

impl AcpiInfo {
    pub const fn empty() -> Self {
        AcpiInfo {
            valid: false, cpu_count: 0, io_apic_addr: 0, local_apic_addr: 0,
            pm1a_cnt_blk: 0, sci_interrupt: 0, pm_profile: 0, rsdt_addr: 0, table_count: 0,
        }
    }
}

static mut ACPI_INFO: AcpiInfo = AcpiInfo::empty();

pub fn get_info() -> AcpiInfo {
    unsafe { core::ptr::read_volatile(&ACPI_INFO) }
}

fn verify_checksum(addr: *const u8, len: usize) -> bool {
    let mut sum: u8 = 0;
    for i in 0..len {
        sum = sum.wrapping_add(unsafe { *addr.add(i) });
    }
    sum == 0
}

pub fn parse_from_rsdt(rsdt_address: u32) {
    let mut info = AcpiInfo::empty();
    info.rsdt_addr = rsdt_address;

    unsafe {
        let header = &*(rsdt_address as *const AcpiTableHeader);
        if !verify_checksum(rsdt_address as *const u8, header.length as usize) {
            return;
        }

        let header_size = core::mem::size_of::<AcpiTableHeader>();
        let entries_size = header.length as usize - header_size;
        let num_entries = entries_size / 4;
        info.table_count = num_entries as u8;

        let entries_ptr = (rsdt_address as usize + header_size) as *const u32;

        for i in 0..num_entries {
            let table_addr = *entries_ptr.add(i);
            let table_header = &*(table_addr as *const AcpiTableHeader);

            match &table_header.signature {
                b"FACP" => parse_fadt(table_addr, &mut info),
                b"APIC" => parse_madt(table_addr, &mut info),
                _ => {}
            }
        }
    }

    info.valid = true;
    unsafe { core::ptr::write_volatile(&mut ACPI_INFO, info); }
}

fn parse_fadt(addr: u32, info: &mut AcpiInfo) {
    unsafe {
        let fadt = &*(addr as *const Fadt);
        info.pm1a_cnt_blk = fadt.pm1a_cnt_blk as u16;
        info.sci_interrupt = fadt.sci_interrupt;
        info.pm_profile = fadt.preferred_pm_profile;
    }
}

fn parse_madt(addr: u32, info: &mut AcpiInfo) {
    unsafe {
        let header = &*(addr as *const AcpiTableHeader);
        let madt_base = addr as usize;

        let local_apic_addr = *((madt_base + 36) as *const u32);
        info.local_apic_addr = local_apic_addr;

        let header_size = 36 + 8;
        let mut offset = header_size;
        let total = header.length as usize;

        while offset + 2 <= total {
            let entry = &*((madt_base + offset) as *const MadtEntryHeader);
            if entry.length < 2 { break; }

            match entry.entry_type {
                0 => {
                    let lapic = &*((madt_base + offset) as *const MadtLocalApic);
                    if lapic.flags & 1 != 0 { info.cpu_count += 1; }
                }
                1 => {
                    let ioapic = &*((madt_base + offset) as *const MadtIoApic);
                    if info.io_apic_addr == 0 { info.io_apic_addr = ioapic.io_apic_address; }
                }
                _ => {}
            }

            offset += entry.length as usize;
        }
    }
}

pub fn display_info() {
    let info = get_info();
    if !info.valid {
        vga::println("  ACPI: Not available");
        return;
    }

    vga::print("  RSDT at:          "); vga::print_hex(info.rsdt_addr as u64); vga::println("");
    vga::print("  ACPI tables:      "); vga::print_num(info.table_count as u64); vga::println("");
    vga::print("  CPU cores:        "); vga::print_num(info.cpu_count as u64); vga::println("");

    vga::print("  Local APIC:       "); vga::print_hex(info.local_apic_addr as u64); vga::println("");
    vga::print("  IO APIC:          "); vga::print_hex(info.io_apic_addr as u64); vga::println("");

    vga::print("  PM1a Control:     0x"); print_hex16(info.pm1a_cnt_blk); vga::println("");
    vga::print("  SCI Interrupt:    IRQ"); vga::print_num(info.sci_interrupt as u64); vga::println("");

    vga::print("  PM Profile:       ");
    vga::println(match info.pm_profile {
        0 => "Unspecified", 1 => "Desktop", 2 => "Mobile",
        3 => "Workstation", 4 => "Enterprise Server", 5 => "SOHO Server",
        6 => "Appliance", 7 => "Performance Server", 8 => "Tablet",
        _ => "Unknown",
    });

    serial::print("ACPI: ");
    serial::print_num(info.cpu_count as u64);
    serial::print(" CPUs, IO APIC @ ");
    serial::print_hex(info.io_apic_addr as u64);
    serial::print(", PM1a=0x");
    serial_hex16(info.pm1a_cnt_blk);
    serial::println("");
}

pub fn shutdown() -> ! {
    let info = get_info();
    if info.pm1a_cnt_blk != 0 {
        serial::println("ACPI: Initiating shutdown...");
        unsafe {
            // SLP_TYPa=5 (S5 sleep state) | SLP_EN (bit 13)
            let val: u16 = (5 << 10) | (1 << 13);
            core::arch::asm!(
                "out dx, ax",
                in("dx") info.pm1a_cnt_blk,
                in("ax") val,
                options(nomem, nostack)
            );
        }
    }
    serial::println("ACPI: Shutdown failed, halting");
    loop { unsafe { core::arch::asm!("cli; hlt"); } }
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
