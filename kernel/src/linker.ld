OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

SECTIONS {
    /* Load kernel at 1 MB */
    . = 1M;

    .text : {
        *(.text._start)  /* put our _start first */
        *(.text*)        /* rest of code */
    }

    .rodata : {
        *(.rodata*)      /* read-only data */
    }

    .data : {
        *(.data*)        /* initialized data */
    }

    .bss : {
        *(.bss*)         /* zero-initialized data */
        *(COMMON)
    }
}
