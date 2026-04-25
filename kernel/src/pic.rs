const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16    = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16    = 0xA1;

const ICW1_INIT: u8 = 0x11;
const ICW4_8086: u8 = 0x01;
const PIC_EOI: u8   = 0x20;

pub const PIC1_OFFSET: u8 = 32;
pub const PIC2_OFFSET: u8 = 40;

#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    core::arch::asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack));
    val
}

#[inline(always)]
unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
}

#[inline(always)]
unsafe fn io_wait() {
    outb(0x80, 0);
}

pub fn init() {
    unsafe {
        let mask1 = inb(PIC1_DATA);
        let mask2 = inb(PIC2_DATA);

        outb(PIC1_COMMAND, ICW1_INIT); io_wait();
        outb(PIC2_COMMAND, ICW1_INIT); io_wait();
        outb(PIC1_DATA, PIC1_OFFSET);  io_wait();
        outb(PIC2_DATA, PIC2_OFFSET);  io_wait();
        outb(PIC1_DATA, 0x04);         io_wait();
        outb(PIC2_DATA, 0x02);         io_wait();
        outb(PIC1_DATA, ICW4_8086);    io_wait();
        outb(PIC2_DATA, ICW4_8086);    io_wait();

        outb(PIC1_DATA, 0xFF);
        outb(PIC2_DATA, 0xFF);

        let _ = (mask1, mask2);
    }
    crate::serial::println("PIC: Remapped IRQs to vectors 32-47");
}

pub fn unmask_irq(irq: u8) {
    unsafe {
        if irq < 8 {
            let mask = inb(PIC1_DATA);
            outb(PIC1_DATA, mask & !(1 << irq));
        } else {
            let slave_irq = irq - 8;
            let mask = inb(PIC2_DATA);
            outb(PIC2_DATA, mask & !(1 << slave_irq));
            let master_mask = inb(PIC1_DATA);
            outb(PIC1_DATA, master_mask & !(1 << 2));
        }
    }
}

pub fn mask_irq(irq: u8) {
    unsafe {
        if irq < 8 {
            let mask = inb(PIC1_DATA);
            outb(PIC1_DATA, mask | (1 << irq));
        } else {
            let slave_irq = irq - 8;
            let mask = inb(PIC2_DATA);
            outb(PIC2_DATA, mask | (1 << slave_irq));
        }
    }
}

pub fn send_eoi(irq: u8) {
    unsafe {
        if irq >= 8 { outb(PIC2_COMMAND, PIC_EOI); }
        outb(PIC1_COMMAND, PIC_EOI);
    }
}
