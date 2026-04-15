OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)
SECTIONS {
    . = 1M;
    
    __kernel_start = .;
    
    .text ALIGN(4K) : {
        __text_start = .;
        KEEP(*(.text._start))
        *(.text*)
        __text_end = .;
    }
    
    .rodata ALIGN(4K) : {
        __rodata_start = .;
        *(.rodata*)
        __rodata_end = .;
    }
    
    .data ALIGN(4K) : {
        __data_start = .;
        *(.data*)
        *(.got*)
        __data_end = .;
    }

    /* Capture .rela.dyn bounds so the kernel can process relocations at boot */
    .rela.dyn ALIGN(8) : {
        __rela_start = .;
        *(.rela.dyn)
        *(.rela.plt)
        __rela_end = .;
    }
    
    .bss ALIGN(4K) : {
        __bss_start = .;
        *(.bss*)
        *(COMMON)
        __bss_end = .;
    }
    
    __kernel_end = .;
    
    /DISCARD/ : {
        *(.eh_frame)
        *(.note*)
        *(.dynamic)
        *(.dynsym)
        *(.dynstr)
        *(.gnu.hash)
        *(.hash)
    }
}
