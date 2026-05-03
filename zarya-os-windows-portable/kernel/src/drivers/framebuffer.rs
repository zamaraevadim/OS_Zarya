//! Драйвер framebuffer (графический буфер)
//! 
//! Предоставляет доступ к графическому буферу через UEFI GOP или VBE.
//! Поддерживает базовый вывод пикселей и отрисовку примитивов.

use crate::sync::Spinlock;

/// Глобальное состояние framebuffer
static FRAMEBUFFER: Spinlock<Option<FrameBuffer>> = Spinlock::new(None);

/// Информация о framebuffer
#[derive(Debug, Clone, Copy)]
pub struct FrameBuffer {
    /// Базовый адрес в памяти
    pub base_addr: u64,
    /// Размер в байтах
    pub size: usize,
    /// Ширина экрана в пикселях
    pub width: u32,
    /// Высота экрана в пикселях
    pub height: u32,
    /// Количество байт на пиксель
    pub bytes_per_pixel: u8,
    /// Количество байт в строке (pitch)
    pub pitch: u32,
    /// Формат цвета
    pub format: PixelFormat,
}

/// Формат пикселя
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// RGB 8-8-8
    Rgb888,
    /// BGR 8-8-8
    Bgr888,
    /// ARGB 8-8-8-8
    Argb8888,
    /// ABGR 8-8-8-8
    Abgr8888,
    /// XRGB 8-8-8-8 (X игнорируется)
    Xrgb8888,
    /// XBCR 8-8-8-8
    Xbgr8888,
}

impl PixelFormat {
    /// Получение количества бит на пиксель
    pub const fn bits_per_pixel(&self) -> u8 {
        match self {
            PixelFormat::Rgb888 | PixelFormat::Bgr888 => 24,
            PixelFormat::Argb8888 | PixelFormat::Abgr8888 
            | PixelFormat::Xrgb8888 | PixelFormat::Xbgr8888 => 32,
        }
    }
    
    /// Получение количества байт на пиксель
    pub const fn bytes_per_pixel(&self) -> u8 {
        (self.bits_per_pixel() + 7) / 8
    }
}

/// Цвет в формате RGBA
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Чёрный цвет
    pub const BLACK: Color = Color::new(0, 0, 0, 255);
    /// Белый цвет
    pub const WHITE: Color = Color::new(255, 255, 255, 255);
    /// Красный цвет
    pub const RED: Color = Color::new(255, 0, 0, 255);
    /// Зелёный цвет
    pub const GREEN: Color = Color::new(0, 255, 0, 255);
    /// Синий цвет
    pub const BLUE: Color = Color::new(0, 0, 255, 255);
    /// Жёлтый цвет
    pub const YELLOW: Color = Color::new(255, 255, 0, 255);
    /// Голубой (Cyan)
    pub const CYAN: Color = Color::new(0, 255, 255, 255);
    /// Пурпурный (Magenta)
    pub const MAGENTA: Color = Color::new(255, 0, 255, 255);
    /// Серый цвет
    pub const GRAY: Color = Color::new(128, 128, 128, 255);
    /// Тёмно-серый
    pub const DARK_GRAY: Color = Color::new(64, 64, 64, 255);
    /// Оранжевый (акцент Zarya)
    pub const ORANGE: Color = Color::new(255, 140, 0, 255);
    /// Золотой (акцент Zarya)
    pub const GOLD: Color = Color::new(255, 215, 0, 255);
    
    /// Создание нового цвета
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }
    
    /// Из HEX значения (#RRGGBB)
    pub const fn from_hex(hex: u32) -> Self {
        Color {
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
            a: 255,
        }
    }
    
    /// Преобразование в u32 для записи в framebuffer
    pub fn to_u32(&self, format: PixelFormat) -> u32 {
        match format {
            PixelFormat::Rgb888 => {
                ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
            }
            PixelFormat::Bgr888 => {
                ((self.b as u32) << 16) | ((self.g as u32) << 8) | (self.r as u32)
            }
            PixelFormat::Argb8888 => {
                ((self.a as u32) << 24) | ((self.r as u32) << 16) 
                | ((self.g as u32) << 8) | (self.b as u32)
            }
            PixelFormat::Abgr8888 => {
                ((self.a as u32) << 24) | ((self.b as u32) << 16) 
                | ((self.g as u32) << 8) | (self.r as u32)
            }
            PixelFormat::Xrgb8888 | PixelFormat::Xbgr8888 => {
                // X бит игнорируется
                ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
            }
        }
    }
}

