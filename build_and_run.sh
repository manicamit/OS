#!/bin/bash
# build_and_run.sh

echo "Building kernel with C serial driver..."

cd kernel
cargo build --release --target x86_64-unknown-none
cd ..

# Copy kernel binary
cp kernel/target/x86_64-unknown-none/release/kernel build/kernel.bin

# Assemble bootloader
nasm -f bin boot/mbr.asm -o build/mbr.bin
nasm -f bin boot/stage2.bin -o build/stage2.bin

# Create disk image
dd if=/dev/zero of=disk.img bs=512 count=2880
dd if=build/mbr.bin of=disk.img conv=notrunc
dd if=build/stage2.bin of=disk.img bs=512 seek=1 conv=notrunc
dd if=build/kernel.bin of=disk.img bs=512 seek=10 conv=notrunc

# Run with serial output to console
echo ""
echo "Starting QEMU with serial output..."
echo "Serial output will appear below:"
echo "========================================"

qemu-system-x86_64 \
    -drive format=raw,file=disk.img \
    -serial stdio \
    -no-reboot \
    -no-shutdown

# Alternative: Save serial output to file
# qemu-system-x86_64 \
#     -drive format=raw,file=disk.img \
#     -serial file:serial.log \
#     -no-reboot
