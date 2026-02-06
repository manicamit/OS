# 🖥️ MyOS – Operating System (Rust + C, x86_64)

## 📂 Project Structure

myos/ \
├── boot/ # Bootloader (UEFI/BIOS)\
├── kernel/ # Kernel core (Rust + C)\
├── drivers/ # Hardware drivers (PCI, disk, net, input)\
├── lib/ # Shared libraries (libc-like, utilities)\
├── user/ # Userland programs and shells\
├── fs/ # Filesystem implementations (FAT32, GPT, Ext2)\
├── docs/ # Documentation and specs\
├── scripts/ # Build/automation scripts\
├── build/ # Compiled binaries and disk images\
└── tools/ # Toolchain, cross-compilers, helper utilities\


## 🗺️ Roadmap
See [roadmap.md](./roadmap.md) for tutorials + phased development plan.

## 🔧 Build Instructions
1. Install **Rust nightly**, QEMU, OVMF, LLVM.
2. Build kernel:  
   ```bash
   cargo build --target x86_64-unknown-none
   ```
3. Create disk image & boot with QEMU (details in boot/README.md).

✨ Goals

Bootable on real hardware & QEMU

Multitasking, filesystem, networking

Shell + GUI

Modular design for extensions