impl FrameBuffer {
    /// Инициализация framebuffer
    pub fn init(base_addr: u64, width: u32, height: u32, 
                pitch: u32, format: PixelFormat) {
        let bytes_per_pixel = format.bytes_per_pixel();
        let size = (pitch as usize) * (height as usize);
        
        let fb = FrameBuffer {
            base_addr,
            size,
            width,
            height,
            bytes_per_pixel,
            pitch,
            format,
        };
        
        *FRAMEBUFFER.lock() = Some(fb);
        
        kernel_log!("[FB] Framebuffer initialized:\n");
        kernel_log!("  Base: 0x{:x}, Size: {} bytes\n", base_addr, size);
        kernel_log!("  Resolution: {}x{}\n", width, height);
        kernel_log!("  Pitch: {}, BPP: {}\n", pitch, bytes_per_pixel);
    }
    
    /// Получение ссылки на framebuffer
    pub fn get() -> Option<&'static FrameBuffer> {
        FRAMEBUFFER.lock().as_ref()
    }
    
    /// Запись пикселя по координатам
    pub fn write_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        
        let offset = (y as usize * self.pitch as usize) 
                   + (x as usize * self.bytes_per_pixel as usize);
        
        let ptr = (self.base_addr + offset as u64) as *mut u32;
        
        unsafe {
            ptr.write_volatile(color.to_u32(self.format));
        }
    }
    
    /// Чтение пикселя
    pub fn read_pixel(&self, x: u32, y: u32) -> Option<Color> {
        if x >= self.width || y >= self.height {
            return None;
        }
        
        let offset = (y as usize * self.pitch as usize) 
                   + (x as usize * self.bytes_per_pixel as usize);
        
        let ptr = (self.base_addr + offset as u64) as *const u32;
        
        unsafe {
            let value = ptr.read_volatile();
            Some(match self.format {
                PixelFormat::Rgb888 | PixelFormat::Xrgb8888 => {
                    Color::new(
                        ((value >> 16) & 0xFF) as u8,
                        ((value >> 8) & 0xFF) as u8,
                        (value & 0xFF) as u8,
                        255,
                    )
                }
                PixelFormat::Bgr888 | PixelFormat::Xbgr8888 => {
                    Color::new(
                        (value & 0xFF) as u8,
                        ((value >> 8) & 0xFF) as u8,
                        ((value >> 16) & 0xFF) as u8,
                        255,
                    )
                }
                PixelFormat::Argb8888 => {
                    Color::new(
                        ((value >> 16) & 0xFF) as u8,
                        ((value >> 8) & 0xFF) as u8,
                        (value & 0xFF) as u8,
                        ((value >> 24) & 0xFF) as u8,
                    )
                }
                PixelFormat::Abgr8888 => {
                    Color::new(
                        (value & 0xFF) as u8,
                        ((value >> 8) & 0xFF) as u8,
                        ((value >> 16) & 0xFF) as u8,
                        ((value >> 24) & 0xFF) as u8,
                    )
                }
            })
        }
    }
    
    /// Заполнение экрана цветом (clear)
    pub fn clear(&mut self, color: Color) {
        for y in 0..self.height {
            for x in 0..self.width {
                self.write_pixel(x, y, color);
            }
        }
    }
    
    /// Отрисовка прямоугольника
    pub fn fill_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: Color) {
        for dy in 0..height {
            for dx in 0..width {
                self.write_pixel(x + dx, y + dy, color);
            }
        }
    }
    
    /// Отрисовка рамки
    pub fn draw_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: Color) {
        // Верхняя и нижняя границы
        for dx in 0..width {
            self.write_pixel(x + dx, y, color);
            self.write_pixel(x + dx, y + height - 1, color);
        }
        
        // Левая и правая границы
        for dy in 0..height {
            self.write_pixel(x, y + dy, color);
            self.write_pixel(x + width - 1, y + dy, color);
        }
    }
    
    /// Отрисовка линии (алгоритм Брезенхема)
    pub fn draw_line(&mut self, x0: u32, y0: u32, x1: u32, y1: u32, color: Color) {
        let mut x0 = x0 as i32;
        let mut y0 = y0 as i32;
        let x1 = x1 as i32;
        let y1 = y1 as i32;
        
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        
        let mut err = if dx > dy { dx } else { dy } / 2;
        
        loop {
            if x0 >= 0 && y0 >= 0 && (x0 as u32) < self.width && (y0 as u32) < self.height {
                self.write_pixel(x0 as u32, y0 as u32, color);
            }
            
            if x0 == x1 && y0 == y1 {
                break;
            }
            
            let e2 = err;
            if e2 > dx {
                err -= dy;
                x0 += sx;
            }
            if e2 < dy {
                err += dx;
                y0 += sy;
            }
        }
    }
    
    /// Отрисовка круга (алгоритм Брезенхема для окружности)
    pub fn draw_circle(&mut self, cx: u32, cy: u32, radius: u32, color: Color) {
        let mut x = 0i32;
        let mut y = radius as i32;
        let mut d = 3 - 2 * radius as i32;
        
        while y >= x {
            // Восьмая часть круга с симметрией
            self.plot_circle_points(cx, cy, x, y, color);
            self.plot_circle_points(cx, cy, x, -y, color);
            self.plot_circle_points(cx, cy, -x, y, color);
            self.plot_circle_points(cx, cy, -x, -y, color);
            self.plot_circle_points(cx, cy, y, x, color);
            self.plot_circle_points(cx, cy, y, -x, color);
            self.plot_circle_points(cx, cy, -y, x, color);
            self.plot_circle_points(cx, cy, -y, -x, color);
            
            x += 1;
            if d > 0 {
                y -= 1;
                d += 4 * (x - y) + 10;
            } else {
                d += 4 * x + 6;
            }
        }
    }
    
    fn plot_circle_points(&mut self, cx: u32, cy: u32, x: i32, y: i32, color: Color) {
        let points = [
            (cx.wrapping_add_signed(x), cy.wrapping_add_signed(y)),
            (cx.wrapping_add_signed(x), cy.wrapping_sub_signed(y)),
            (cx.wrapping_sub_signed(x), cy.wrapping_add_signed(y)),
            (cx.wrapping_sub_signed(x), cy.wrapping_sub_signed(y)),
        ];
        
        for (px, py) in points.iter() {
            if *px < self.width && *py < self.height {
                self.write_pixel(*px, *py, color);
            }
        }
    }
}

