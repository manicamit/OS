// kernel/src/acpi.rs
//! ACPI (Advanced Configuration and Power Interface) table parser

use crate::vga;

/// ACPI Table Header (common to all ACPI tables)
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct AcpiTableHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

/// RSDT (Root System Description Table)
#[repr(C, packed)]
pub struct Rsdt {
    pub header: AcpiTableHeader,
    // Followed by array of 32-bit pointers
}

/// XSDT (Extended System Description Table) - 64-bit version
#[repr(C, packed)]
pub struct Xsdt {
    pub header: AcpiTableHeader,
    // Followed by array of 64-bit pointers
}

/// FADT (Fixed ACPI Description Table)
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Fadt {
    pub header: AcpiTableHeader,
    pub firmware_ctrl: u32,
    pub dsdt: u32,
    pub reserved: u8,
    pub preferred_pm_profile: u8,
    pub sci_interrupt: u16,
    pub smi_command_port: u32,
    pub acpi_enable: u8,
    pub acpi_disable: u8,
    // ... many more fields, but we'll keep it simple
}

/// MADT (Multiple APIC Description Table)
#[repr(C, packed)]
pub struct Madt {
    pub header: AcpiTableHeader,
    pub local_apic_address: u32,
    pub flags: u32,
    // Followed by variable-length interrupt controller structures
}

/// MADT Entry Types
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MadtEntryType {
    LocalApic = 0,
    IoApic = 1,
    InterruptOverride = 2,
    NmiSource = 3,
    LocalApicNmi = 4,
    Unknown = 0xFF,
}

impl From<u8> for MadtEntryType {
    fn from(val: u8) -> Self {
        match val {
            0 => MadtEntryType::LocalApic,
            1 => MadtEntryType::IoApic,
            2 => MadtEntryType::InterruptOverride,
            3 => MadtEntryType::NmiSource,
            4 => MadtEntryType::LocalApicNmi,
            _ => MadtEntryType::Unknown,
        }
    }
}

/// MADT Entry Header
#[repr(C, packed)]
struct MadtEntryHeader {
    entry_type: u8,
    length: u8,
}

/// MADT Local APIC Entry
#[repr(C, packed)]
struct MadtLocalApic {
    header: MadtEntryHeader,
    processor_id: u8,
    apic_id: u8,
    flags: u32,
}

/// MADT IO APIC Entry
#[repr(C, packed)]
struct MadtIoApic {
    header: MadtEntryHeader,
    io_apic_id: u8,
    reserved: u8,
    io_apic_address: u32,
    global_system_interrupt_base: u32,
}

/// Verify ACPI table checksum
fn verify_checksum(header: &AcpiTableHeader) -> bool {
    unsafe {
        let ptr = header as *const AcpiTableHeader as *const u8;
        let length = header.length as usize;
        let bytes = core::slice::from_raw_parts(ptr, length);
        let sum: u8 = bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        sum == 0
    }
}

/// Parse RSDT and display all tables
pub fn parse_rsdt(rsdt_address: u32) {
    unsafe {
        let rsdt = &*(rsdt_address as *const Rsdt);
        
        vga::println("=== RSDT (Root System Description Table) ===");
        
        // Verify checksum
        if !verify_checksum(&rsdt.header) {
            vga::println("ERROR: RSDT checksum failed!");
            return;
        }
        
        vga::print("  Signature: ");
        vga::print_ascii(&rsdt.header.signature);
        vga::println("");
        
        vga::print("  Length: ");
        vga::print_num(rsdt.header.length as u64);
        vga::println(" bytes");
        
        vga::print("  OEM ID: ");
        vga::print_ascii(&rsdt.header.oem_id);
        vga::println("");
        
        // Calculate number of table pointers
        let header_size = core::mem::size_of::<AcpiTableHeader>();
        let entries_size = rsdt.header.length as usize - header_size;
        let num_entries = entries_size / 4; // 32-bit pointers
        
        vga::print("  Tables: ");
        vga::print_num(num_entries as u64);
        vga::println("");
        vga::println("");
        
        // Get pointer to first entry (right after header)
        let entries_ptr = (rsdt_address + header_size as u32) as *const u32;
        let entries = core::slice::from_raw_parts(entries_ptr, num_entries);
        
        // Parse each table
        for (i, &table_addr) in entries.iter().enumerate() {
            let table_header = &*(table_addr as *const AcpiTableHeader);
            
            vga::print("  [");
            vga::print_num(i as u64);
            vga::print("] ");
            vga::print_ascii(&table_header.signature);
            vga::print(" at ");
            vga::print_hex(table_addr as u64);
            
            // Verify checksum
            if verify_checksum(table_header) {
                vga::print(" [OK]");
            } else {
                vga::print(" [BAD CHECKSUM]");
            }
            vga::println("");
            
            // Parse specific tables
            match &table_header.signature {
                b"FACP" => parse_fadt(table_addr),
                b"APIC" => parse_madt(table_addr),
                _ => {}
            }
        }
    }
}

