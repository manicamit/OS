; =====================================================
; Stage 2 Loader - WITH DEBUG
; =====================================================

CODE_SEL    equ 0x08
DATA_SEL    equ 0x10
CODE64_SEL  equ 0x18

KERNEL_LBA     equ 2048
KERNEL_SECTORS equ 128
KERNEL_LOAD    equ 0x00100000

E820_BUFFER    equ 0x00008000
E820_ENTRY_SZ  equ 24
E820_MAX_ENTS  equ 128

BITS 16
ORG 0x1000

start2:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00

    ; Print '2' to show stage2 started
    mov ah, 0x0E
    mov al, '2'
    int 0x10

    ; Enable A20
    in  al, 0x92
    or  al, 00000010b
    out 0x92, al

; ---------------------------------------
; Collect E820
; ---------------------------------------
    xor ebx, ebx
    mov di, E820_BUFFER
    mov word [e820_count], 0

.e820_loop:
    mov eax, 0xE820
    mov ecx, E820_ENTRY_SZ
    mov edx, 0x534D4150
    int 0x15
    jc .e820_failed

    cmp eax, 0x534D4150
    jne .e820_failed

    ; Check length
    mov eax, dword [di + 8]
    or  eax, dword [di + 12]
    jz  .skip_entry

    ; Valid entry
    inc word [e820_count]
    add di, E820_ENTRY_SZ

    ; Print 'E' for each entry found
    mov ah, 0x0E
    mov al, 'E'
    int 0x10

.skip_entry:
    test ebx, ebx
    jz .e820_done
    
    cmp word [e820_count], E820_MAX_ENTS
    jge .e820_done
    
    jmp .e820_loop

.e820_failed:
    ; Print 'F' for E820 failed
    mov ah, 0x0E
    mov al, 'F'
    int 0x10
    jmp .e820_done

.e820_done:
    ; Print the count (single digit)
    mov ah, 0x0E
    mov al, byte [e820_count]
    add al, '0'
    int 0x10

; ---------------------------------------
; Load kernel
; ---------------------------------------
    mov word  [dap], 0x10
    mov word  [dap+2], KERNEL_SECTORS
    mov word  [dap+4], 0x0000
    mov word  [dap+6], 0x1000
    mov dword [dap+8], KERNEL_LBA
    mov dword [dap+12], 0

    mov si, dap
    mov ah, 0x42
    mov dl, 0x80
    int 0x13
    jc disk_error

    ; Print 'K' for kernel loaded
    mov ah, 0x0E
    mov al, 'K'
    int 0x10

    lgdt [gdt_descriptor]

    mov eax, cr0
    or eax, 1
    mov cr0, eax
    jmp CODE_SEL:protected_mode_entry

disk_error:
    mov ah, 0x0E
    mov al, 'X'
    int 0x10
    cli
.hang:
    hlt
    jmp .hang

dap: times 16 db 0
e820_count: dw 0

align 8
gdt_start:
    dq 0x0000000000000000
    dq 0x00CF9A000000FFFF
    dq 0x00CF92000000FFFF
    dq 0x00209A0000000000
gdt_end:

gdt_descriptor:
    dw gdt_end - gdt_start - 1
    dd gdt_start

BITS 32
protected_mode_entry:
    mov ax, DATA_SEL
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov esp, 0x9FC00

    ; Write 'P' to VGA
    mov dword [0xB8000], 0x0F500F50

    ; Copy kernel
    mov esi, 0x00010000
    mov edi, KERNEL_LOAD
    mov ecx, KERNEL_SECTORS * 512
    shr ecx, 2
    rep movsd

    ; Write 'C' to VGA
    mov dword [0xB8002], 0x0F430F43

    ; Clear paging
    mov edi, 0x2000
    mov ecx, (4096 * 6) / 4
    xor eax, eax
    rep stosd

    ; Setup paging
    mov dword [0x2000], 0x3003
    mov dword [0x2004], 0
    
    mov dword [0x3000], 0x4003
    mov dword [0x3004], 0
    mov dword [0x3008], 0x5003
    mov dword [0x300C], 0
    mov dword [0x3010], 0x6003
    mov dword [0x3014], 0
    mov dword [0x3018], 0x7003
    mov dword [0x301C], 0

    mov edi, 0x4000
    xor ebx, ebx
    mov ecx, 512 * 4

.make_pd:
    mov eax, ebx
    or eax, 0x83
    mov [edi], eax
    mov dword [edi+4], 0
    add ebx, 0x200000
    add edi, 8
    loop .make_pd

    mov eax, 0x2000
    mov cr3, eax

    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax

    jmp CODE64_SEL:long_mode_entry

BITS 64
long_mode_entry:
    mov ax, DATA_SEL
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov rsp, 0x80000
    and rsp, -16

    ; Write '8' to VGA
    mov dword [0xB8004], 0x0F380F38

    ; Pass E820 to kernel
    mov rdi, E820_BUFFER
    movzx rsi, word [e820_count]

    call KERNEL_LOAD

.halt:
    cli
    hlt
    jmp .halt

times 512*8-($-$$) db 0
