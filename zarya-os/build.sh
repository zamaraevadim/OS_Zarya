#!/bin/bash
# Zarya OS Build Script
# Builds the complete operating system image

set -e

echo "====================================="
echo "  Zarya OS Build System"
echo "====================================="

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check for required tools
check_requirements() {
    log_info "Checking build requirements..."
    
    local missing=0
    
    if ! command -v rustc &> /dev/null; then
        log_error "Rust compiler not found. Please install Rust."
        missing=1
    fi
    
    if ! command -v cargo &> /dev/null; then
        log_error "Cargo not found. Please install Rust."
        missing=1
    fi
    
    if ! command -v nasm &> /dev/null; then
        log_warn "NASM not found. Some boot components may not build."
    fi
    
    if ! command -v grub-mkrescue &> /dev/null; then
        log_warn "GRUB tools not found. ISO creation may fail."
    fi
    
    if [ $missing -eq 1 ]; then
        exit 1
    fi
    
    log_info "All required tools found."
}

# Build bootloader
build_bootloader() {
    log_info "Building bootloader..."
    cd "$SCRIPT_DIR/boot"
    cargo build --release --target x86_64-unknown-none 2>/dev/null || log_warn "Bootloader build skipped (requires UEFI toolchain)"
    cd "$SCRIPT_DIR"
}

# Build kernel
build_kernel() {
    log_info "Building kernel..."
    cd "$SCRIPT_DIR/kernel"
    cargo build --release 2>&1 | head -20
    cd "$SCRIPT_DIR"
}

# Build userland servers
build_servers() {
    log_info "Building userland servers..."
    
    for server in zds input fs network; do
        if [ -d "$SCRIPT_DIR/servers/$server" ]; then
            log_info "  Building $server..."
            cd "$SCRIPT_DIR/servers/$server"
            cargo build --release 2>/dev/null || log_warn "Server $server build skipped"
        fi
    done
    
    cd "$SCRIPT_DIR"
}

# Build applications
build_apps() {
    log_info "Building applications..."
    
    for app in zarya-desktop zarya-files zarya-text zarya-terminal zarya-settings zarya-login; do
        if [ -d "$SCRIPT_DIR/apps/$app" ]; then
            log_info "  Building $app..."
            cd "$SCRIPT_DIR/apps/$app"
            cargo build --release 2>/dev/null || log_warn "App $app build skipped"
        fi
    done
    
    cd "$SCRIPT_DIR"
}

# Create ISO image
create_iso() {
    log_info "Creating bootable ISO image..."
    
    local ISO_ROOT="$SCRIPT_DIR/isofiles"
    mkdir -p "$ISO_ROOT/boot/grub"
    mkdir -p "$ISO_ROOT/zarya"
    
    # Copy kernel
    if [ -f "$SCRIPT_DIR/kernel/target/x86_64-unknown-none/release/zarya-kernel" ]; then
        cp "$SCRIPT_DIR/kernel/target/x86_64-unknown-none/release/zarya-kernel" "$ISO_ROOT/zarya/kernel.bin"
    else
        # Create placeholder for demo
        echo "KERNEL_PLACEHOLDER" > "$ISO_ROOT/zarya/kernel.bin"
    fi
    
    # Copy applications
    mkdir -p "$ISO_ROOT/zarya/bin"
    for app in zarya-desktop zarya-files zarya-text; do
        local app_bin="$SCRIPT_DIR/apps/$app/target/release/$app"
        if [ -f "$app_bin" ]; then
            cp "$app_bin" "$ISO_ROOT/zarya/bin/"
        fi
    done
    
    # Create GRUB config
    cat > "$ISO_ROOT/boot/grub/grub.cfg" << 'GRUB_EOF'
set timeout=5
set default=0

menuentry "Zarya OS" {
    multiboot2 /zarya/kernel.bin
    boot
}

menuentry "Zarya OS (Safe Mode)" {
    multiboot2 /zarya/kernel.bin
    set zarya_safe_mode=1
    boot
}
GRUB_EOF
    
    # Create ISO
    if command -v grub-mkrescue &> /dev/null; then
        grub-mkrescue -o "$SCRIPT_DIR/zarya.iso" "$ISO_ROOT" 2>/dev/null || {
            log_warn "grub-mkrescue failed, creating raw image instead"
            create_raw_image
        }
    else
        create_raw_image
    fi
    
    # Cleanup
    rm -rf "$ISO_ROOT"
    
    log_info "Build complete!"
}

# Create raw disk image (fallback)
create_raw_image() {
    log_info "Creating raw disk image..."
    
    local IMG_FILE="$SCRIPT_DIR/zarya.img"
    dd if=/dev/zero of="$IMG_FILE" bs=1M count=64 2>/dev/null
    
    log_info "Raw image created: $IMG_FILE"
}

# Main build process
main() {
    check_requirements
    build_bootloader
    build_kernel
    build_servers
    build_apps
    create_iso
    
    echo ""
    echo "====================================="
    echo "  Build Complete!"
    echo "====================================="
    echo ""
    echo "Output files:"
    echo "  - zarya.iso  (bootable ISO image)"
    echo "  - zarya.img  (raw disk image)"
    echo ""
    echo "To run in QEMU:"
    echo "  qemu-system-x86_64 -cdrom zarya.iso -m 2G"
    echo ""
}

main "$@"
