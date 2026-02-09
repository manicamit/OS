// kernel/drivers/serial.c

#include <stdint.h>

#define SERIAL_COM1 0x3F8

// I/O port functions
static inline void outb(uint16_t port, uint8_t value) {
    asm volatile("outb %0, %1" : : "a"(value), "Nd"(port));
}

static inline uint8_t inb(uint16_t port) {
    uint8_t value;
    asm volatile("inb %1, %0" : "=a"(value) : "Nd"(port));
    return value;
}

// Initialize serial port
void serial_init(void) {
    outb(SERIAL_COM1 + 1, 0x00);    // Disable interrupts
    outb(SERIAL_COM1 + 3, 0x80);    // Enable DLAB
    outb(SERIAL_COM1 + 0, 0x03);    // Set divisor to 3 (38400 baud) low byte
    outb(SERIAL_COM1 + 1, 0x00);    // High byte
    outb(SERIAL_COM1 + 3, 0x03);    // 8 bits, no parity, one stop bit
    outb(SERIAL_COM1 + 2, 0xC7);    // Enable FIFO, clear, 14-byte threshold
    outb(SERIAL_COM1 + 4, 0x0B);    // Enable IRQs, set RTS/DSR
}

// Check if transmit buffer is empty
static int serial_is_transmit_empty(void) {
    return inb(SERIAL_COM1 + 5) & 0x20;
}

// Write a single character
void serial_putchar(char c) {
    while (!serial_is_transmit_empty());
    outb(SERIAL_COM1, c);
}

// Write a string
void serial_write(const char *str) {
    while (*str) {
        serial_putchar(*str++);
    }
}

// Write a string with newline
void serial_println(const char *str) {
    serial_write(str);
    serial_putchar('\n');
}

// Write a hex number
void serial_write_hex(uint64_t num) {
    serial_write("0x");
    
    char buf[17];
    buf[16] = '\0';
    
    for (int i = 15; i >= 0; i--) {
        uint8_t nibble = num & 0xF;
        buf[i] = (nibble < 10) ? ('0' + nibble) : ('A' + nibble - 10);
        num >>= 4;
    }
    
    serial_write(buf);
}

// Write a decimal number
void serial_write_num(uint64_t num) {
    if (num == 0) {
        serial_putchar('0');
        return;
    }
    
    char buf[21]; // Max digits for uint64_t
    int i = 0;
    
    while (num > 0) {
        buf[i++] = '0' + (num % 10);
        num /= 10;
    }
    
    // Print in reverse
    while (i > 0) {
        serial_putchar(buf[--i]);
    }
}
