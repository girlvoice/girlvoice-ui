// display config (round 240x240 1.8" LCD, GC9A01)
use libm::{fabsf};

pub const DISPLAY_SIZE: usize = 240;

// DSP config
pub const CHANNELS: usize = 16;

#[derive(Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Default for Color {
    fn default() -> Self {
        Self { r: 0, g: 0, b: 0 }
    }
}


impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    // use RGB565 for embedded display
    pub fn to_rgb565(self) -> u16 {
        let r = (self.r as u16 >> 3) & 0x1F;
        let g = (self.g as u16 >> 2) & 0x3F;
        let b = (self.b as u16 >> 3) & 0x1F;
        (r << 11) | (g << 5) | b
    }

    // use to 24bit RGB for simulator
    pub fn to_argb32(self) -> u32 {
        0xFF000000 | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    // interpolate between two colors
    pub fn lerp(a: Color, b: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        Color {
            r: (a.r as f32 * (1.0 - t) + b.r as f32 * t) as u8,
            g: (a.g as f32 * (1.0 - t) + b.g as f32 * t) as u8,
            b: (a.b as f32 * (1.0 - t) + b.b as f32 * t) as u8,
        }
    }

    // color from HSV (hue 0-360, sat/val 0-1)
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Color {
        let h = h % 360.0;
        let c = v * s;
        let x = c * (1.0 - fabsf((h / 60.0) % 2.0 - 1.0));
        let m = v - c;

        let (r, g, b) = if h < 60.0 {
            (c, x, 0.0)
        } else if h < 120.0 {
            (x, c, 0.0)
        } else if h < 180.0 {
            (0.0, c, x)
        } else if h < 240.0 {
            (0.0, x, c)
        } else if h < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        Color {
            r: ((r + m) * 255.0) as u8,
            g: ((g + m) * 255.0) as u8,
            b: ((b + m) * 255.0) as u8,
        }
    }
    
    pub fn scale(self, factor: f32) -> Color {
        Color {
            r: (self.r as f32 * factor) as u8,
            g: (self.g as f32 * factor) as u8,
            b: (self.b as f32 * factor) as u8,
        }
    }
}

pub struct ColorPalette {
    pub colors: [Color; 16],
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
}


impl ColorPalette {
    pub fn new() -> Self {
        Self::default()
    }

    // get a color by index
    pub fn get(&self, index: usize) -> Color {
        self.colors[index % 16].clone()
    }

    // get a color by position
    pub fn sample(&self, t: f32) -> Color {
        let t = t.clamp(0.0, 0.9999);
        let idx = (t * 16.0) as usize;
        let frac = t * 16.0 - idx as f32;
        let next_idx = (idx + 1) % 16;
        Color::lerp(self.colors[idx].clone(), self.colors[next_idx].clone(), frac)
    }
}

impl Default for ColorPalette {
    fn default() -> Self {
        // default palette
        let colors = core::array::from_fn(|i| palette::rainbow(i as f32 / 16.0));
        Self {
            colors,
            primary: palette::PINK,
            secondary: palette::CYAN,
            accent: palette::PURPLE,
        }
    }
}
pub mod palette {
    use super::Color;

    pub const PINK: Color = Color::new(255, 20, 147);
    pub const CYAN: Color = Color::new(0, 255, 255);
    pub const PURPLE: Color = Color::new(148, 0, 211);
    pub const MAGENTA: Color = Color::new(255, 0, 255);
    pub const BLUE: Color = Color::new(30, 144, 255);
    pub const GREEN: Color = Color::new(0, 255, 127);
    pub const ORANGE: Color = Color::new(255, 140, 0);
    pub const YELLOW: Color = Color::new(255, 255, 0);
    pub const BLACK: Color = Color::new(0, 0, 0);
    pub const WHITE: Color = Color::new(255, 255, 255);
    
    // get a rainbow gradient based on position (0-1)
    pub fn rainbow(t: f32) -> Color {
        Color::from_hsv(t * 360.0, 1.0, 1.0)
    }
}
