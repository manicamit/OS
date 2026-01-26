; boot/mbr.asm - Debug version with visual feedback
BITS 16 
ORG 0x7C00
 
start: 
    cli
    xor ax, ax 
    mov ds, ax 
    mov es, ax 
    mov ss, ax 
    mov sp, 0x7C00
    mov [boot_drive], dl
    
    ; Print "M" to show MBR loaded
    mov ah, 0x0E
    mov al, 'M'
    int 0x10
    
    ; Enable A20
    in al, 0x92 
    or al, 00000010b 
    out 0x92, al 
    
    ; Print "A" to show A20 enabled
    mov ah, 0x0E
    mov al, 'A'
    int 0x10
    
    ; Check EDD support 
    mov ah, 0x41 
    mov bx, 0x55AA 
    mov dl, [boot_drive] 
    int 0x13 
    jc no_edd_support 
    cmp bx, 0xAA55 
    jne no_edd_support 
    
    ; Print "E" for EDD support confirmed
    mov ah, 0x0E
    mov al, 'E'
    int 0x10
    
    ; Prepare DAP
    mov byte [dap+0], 0x10
    mov byte [dap+1], 0x00
    mov word [dap+2], 8          ; 8 sectors
    mov word [dap+4], 0x1000     ; offset
    mov word [dap+6], 0x0000     ; segment
    mov dword [dap+8], 1         ; LBA = 1
    mov dword [dap+12], 0
    
    ; Read stage2
    mov si, dap
    mov ah, 0x42 
    mov dl, [boot_drive] 
    int 0x13 
    jc disk_error 
    
    ; Print "D" for disk read success
    mov ah, 0x0E
    mov al, 'D'
    int 0x10
    
    ; Print "J" before jump
    mov ah, 0x0E
    mov al, 'J'
    int 0x10
    
    ; Jump to stage2
    jmp 0x0000:0x1000 

disk_error: 
    mov ah, 0x0E
    mov al, 'X'
    int 0x10
    cli
.hang: 
    hlt 
    jmp .hang 

no_edd_support: 
    mov ah, 0x0E
    mov al, 'N'
    int 0x10
    jmp disk_error.hang

dap: times 16 db 0
boot_drive db 0 

times 510-($-$$) db 0 
dw 0xAA55