/// Parse FADT (Fixed ACPI Description Table)
fn parse_fadt(fadt_address: u32) {
    unsafe {
        let fadt = &*(fadt_address as *const Fadt);
        
        vga::println("    === FADT (Fixed ACPI Description Table) ===");
        
        vga::print("      DSDT Address: ");
        vga::print_hex(fadt.dsdt as u64);
        vga::println("");
        
        vga::print("      Preferred PM Profile: ");
        match fadt.preferred_pm_profile {
            0 => vga::println("Unspecified"),
            1 => vga::println("Desktop"),
            2 => vga::println("Mobile"),
            3 => vga::println("Workstation"),
            4 => vga::println("Enterprise Server"),
            5 => vga::println("SOHO Server"),
            6 => vga::println("Appliance PC"),
            7 => vga::println("Performance Server"),
            8 => vga::println("Tablet"),
            _ => vga::println("Unknown"),
        }
        
        vga::print("      SCI Interrupt: ");
        vga::print_num(fadt.sci_interrupt as u64);
        vga::println("");
    }
}

/// Parse MADT (Multiple APIC Description Table)
fn parse_madt(madt_address: u32) {
    unsafe {
        let madt = &*(madt_address as *const Madt);
        
        vga::println("    === MADT (Multiple APIC Description Table) ===");
        
        vga::print("      Local APIC Address: ");
        vga::print_hex(madt.local_apic_address as u64);
        vga::println("");
        
        vga::print("      Flags: ");
        vga::print_hex(madt.flags as u64);
        vga::println("");
        
        // Parse MADT entries
        let header_size = core::mem::size_of::<AcpiTableHeader>() + 8; // header + local_apic_address + flags
        let mut offset = header_size;
        let total_length = madt.header.length as usize;
        
        let mut cpu_count = 0;
        let mut io_apic_count = 0;
        
        while offset < total_length {
            let entry_ptr = (madt_address as usize + offset) as *const MadtEntryHeader;
            let entry_header = &*entry_ptr;
            
            match MadtEntryType::from(entry_header.entry_type) {
                MadtEntryType::LocalApic => {
                    let local_apic = &*(entry_ptr as *const MadtLocalApic);
                    if local_apic.flags & 1 != 0 { // Processor enabled
                        cpu_count += 1;
                    }
                }
                MadtEntryType::IoApic => {
                    let io_apic = &*(entry_ptr as *const MadtIoApic);
                    if io_apic_count == 0 {
                        vga::print("      IO APIC Address: ");
                        vga::print_hex(io_apic.io_apic_address as u64);
                        vga::println("");
                    }
                    io_apic_count += 1;
                }
                _ => {}
            }
            
            offset += entry_header.length as usize;
        }
        
        vga::print("      CPU Cores: ");
        vga::print_num(cpu_count);
        vga::println("");
        
        vga::print("      IO APICs: ");
        vga::print_num(io_apic_count);
        vga::println("");
    }
}
