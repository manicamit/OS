; boot/mbr.asm 
; Minimal BIOS MBR boot sector (512 bytes) 
; Loads stage2 from LBA=1 into physical address 0x0000:0x1000 (linear 0x1000) 
; Exits to that address (far jump) 
BITS 16 
ORG 0x7C00
 
start: 
	cli ; clear interrupts while we set up 
	xor ax, ax 
	mov ds, ax 
	mov es, ax 
	mov ss, ax 
	mov sp, 0x7C00 ; stack near boot sector 
	mov [boot_drive], dl ; Save DL (boot drive number) 

	; Enable A20 (fast method via port 0x92) 
	in al, 0x92 
	or al, 00000010b ; set A20 enable bit 
	out 0x92, al 

	; Check EDD support 
	mov ah, 0x41 
	mov bx, 0x55AA 
	mov dl, [boot_drive] 
	int 0x13 
	jc no_edd_support 
	cmp bx, 0xAA55 
	jne no_edd_support 

	; Prepare Disk Address Packet (16 bytes) for INT 13h AH=0x42 (EDD) 
	lea si, [dap] 
	mov byte [si+0], 0x10 ; size = 16 
	mov byte [si+1], 0x00 ; reserved 
	mov ax, [count_sectors] ; Load count_sectors into ax 
	mov word [si+2], ax ; Store ax into [si+2] 
	mov word [si+4], 0x1000 ; buffer offset 
	mov word [si+6], 0x0000 ; buffer segment 
	mov dword [si+8], 0x00000001 ; LBA low dword 
	mov dword [si+12], 0x00000000 ; LBA high dword 

	; Call BIOS int 13h ext read 
	mov si, dap 
	mov ah, 0x42 
	mov dl, [boot_drive] 
	int 0x13 
	jc disk_error 

	; On success jump to stage2 at physical 0x1000 
	jmp 0x0000:0x1000 

print_string: 
	lodsb 
	or al, al 
	jz .done 
	mov ah, 0x0E 
	int 0x10 
	jmp print_string 

.done: 
	ret 

disk_error: 
	; Print "Disk error" then hang 
	mov si, err_msg 
	call print_string 
	jmp .hang 

.hang: 
	hlt 
	jmp .hang 

no_edd_support: 
	mov si, edd_err_msg 
	call print_string 
	jmp disk_error.hang 

; Data 
dap: 
	times 16 db 0 
	
count_sectors: dw 8 ; read 8 sectors (4KB) 

err_msg db "Disk read error", 0 
edd_err_msg db "No EDD support", 0 

boot_drive db 0 
; Fill to 510 and put boot signature 
times 510-($-$$) db 0 
dw 0xAA55
