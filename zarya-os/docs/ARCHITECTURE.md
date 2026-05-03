# Zarya OS Architecture Documentation

## Overview

Zarya is a hybrid-kernel operating system designed to combine the best features of modern operating systems:
- macOS-like visual aesthetics and smooth animations
- Windows-like functionality and customization
- Android-like touch responsiveness and mobile readiness
- Linux-like freedom and modularity

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    User Applications                         │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐   │
│  │  Files   │ │   Text   │ │ Terminal │ │   Settings   │   │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                   System Servers                             │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐   │
│  │   ZDS    │ │  Input   │ │    FS    │ │   Network    │   │
│  │ (Display)│ │ Server   │ │  Server  │ │    Server    │   │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                    libzarya (System Libs)                    │
│         POSIX Layer | IPC | Memory | Threading              │
├─────────────────────────────────────────────────────────────┤
│                      Zarya Kernel                            │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐   │
│  │ Scheduler│ │  Memory  │ │    IPC   │ │   Drivers    │   │
│  │          │ │ Manager  │ │          │ │              │   │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────┘   │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐   │
│  │   VFS    │ │ Syscalls │ │  Network │ │  Synchronization│ │
│  │          │ │          │ │   Stack  │ │              │   │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                         Hardware                             │
│     CPU (x86_64/ARM64) | RAM | Storage | GPU | Network      │
└─────────────────────────────────────────────────────────────┘
```

## Kernel Components

### 1. Memory Management (`kernel/src/memory/`)
- **Physical Allocator**: Buddy allocator for physical pages
- **Virtual Memory**: 4-level paging on x86_64
- **Slab Allocator**: Efficient kernel object allocation
- **VMA Management**: Virtual memory area tracking

### 2. Task Scheduler (`kernel/src/task/`)
- **Preemptive Multitasking**: Round-robin with priority
- **Process States**: Running, Ready, Blocked, Terminated
- **Thread Support**: Multiple threads per process
- **Priority Classes**: Idle, Normal, High, RealTime

### 3. IPC Subsystem (`kernel/src/ipc/`)
- **Message Passing**: L4-style synchronous/asynchronous
- **Ports**: Communication endpoints
- **Shared Memory**: Zero-copy data sharing
- **Signals**: Process notification mechanism

### 4. Device Drivers (`kernel/src/drivers/`)
- **Serial**: Debug console output
- **Keyboard**: PS/2 and USB HID support
- **Mouse**: PS/2 protocol
- **Framebuffer**: GOP/VESA display output

### 5. Filesystem (`kernel/src/fs/`)
- **VFS Layer**: Unified filesystem interface
- **ZFS**: Native Zarya File System (journaling)
- **Compatibility**: FAT32, ext4, NTFS modules

### 6. Network Stack (`kernel/src/net/`)
- **TCP/IP**: Basic protocol implementation
- **Ethernet**: Driver interface
- **Socket API**: BSD-compatible sockets

### 7. Syscalls (`kernel/src/syscall/`)
- **POSIX Layer**: Standard Unix syscalls
- **Zarya Extensions**: Custom syscalls for IPC, etc.

## Display Server (ZDS)

The Zarya Display Server is a compositing window manager that provides:
- Hardware-accelerated rendering
- Window decorations and effects
- Input event distribution
- Multi-monitor support

## GUI Toolkit (ZUI)

ZUI is the native widget toolkit featuring:
- Immediate mode rendering
- Vector-based UI elements
- Touch-friendly controls
- Theme support

## File Structure

```
zarya-os/
├── boot/           # Bootloader (UEFI/Multiboot)
├── kernel/         # Kernel source code
│   └── src/
│       ├── memory/ # Memory management
│       ├── task/   # Scheduler
│       ├── ipc/    # IPC
│       ├── drivers/# Device drivers
│       ├── fs/     # Filesystem
│       ├── syscall/# System calls
│       ├── net/    # Network
│       └── sync/   # Synchronization
├── servers/        # System servers
│   ├── zds/        # Display server
│   ├── input/      # Input server
│   ├── fs/         # File server
│   └── network/    # Network server
├── apps/           # User applications
│   ├── zarya-desktop/
│   ├── zarya-files/
│   ├── zarya-text/
│   ├── zarya-terminal/
│   ├── zarya-settings/
│   └── zarya-login/
├── lib/            # Libraries
│   ├── libzarya/   # System library
│   └── zui/        # GUI toolkit
├── docs/           # Documentation
├── build.sh        # Build script
└── Makefile        # Quick commands
```

## Building

```bash
# Full build
./build.sh

# Or using make
make build

# Run in QEMU
make run

# Clean build
make clean
```

## Requirements

- Rust nightly toolchain
- NASM assembler
- GRUB tools (for ISO creation)
- QEMU (for testing)

## License

MIT License
