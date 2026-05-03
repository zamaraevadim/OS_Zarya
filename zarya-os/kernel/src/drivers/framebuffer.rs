//! # Драйвер Framebuffer (графический буфер)
//!
//! Обеспечивает низкоуровневый доступ к графическому буферу.
//! Поддерживает рисование пикселей, примитивов и логотипа Zarya.

#![no_std]

use bootloader::boot_info::FrameBuffer;
use core::ptr;

/// Цвета в формате RGB888
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
    
    /// Преобразование в u32 (RGB)
    pub const fn to_u32(&self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }
    
    /// Из u32
    pub const fn from_u32(value: u32) -> Self {
        Self {
            r: ((value >> 16) & 0xFF) as u8,
            g: ((value >> 8) & 0xFF) as u8,
            b: (value & 0xFF) as u8,
        }
    }
}

/// Цвета темы Zarya "Рассвет"
impl Color {
    pub const BLACK: Self = Self::new(0x00, 0x00, 0x00);
    pub const WHITE: Self = Self::new(0xFF, 0xFF, 0xFF);
    pub const DARK_BG: Self = Self::new(0x1A, 0x1A, 0x1A);
    pub const LIGHT_BG: Self = Self::new(0x2D, 0x2D, 0x2D);
    pub const ACCENT_ORANGE: Self = Self::new(0xFF, 0x6B, 0x35);
    pub const ACCENT_GOLD: Self = Self::new(0xFF, 0xA5, 0x00);
    pub const ACCENT_YELLOW: Self = Self::new(0xFF, 0xD7, 0x00);
    pub const TEXT_PRIMARY: Self = Self::new(0xFF, 0xFF, 0xFF);
    pub const TEXT_SECONDARY: Self = Self::new(0xAA, 0xAA, 0xAA);
    pub const PANEL_BG: Self = Self::new(0x1F, 0x1F, 0x1F);
}

/// Информация о framebuffer
pub struct FramebufferInfo {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub buffer_ptr: *mut u8,
    pub buffer_len: usize,
}

/// Драйвер Framebuffer
pub struct Framebuffer {
    info: FramebufferInfo,
}

impl Framebuffer {
    /// Создание из информации загрузчика
    pub fn from_info(fb_info: &FrameBuffer) -> Self {
        let info = FramebufferInfo {
            width: fb_info.info.width,
            height: fb_info.info.height,
            stride: fb_info.info.stride,
            buffer_ptr: fb_info.buffer.as_mut_ptr(),
            buffer_len: fb_info.buffer.len(),
        };
        
        Self { info }
    }
    
    /// Получение ширины экрана
    pub fn width(&self) -> u32 {
        self.info.width
    }
    
    /// Получение высоты экрана
    pub fn height(&self) -> u32 {
        self.info.height
    }
    
    /// Установка пикселя по координатам
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x >= self.info.width || y >= self.info.height {
            return;
        }
        
        let offset = (y * self.info.stride + x * 4) as usize;
        
