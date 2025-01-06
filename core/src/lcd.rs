use crate::shaders;

pub const LCDW: usize = 160;
pub const LCDH: usize = 144;
pub const LCD_BUFFER_SIZE: usize = LCDW * LCDH;

#[derive(Clone)]
pub struct LCD {
    pub frame: Vec<u32>,
    pub background: Vec<u32>,
    pub foreground: Vec<u32>,

    cgb_mode: bool,
    pub shader_idx: i16,
    pub palette_idx: i16,
}
impl LCD {
    pub fn new() -> Self {
        Self {
            frame: vec![0; LCD_BUFFER_SIZE],
            background: vec![0; LCD_BUFFER_SIZE],
            foreground: vec![0; LCD_BUFFER_SIZE],
            cgb_mode: false,
            shader_idx: 0,
            palette_idx: 0,
        }
    }

    pub fn to_idx(x: usize, y: usize, scale: usize, dx: usize, dy: usize) -> usize {
        (x * scale + dx) + (y * scale + dy) * LCDW * scale
    }

    pub fn set_palette(&mut self, index: i16) {
        self.palette_idx = index.rem_euclid(palette::DMG_PALETTES.len() as i16);
    }

    pub fn set_shader(&mut self, index: i16) {
        self.shader_idx = index.rem_euclid(5);
    }

    pub fn to_color_dmg(&self, val: u8, palette: u8) -> u32 {
        let color_idx = match val {
            0 => (palette & 0x03) >> 0,
            1 => (palette & 0x0C) >> 2,
            2 => (palette & 0x30) >> 4,
            3 => (palette & 0xC0) >> 6,
            _ => panic!("Color ID {} not supported", val),
        };
        palette::DMG_PALETTES[self.palette_idx as usize].1[color_idx as usize]
    }

    pub fn to_color_cgb(&self, val: u8, palette: &[u8]) -> u32 {
        // Get 15bit color from palette
        let color15 = match val {
            0 => u16::from_le_bytes([palette[0], palette[1]]),
            1 => u16::from_le_bytes([palette[2], palette[3]]),
            2 => u16::from_le_bytes([palette[4], palette[5]]),
            3 => u16::from_le_bytes([palette[6], palette[7]]),
            _ => panic!("Color ID {} not supported", val),
        };
        let (r5, g5, b5) = (color15 & 0x1F, (color15 >> 5) & 0x1F, (color15 >> 10) & 0x1F);
        // Convert to 32bit using color correction
        let r8 = (((r5 * 13 + g5 * 2 + b5) >> 1) & 0xFF) as u8;
        let g8 = (((g5 * 3 + b5) << 1) & 0xFF) as u8;
        let b8 = (((r5 * 3 + g5 * 2 + b5 * 11) >> 1) & 0xFF) as u8;
        // Merge into a single 32bit value
        (r8 as u32) << 24 | (g8 as u32) << 16 | (b8 as u32) << 8 | 0xFF
    }

    fn w(&mut self, x: u8, y: u8, color: u32, is_foreground: bool) {
        let idx = LCD::to_idx(x as usize, y as usize, 1, 0, 0);
        self.frame[idx] = color;
        if is_foreground {
            self.foreground[idx] = color;
        } else {
            self.background[idx] = color;
            self.foreground[idx] = 0;
        }
    }

    pub fn w_dmg(&mut self, x: u8, y: u8, val: u8, palette: u8, is_foreground: bool) {
        self.cgb_mode = false;
        self.w(x, y, self.to_color_dmg(val, palette), is_foreground);
    }

    pub fn w_cgb(&mut self, x: u8, y: u8, val: u8, palette: &[u8], is_foreground: bool) {
        self.cgb_mode = true;
        self.w(x, y, self.to_color_cgb(val, palette), is_foreground);
    }

    pub fn w_rewind_symbol(&mut self) {
        // Draw two left triangles on top-right corner
        let (size, px, py) = (5, LCDW as u8 - 12, 2);
        for i in 0..2 {
            for y in 0..(size * 2) - 1 {
                let x_start = if y < size { size - y - 1 } else { y - size + 1 };
                for x in x_start..size {
                    self.w(px + x + (i * size), py + y, 0xff0000ff, true);
                }
            }
        }
    }

    pub fn draw_frame(&self, out: &mut [u8], scale: usize) {
        let dmg_bg_palette = palette::DMG_PALETTES[self.palette_idx as usize].1[0];
        match self.shader_idx {
            0 => shaders::normal(&self.frame, out, scale),
            1 => shaders::lcd(&self, out, scale, if self.cgb_mode { None } else { Some(dmg_bg_palette) }),
            2 => shaders::crt(&self, out, scale),
            3 => shaders::drop_shadow(&self, out, scale, 2, 2),
            4 => shaders::anaglyph_3d(&self, out, scale, 2, 6),
            val => panic!("shader {} not supported", val),
        }
    }
}

#[rustfmt::skip]
pub mod palette{
    // Color mappings for DMG
    pub const DMG_PALETTES: [(&str, [u32; 4]); 13] = [
        ( "Default", [0xc5dbd4ff, 0x778e98ff, 0x41485dff, 0x221e31ff]),
        (     "DMG", [0x818f38ff, 0x647d43ff, 0x566d3fff, 0x314a2dff]),
        (  "Autumn", [0xdad3afff, 0xd58863ff, 0xc23a73ff, 0x2c1e74ff]),
        (   "Earth", [0xf5f29eff, 0xacb965ff, 0xb87652ff, 0x774346ff]),
        (  "Hollow", [0xfafbf6ff, 0xc6b7beff, 0x565a75ff, 0x0f0f1bff]),
        (    "Mist", [0xc4f0c2ff, 0x5ab9a8ff, 0x1e606eff, 0x2d1b00ff]),
        ("Coldfire", [0xf6c6a8ff, 0xd17c7cff, 0x5b768dff, 0x46425eff]),
        (    "Link", [0xffffb5ff, 0x7bc67bff, 0x6b8c42ff, 0x5a3921ff]),
        (    "Pink", [0xf7bef7ff, 0xe78686ff, 0x7733e7ff, 0x2c2c96ff]),
        (    "Mint", [0xfbffe0ff, 0x95c798ff, 0x856d52ff, 0x40332fff]),
        ( "Nuclear", [0xe2f3e4ff, 0x94e344ff, 0x46878fff, 0x332c50ff]),
        (  "Rustic", [0xa96868ff, 0xedb4a1ff, 0x764462ff, 0x2c2137ff]),
        (    "Wish", [0x8be5ffff, 0x608fcfff, 0x7550e8ff, 0x622e4cff]),
    ];
}
