#ifndef SERIAL_H
#define SERIAL_H

#include <stdint.h>

// Initialize serial port
void serial_init(void);

// Write functions
void serial_putchar(char c);
void serial_write(const char *str);
void serial_println(const char *str);
void serial_write_hex(uint64_t num);
void serial_write_num(uint64_t num);

#endif // SERIAL_H
