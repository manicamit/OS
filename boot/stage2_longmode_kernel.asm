; =====================================================
; Stage 2 Loader - DEBUG VERSION
; Real Mode → Load Kernel → Protected Mode → Long Mode
; =====================================================
; -----------------------
; Constants
; -----------------------
CODE_SEL    equ 0x08
DATA_SEL    equ 0x10
CODE64_SEL  equ 0x18
KERNEL_LBA     equ 2048
KERNEL_SECTORS equ 126
KERNEL_LOAD    equ 0x00100000     ; 1 MiB (final destination)
KERNEL_TEMP    equ 0x00010000     ; 64KB (temporary load location)

; -----------------------
; Real Mode Entry
; -----------------------
BITS 16
ORG 0x1000
start2:
    cli
    
    ; Print "2" to show stage2 loaded
    mov ah, 0x0E
    mov al, '2'
    int 0x10
    
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00

    ; Print "A" - about to enable A20
    mov ah, 0x0E
    mov al, 'A'
    int 0x10

    ; Enable A20
    in  al, 0x92
    or  al, 00000010b
    out 0x92, al

    ; Print "K" - about to load kernel
    mov ah, 0x0E
    mov al, 'K'
    int 0x10

    ; -------------------------------
    ; BIOS disk read (REAL MODE ONLY)
    ; Load kernel to 0x10000 (64KB) temporarily
    ; -------------------------------
    mov word  [dap+0], 0x0010        ; size = 16
    mov word  [dap+2], KERNEL_SECTORS ; sectors to read
    mov word  [dap+4], 0x0000        ; offset = 0
    mov word  [dap+6], 0x1000        ; segment = 0x1000 (0x1000:0x0000 = 0x10000 = 64KB)
    mov dword [dap+8], KERNEL_LBA    ; LBA low
    mov dword [dap+12], 0x00000000   ; LBA high

    mov si, dap
    mov ah, 0x42
    mov dl, 0x80
    int 0x13
    jc disk_error

    ; Print "L" - kernel loaded
    mov ah, 0x0E
    mov al, 'L'
    int 0x10

    ; Load GDT
    lgdt [gdt_descriptor]

    ; Print "G" - GDT loaded
    mov ah, 0x0E
    mov al, 'G'
    int 0x10

    ; Enter protected mode
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    jmp CODE_SEL:protected_mode_entry

disk_error:
    ; Print "X" for disk error
    mov ah, 0x0E
    mov al, 'X'
    int 0x10
    cli
.hang:
    hlt
    jmp .hang

dap: times 16 db 0

; -----------------------
; GDT
; -----------------------
align 8
gdt_start:
    dq 0x0000000000000000        ; null
    dq 0x00CF9A000000FFFF        ; 32-bit code
    dq 0x00CF92000000FFFF        ; 32-bit data
    dq 0x00209A0000000000        ; 64-bit code
gdt_end:

gdt_descriptor:
    dw gdt_end - gdt_start - 1
    dd gdt_start

; -----------------------
; Protected Mode (32-bit)
; -----------------------
[BITS 32]
protected_mode_entry:
    mov ax, DATA_SEL
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov esp, 0x9FC00

    ; Can't print in protected mode easily, so we'll write to VGA directly
    ; Write "P" to top-left of screen (white on black)
    mov dword [0xB8000], 0x0F500F50  ; "PP"

    ; ---------------------------------
    ; Copy kernel from 0x10000 to 0x100000 (1MB)
    ; ---------------------------------
    mov esi, KERNEL_TEMP            ; source: 64KB
    mov edi, KERNEL_LOAD            ; dest: 1MB
    mov ecx, KERNEL_SECTORS * 512
    shr ecx, 2                      ; divide by 4 (copy dwords)
    rep movsd

    ; Write "C" - copy done
    mov dword [0xB8002], 0x0F430F43  ; "CC"

    ; ---------------------------------
    ; Clear page tables (PML4 + PDPT + 4 PDs)
    ; ---------------------------------
    mov edi, 0x2000
    mov ecx, (4096 * 6) / 4
    xor eax, eax
    rep stosd

    ; Write "T" - tables cleared
    mov dword [0xB8004], 0x0F540F54  ; "TT"

    ; ---------------------------------
    ; PML4[0] → PDPT
    ; ---------------------------------
    mov dword [0x2000], 0x3003
    mov dword [0x2004], 0

    ; ---------------------------------
    ; PDPT[0..3] → 4 Page Directories
    ; ---------------------------------
    mov dword [0x3000 + 0*8], 0x4003
    mov dword [0x3004 + 0*8], 0
    mov dword [0x3000 + 1*8], 0x5003
    mov dword [0x3004 + 1*8], 0
    mov dword [0x3000 + 2*8], 0x6003
    mov dword [0x3004 + 2*8], 0
    mov dword [0x3000 + 3*8], 0x7003
    mov dword [0x3004 + 3*8], 0

    ; ---------------------------------
    ; Build Page Directories
    ; Identity-map first 4 GiB
    ; ---------------------------------
    mov edi, 0x4000
    xor ebx, ebx
    mov ecx, 512 * 4
.make_pd:
    mov eax, ebx
    or eax, 0x83                ; present | writable | 2 MiB
    mov [edi], eax
    mov dword [edi+4], 0
    add ebx, 0x200000
    add edi, 8
    loop .make_pd

    ; Write "M" - page tables built
    mov dword [0xB8006], 0x0F4D0F4D  ; "MM"

    ; ---------------------------------
    ; Enable paging & long mode
    ; ---------------------------------
    mov eax, 0x2000
    mov cr3, eax

    mov eax, cr4
    or eax, 1 << 5              ; PAE
    mov cr4, eax

    mov ecx, 0xC0000080         ; EFER
    rdmsr
    or eax, 1 << 8              ; LME
    wrmsr

    mov eax, cr0
    or eax, 1 << 31             ; PG
    mov cr0, eax

    ; Write "6" before entering long mode
    mov dword [0xB8008], 0x0F360F36  ; "66"

    jmp CODE64_SEL:long_mode_entry

; -----------------------
; Long Mode (64-bit)
; -----------------------
[BITS 64]
long_mode_entry:
    mov ax, DATA_SEL
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov rsp, 0x80000
    and rsp, -16                ; SysV ABI alignment

   ; Write "8" - in long mode
    mov dword [abs 0xB800A], 0x0F380F38  ; "88"
    
    ; Write "L" for LONGMODE  
    mov dword [abs 0xB800C], 0x0F4C0F4C  ; "LL"
    
    ; ADDED: Write directly to VGA to prove we're here
    mov rax, 0xB8000
    mov word [rax + 14], 0x4F4F  ; Red 'O'
    mov word [rax + 16], 0x4F4B  ; Red 'K'

    
    ; Jump to actual code (kernel loaded at 0x100000, code at +0x1000)
    call KERNEL_LOAD
    
.halt:
    cli
    hlt
    jmp .halt

; Pad to exactly 8 sectors
times 512*8-($-$$) db 0
