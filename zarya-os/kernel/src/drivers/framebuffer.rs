//! Framebuffer Driver
//! 
//! Provides access to graphics framebuffer for display output.

use x86_64::PhysAddr;

/// Framebuffer information
#[derive(Debug, Clone, Copy)]
pub struct FramebufferInfo {
    pub addr: PhysAddr,
    pub width: u32,
    pub height: u32,
    pub bytes_per_pixel: u32,
    pub stride: u32,
}

/// Global framebuffer info
static mut FRAMEBUFFER_INFO: Option<FramebufferInfo> = None;

/// Initialize framebuffer from boot info
pub fn init() {
    println!("Initializing framebuffer...");
    // Framebuffer would be initialized from boot info in real implementation
    unsafe {
        FRAMEBUFFER_INFO = Some(FramebufferInfo {
            addr: PhysAddr::new(0xE0000000), // Example address
            width: 1920,
            height: 1080,
            bytes_per_pixel: 4,
            stride: 1920 * 4,
        });
    }
}

/// Get framebuffer info
pub fn get_info() -> Option<FramebufferInfo> {
    unsafe { FRAMEBUFFER_INFO }
}

/// Draw a pixel at (x, y) with color (r, g, b)
pub fn draw_pixel(x: u32, y: u32, r: u8, g: u8, b: u8) {
    unsafe {
        if let Some(fb) = FRAMEBUFFER_INFO {
            if x >= fb.width || y >= fb.height {
                return;
            }
            
            let offset = (y * fb.stride + x * fb.bytes_per_pixel) as usize;
            let ptr = (fb.addr.as_u64() + offset as u64) as *mut u8;
            
            // Write pixel in BGRA format (common for framebuffers)
            ptr.write(b);
            ptr.add(1).write(g);
            ptr.add(2).write(r);
            ptr.add(3).write(0xFF); // Alpha
        }
    }
}

/// Clear screen with color
pub fn clear_screen(r: u8, g: u8, b: u8) {
    unsafe {
        if let Some(fb) = FRAMEBUFFER_INFO {
            for y in 0..fb.height {
                for x in 0..fb.width {
                    draw_pixel(x, y, r, g, b);
                }
            }
        }
    }
}

/// Fill rectangle with color
pub fn fill_rect(x: u32, y: u32, width: u32, height: u32, r: u8, g: u8, b: u8) {
    for dy in 0..height {
        for dx in 0..width {
            draw_pixel(x + dx, y + dy, r, g, b);
        }
    }
}

/// Draw a simple character (bitmap font)
pub fn draw_char(x: u32, y: u32, ch: char, fg_r: u8, fg_g: u8, fg_b: u8) {
    // Simple 8x8 bitmap font would go here
    // For now, just draw a placeholder
    draw_pixel(x, y, fg_r, fg_g, fg_b);
}

/// Draw string
pub fn draw_string(x: u32, y: u32, s: &str, fg_r: u8, fg_g: u8, fg_b: u8) {
    let mut cursor_x = x;
    for ch in s.chars() {
        if ch == '\n' {
            cursor_x = x;
        } else {
            draw_char(cursor_x, y, ch, fg_r, fg_g, fg_b);
            cursor_x += 8; // Character width
        }
    }
}
