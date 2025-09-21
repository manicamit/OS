; boot/stage2.asm 
BITS 16 
ORG 0x1000 

; stage2 is loaded at physical 0x1000 

start2: 
	cli 
	; set up data segments to 0 
	xor ax, ax 
	mov ds, ax 
	mov ss, ax 
	mov sp, 0x7C00 ; temporary stack 

	; point ES to VGA segment (0xB800) 
	mov ax, 0xB800 
	mov es, ax 
	xor di, di ; start of VGA text buffer 

	; SI points to our string 
	mov si, msg 

.print: 
	lodsb ; load next byte from [DS:SI] into AL 
	or al, al ; check if zero terminator 
	jz .done 
	mov ah, 0x02 ; attribute: light grey on black 
	stosw ; store AX at ES:DI, then DI+=2 
	jmp .print 

.done: 
	hlt 
	jmp .done ; halt forever 

; Data 
msg db "Hello from stage2 (real mode)!", 0 

; pad stage2 to 8 sectors (same size we asked MBR to load) 
times 512*8-($-$$) db 0
