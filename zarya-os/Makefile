# Zarya OS Makefile
# Quick commands for building and running the OS

.PHONY: all build run clean help kernel apps servers iso

all: build

build:
@echo "Building Zarya OS..."
@./build.sh

kernel:
@echo "Building kernel..."
@cd kernel && cargo build --release

apps:
@echo "Building applications..."
@for app in apps/*; do \
if [ -d "$$app" ]; then \
echo "  Building $$(basename $$app)..."; \
cd "$$app" && cargo build --release 2>/dev/null || true; \
cd ../..; \
fi \
done

servers:
@echo "Building servers..."
@for server in servers/*; do \
if [ -d "$$server" ]; then \
echo "  Building $$(basename $$server)..."; \
cd "$$server" && cargo build --release 2>/dev/null || true; \
cd ../..; \
fi \
done

iso: build

run: build
@echo "Starting QEMU..."
@qemu-system-x86_64 \
-cdrom zarya.iso \
-m 2G \
-smp 2 \
-boot d \
-serial stdio \
-vga vmware \
-usb \
-device usb-kbd \
-device usb-tablet

run-raw: build
@echo "Starting QEMU with raw image..."
@qemu-system-x86_64 \
-drive file=zarya.img,format=raw \
-m 2G \
-smp 2 \
-serial stdio \
-vga vmware

debug: kernel
@echo "Starting QEMU with GDB stub..."
@qemu-system-x86_64 \
-cdrom zarya.iso \
-m 2G \
-s \
-S \
-serial stdio

clean:
@echo "Cleaning build artifacts..."
@cd kernel && cargo clean
@cd boot && cargo clean
@for dir in apps/* servers/*; do \
if [ -d "$$dir" ]; then \
cd "$$dir" && cargo clean 2>/dev/null || true; \
cd ../..; \
fi \
done
@rm -f zarya.iso zarya.img
@rm -rf isofiles
@echo "Clean complete."

help:
@echo "Zarya OS Build System"
@echo ""
@echo "Available targets:"
@echo "  all      - Build everything (default)"
@echo "  build    - Run full build script"
@echo "  kernel   - Build only the kernel"
@echo "  apps     - Build userland applications"
@echo "  servers  - Build system servers"
@echo "  iso      - Create bootable ISO"
@echo "  run      - Build and run in QEMU"
@echo "  debug    - Run with GDB debugging"
@echo "  clean    - Remove build artifacts"
@echo "  help     - Show this help message"
@echo ""