/// Инициализация драйвера framebuffer
pub fn init() {
    // В реальной системе здесь будет получение информации от загрузчика
    // Для демонстрации используем фиктивные параметры
    
    // Типичные параметры для QEMU с std VGA
    let base_addr = 0xE0000000u64;
    let width = 1024u32;
    let height = 768u32;
    let pitch = 4096u32;
    let format = PixelFormat::Xrgb8888;
    
    FrameBuffer::init(base_addr, width, height, pitch, format);
    
    kernel_log!("[FB] Driver initialized\n");
}

/// Удобные функции для рисования
pub mod draw {
    use super::*;
    
    /// Заполнить экран цветом
    pub fn clear(color: Color) {
        if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
            fb.clear(color);
        }
    }
    
    /// Нарисовать пиксель
    pub fn pixel(x: u32, y: u32, color: Color) {
        if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
            fb.write_pixel(x, y, color);
        }
    }
    
    /// Нарисовать прямоугольник
    pub fn rect(x: u32, y: u32, w: u32, h: u32, color: Color) {
        if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
            fb.fill_rect(x, y, w, h, color);
        }
    }
    
    /// Нарисовать линию
    pub fn line(x0: u32, y0: u32, x1: u32, y1: u32, color: Color) {
        if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
            fb.draw_line(x0, y0, x1, y1, color);
        }
    }
    
    /// Нарисовать круг
    pub fn circle(cx: u32, cy: u32, r: u32, color: Color) {
        if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
            fb.draw_circle(cx, cy, r, color);
        }
    }
}

/// Получение размеров экрана
pub fn get_resolution() -> Option<(u32, u32)> {
    FRAMEBUFFER.lock().as_ref().map(|fb| (fb.width, fb.height))
}
