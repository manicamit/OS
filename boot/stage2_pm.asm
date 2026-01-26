; boot/stage2_pm.asm
; Stage2: load at 0x1000 (real mode), then switch to 32-bit protected mode.
BITS 16
ORG 0x1000

start2:
    cli                     ; disable interrupts during transition
    xor ax, ax
    mov ds, ax
    mov ss, ax
    mov sp, 0x7C00          ; safe temporary stack in real mode

    ; --- ensure A20 (safe if already enabled) ---
    in  al, 0x92
    or  al, 00000010b
    out 0x92, al

    ; --- load the GDT pointer (gdtr) ---
    lgdt [gdt_descriptor]   ; loads 6-byte descriptor: word(limit) + dword(base)

    ; --- enable protected mode (set PE bit in CR0) ---
    mov eax, cr0
    or  eax, 1              ; set CR0.PE = 1
    mov cr0, eax

    ; --- far jump to flush prefetch and load CS with our code selector ---
    jmp CODE_SEL:pm_entry

; -----------------------
; GDT (three entries)
; -----------------------
; We use dq literals for clarity: each 'dq' is one 8-byte descriptor.
;  - first is null
;  - second: code descriptor (base=0, limit=0xFFFFF, 4K gran, executable, readable)
;  - third : data descriptor (base=0, limit=0xFFFFF, 4K gran, writable)
gdt_start:
    dq 0x0000000000000000   ; null descriptor

    ; code descriptor:
    ;  0x00CF9A000000FFFF as 8-byte literal works (little-endian in file)
    dq 0x00CF9A000000FFFF

    ; data descriptor:
    dq 0x00CF92000000FFFF

gdt_end:

; GDTR: 6 bytes: limit (word), base (dword)
gdt_descriptor:
    dw gdt_end - gdt_start - 1   ; size = size of GDT in bytes - 1
    dd gdt_start                 ; address of GDT

; -----------------------
; Protected-mode entry
; -----------------------
; Switch to 32-bit instructions
[BITS 32]
pm_entry:
    ; Now we are executing with CS = CODE_SEL (0x08).
    ; Load data segment selectors (these are 16-bit values but we 'mov' them via ax).
    mov ax, DATA_SEL
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    ; Load SS and set 32-bit ESP (do this while interrupts are still disabled)
    mov ax, DATA_SEL
    mov ss, ax
    mov esp, 0x9FC00       ; choose a 32-bit stack somewhere (adjust for your memory map)

    ; (Optional) enable interrupts later when system is ready: sti

    ; Demo: write a message to VGA text memory (32-bit)
    mov esi, msg
    mov edi, 0xB8000
.print_pm:
    lodsb                 ; AL = [ESI], ESI++
    test al, al
    jz .done_pm
    mov ah, 0x02
    stosw                 ; stores AX to [EDI], EDI += 2
    jmp .print_pm

.done_pm:
    cli
.hlt_loop:
    hlt
    jmp .hlt_loop

; -----------------------
; Data & selectors
; -----------------------
msg db "Now in 32-bit Protected Mode!", 0

; selectors: offset into GDT: index<<3, RPL=0 -> 0x08 and 0x10
CODE_SEL equ 0x08
DATA_SEL equ 0x10

; pad stage2 to same size as before (8 sectors)
times 512*8-($-$$) db 0