        unsafe {
            let ptr = self.info.buffer_ptr.add(offset);
            ptr::write(ptr, color.b);     // B
            ptr::write(ptr.add(1), color.g); // G
            ptr::write(ptr.add(2), color.r); // R
            ptr::write(ptr.add(3), 0xFF);    // Alpha
        }
    }
    
    /// Очистка экрана цветом
    pub fn clear_color(&mut self, r: u8, g: u8, b: u8) {
        let color = Color::new(r, g, b);
        
        for y in 0..self.info.height {
            for x in 0..self.info.width {
                self.set_pixel(x, y, color);
            }
        }
    }
    
    /// Рисование прямоугольника
    pub fn draw_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: Color) {
        for dy in 0..height {
            for dx in 0..width {
                self.set_pixel(x + dx, y + dy, color);
            }
        }
    }
    
    /// Рисование прямоугольника с закругленными углами
    pub fn draw_rounded_rect(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        radius: u32,
        color: Color,
    ) {
        // Упрощенная версия - рисуем обычный прямоугольник
        // В полной версии здесь был бы алгоритм для скругления
        self.draw_rect(x, y, width, height, color);
    }
    
    /// Рисование горизонтальной линии
    pub fn draw_hline(&mut self, x: u32, y: u32, length: u32, color: Color) {
        for dx in 0..length {
            self.set_pixel(x + dx, y, color);
        }
    }
    
    /// Рисование вертикальной линии
    pub fn draw_vline(&mut self, x: u32, y: u32, length: u32, color: Color) {
        for dy in 0..length {
            self.set_pixel(x, y + dy, color);
        }
    }
    
    /// Рисование круга (алгоритм Брезенхема)
    pub fn draw_circle(&mut self, cx: u32, cy: u32, radius: u32, color: Color) {
        let mut x = 0i32;
        let mut y = radius as i32;
        let mut d = 3 - 2 * radius as i32;
        
        while y >= x {
            self.plot_circle_points(cx, cy, x as u32, y as u32, color);
            
            if d > 0 {
                d += 4 * (x - y) + 10;
                y -= 1;
            } else {
                d += 4 * x + 6;
            }
            x += 1;
        }
    }
    
    /// Вспомогательная функция для рисования точек круга
    fn plot_circle_points(&mut self, cx: u32, cy: u32, x: u32, y: u32, color: Color) {
        self.set_pixel(cx + x, cy + y, color);
        self.set_pixel(cx - x, cy + y, color);
        self.set_pixel(cx + x, cy - y, color);
        self.set_pixel(cx - x, cy - y, color);
        self.set_pixel(cx + y, cy + x, color);
        self.set_pixel(cx - y, cy + x, color);
        self.set_pixel(cx + y, cy - x, color);
        self.set_pixel(cx - y, cy - x, color);
    }
    
    /// Рисование логотипа Zarya (солнце/рассвет)
    pub fn draw_logo(&mut self) {
        let center_x = self.info.width / 2;
        let center_y = self.info.height / 2;
        
        // Градиентный фон (темно-серый к черному)
        for y in 0..self.info.height {
            let brightness = 0x1A + ((y * 0x10) / self.info.height) as u8;
            let color = Color::new(brightness, brightness, brightness);
            self.draw_hline(0, y, self.info.width, color);
        }
        
        // Солнце (оранжево-золотой градиент)
        let sun_radius = core::cmp::min(self.info.width, self.info.height) / 6;
        
        for r in (0..sun_radius).rev() {
            let t = r as f32 / sun_radius as f32;
            let color = if t > 0.5 {
                // Внешняя часть - оранжевый
                Color::new(
                    0xFF,
                    (0x6B as f32 + (0xA5 - 0x6B) as f32 * (t - 0.5) * 2.0) as u8,
                    0x35,
                )
            } else {
                // Внутренняя часть - золотой
                Color::new(
                    0xFF,
                    (0xA5 as f32 + (0xD7 - 0xA5) as f32 * t * 2.0) as u8,
                    (0x00 as f32 + (0x00) as f32 * t * 2.0) as u8,
                )
            };
            
            // Рисуем кольцо
            for angle in 0..360 {
                let rad = angle as f32 * core::f32::consts::PI / 180.0;
                let x = center_x + (r as f32 * rad.cos()) as u32;
                let y = center_y + (r as f32 * rad.sin()) as u32;
                self.set_pixel(x, y, color);
            }
        }
        
        // Название "ZARYA" под солнцем
        let logo_y = center_y + sun_radius + 40;
        self.draw_text("ZARYA", center_x, logo_y, Color::WHITE, 24);
    }
    
    /// Рисование текста (упрощенное, без шрифтов)
    pub fn draw_text(&mut self, text: &str, x: u32, y: u32, color: Color, size: u32) {
        // В полной версии здесь была бы отрисовка шрифта
        // Для демонстрации рисуем простые прямоугольники вместо букв
        
        let mut current_x = x;
        for _char in text.chars() {
            // Рисуем "букву" как прямоугольник 10x16
            self.draw_rect(current_x, y, size / 2, size, color);
            current_x += size / 2 + 4;
        }
    }
    
    /// Заполнение области цветом
    pub fn fill(&mut self, x: u32, y: u32, width: u32, height: u32, color: Color) {
        self.draw_rect(x, y, width, height, color);
    }
    
    /// Рисование тени (градиент)
    pub fn draw_shadow(&mut self, x: u32, y: u32, width: u32, height: u32, blur: u32) {
        for i in 0..blur {
            let alpha = 0xFF - (i * (0xFF / blur)) as u8;
            let shadow_color = Color::new(0, 0, 0); // Черная тень
            
            // Рисуем тень по краям
            self.draw_hline(x - i as i32 as u32, y + height + i, width + 2 * i, shadow_color);
        }
    }
    
    /// Получение сырого указателя на буфер
    pub fn buffer(&self) -> (*mut u8, usize) {
        (self.info.buffer_ptr, self.info.buffer_len)
    }
}

/// Простой терминал поверх framebuffer
pub struct FramebufferTerminal {
    fb: *mut Framebuffer,
    cursor_x: u32,
    cursor_y: u32,
    char_width: u32,
    char_height: u32,
    fg_color: Color,
    bg_color: Color,
}

impl FramebufferTerminal {
    pub fn new(fb: &mut Framebuffer) -> Self {
        Self {
            fb: fb as *mut Framebuffer,
            cursor_x: 0,
            cursor_y: 0,
            char_width: 8,
            char_height: 16,
            fg_color: Color::WHITE,
            bg_color: Color::BLACK,
        }
    }
    
    pub fn write_char(&mut self, c: char) {
        unsafe {
            let fb = &mut *self.fb;
            
            match c {
                '\n' => {
                    self.cursor_x = 0;
                    self.cursor_y += self.char_height;
                }
                '\t' => {
                    self.cursor_x += 4 * self.char_width;
                }
                '\r' => {
                    self.cursor_x = 0;
                }
                _ => {
                    // Рисуем символ
                    fb.draw_rect(
                        self.cursor_x,
                        self.cursor_y,
                        self.char_width,
                        self.char_height,
                        self.fg_color,
                    );
                    self.cursor_x += self.char_width;
                }
            }
            
            // Проверка выхода за границы
            if self.cursor_y >= fb.height() {
                // В полной версии здесь был бы скроллинг
                self.cursor_y = 0;
            }
        }
    }
    
    pub fn write_str(&mut self, s: &str) {
        for c in s.chars() {
            self.write_char(c);
        }
    }
    
    pub fn clear(&mut self) {
        unsafe {
            let fb = &mut *self.fb;
            fb.clear_color(self.bg_color.r, self.bg_color.g, self.bg_color.b);
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
    }
}
