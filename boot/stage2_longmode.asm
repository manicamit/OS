; boot/stage2_longmode.asm
; =====================================================
; Stage 2 Loader
; Real Mode → Protected Mode → Long Mode (x86_64)
; Loads at physical address 0x1000
; =====================================================

; -----------------------
; Constants (must be first)
; -----------------------
CODE_SEL    equ 0x08
DATA_SEL    equ 0x10
CODE64_SEL  equ 0x18

; -----------------------
; Real Mode Entry
; -----------------------
BITS 16
ORG 0x1000

start2:
    cli
    xor ax, ax
    mov ds, ax
    mov ss, ax
    mov sp, 0x7C00

    ; Enable A20
    in  al, 0x92
    or  al, 00000010b
    out 0x92, al

    ; Load GDT
    lgdt [gdt_descriptor]

    ; Enter protected mode
    mov eax, cr0
    or eax, 1
    mov cr0, eax

    jmp CODE_SEL:protected_mode_entry

; -----------------------
; GDT (correct & aligned)
; -----------------------
align 8
gdt_start:
    dq 0x0000000000000000        ; null
    dq 0x00CF9A000000FFFF        ; 32-bit code
    dq 0x00CF92000000FFFF        ; 32-bit data
    dq 0x00209A0000000000        ; 64-bit code (L=1, D=0)
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

    ; ---------------------------------
    ; Clear page tables (3 pages)
    ; ---------------------------------
    mov edi, 0x2000
    mov ecx, (4096 * 3) / 4
    xor eax, eax
    rep stosd

    ; ---------------------------------
    ; PML4[0] → PDPT
    ; ---------------------------------
    mov dword [0x2000], 0x3003
    mov dword [0x2004], 0

    ; ---------------------------------
    ; PDPT[0] → PD
    ; ---------------------------------
    mov dword [0x3000], 0x4003
    mov dword [0x3004], 0

    ; ---------------------------------
    ; Build Page Directory (2 MiB pages)
    ; Identity map first 1 GiB
    ; ---------------------------------
    xor ebx, ebx        ; physical address
    xor edi, edi        ; PD offset
    mov ecx, 512

.make_pd:
    mov eax, ebx
    or eax, 0x83        ; present | writable | 2MiB
    mov [0x4000 + edi], eax
    mov dword [0x4000 + edi + 4], 0
    add ebx, 0x200000
    add edi, 8
    loop .make_pd

    ; ---------------------------------
    ; Enable paging & long mode
    ; ---------------------------------
    mov eax, 0x2000
    mov cr3, eax

    mov eax, cr4
    or eax, 1 << 5      ; PAE
    mov cr4, eax

    mov ecx, 0xC0000080 ; EFER
    rdmsr
    or eax, 1 << 8      ; LME
    wrmsr

    mov eax, cr0
    or eax, 1 << 31     ; PG
    mov cr0, eax

    ; Jump to 64-bit code
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

    ; Write message to VGA
    mov rsi, msg64
    mov rdi, 0xB8000

.print:
    lodsb
    test al, al
    jz .done
    mov ah, 0x02
    stosw
    jmp .print

.done:
    cli
.hang:
    hlt
    jmp .hang

msg64 db "Hello from 64-bit long mode!", 0

; Pad to exactly 8 sectors
times 512*8-($-$$) db 0
