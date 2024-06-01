pub const LCDW: usize = 160;
pub const LCDH: usize = 144;
pub const LCD_BUFFER_SIZE: usize = LCDW * LCDH;

#[derive(Clone)]
pub struct LCDBuffer {
    pub frame: Vec<u32>,
    background: Vec<u32>,
    foreground: Vec<u32>,
}
impl LCDBuffer {
    pub fn new() -> Self {
        Self {
            frame: vec![0; LCD_BUFFER_SIZE],
            background: vec![0; LCD_BUFFER_SIZE],
            foreground: vec![0; LCD_BUFFER_SIZE],
        }
    }

    fn to_idx(x: u8, y: u8) -> usize {
        return (x as usize) + (y as usize) * LCDW;
    }

    fn w(&mut self, x: u8, y: u8, color: u32, is_foreground: bool) {
        let idx =  LCDBuffer::to_idx(x, y);
        self.frame[idx] = color;
        if is_foreground {
            self.foreground[idx] = color;
        } else {
            self.background[idx] = color;
            self.foreground[idx] = 0;
        }
    }

    fn draw_drop_shadow(&mut self, offset_x: i16, offset_y: i16) {
        for x in 0..(LCDW as i16) {
            for y in 0..(LCDH as i16) {
                let idx =  LCDBuffer::to_idx(x as u8, y as u8);
                if self.foreground[idx] != 0 {
                    self.frame[idx] = self.foreground[idx];
                } else {
                    self.frame[idx] = self.background[idx];
                    let (shadow_ref_x, shadow_ref_y) = (x - offset_x, y - offset_y);
                    if 0 <= shadow_ref_x && shadow_ref_x < LCDW as i16 && 0 <= shadow_ref_y && shadow_ref_y < LCDH as i16 {
                        let shadow_ref_idx = LCDBuffer::to_idx(shadow_ref_x as u8, shadow_ref_y as u8);
                        if self.foreground[shadow_ref_idx] != 0 {
                            let [_, r, g, b] = self.frame[idx].to_be_bytes();
                            self.frame[idx] = u32::from_be_bytes([0xFF, r / 4 * 3, g / 4 * 3, b / 4 * 3]);
                        }
                    }
                }
            }
        }
    }

    fn draw_anaglyph_3d(&mut self, offset_background: u8, offset_foreground: u8) {
        for xr in 0..(LCDW as u8) {
            for y in 0..(LCDH as u8) {
                // Retrieve right pixel from original frame
                let idxr: usize =  LCDBuffer::to_idx(xr as u8, y as u8);
                let pxr = if self.foreground[idxr] != 0 { self.foreground[idxr] } else { self.background[idxr] };
                let [_, _, gr, br] = pxr.to_be_bytes();
                // Retrieve left pixel with a different displacement for foreground and background
                let (xl_bg, xl_fg) = (xr + offset_background,  xr + offset_foreground);
                let idxl_fg: usize =  LCDBuffer::to_idx(xl_fg as u8, y as u8);
                let pxl = if xl_fg < LCDW as u8 && self.foreground[idxl_fg] != 0 {
                    self.foreground[idxl_fg]
                } else if xl_bg < LCDW as u8 {
                    let idxl_bg = LCDBuffer::to_idx(xl_bg as u8, y as u8);
                    self.background[idxl_bg]
                } else {
                    0
                };
                let [_, rl, _, _] = pxl.to_be_bytes();
                // Source: https://www.3dtv.at/knowhow/anaglyphcomparison_en.aspx
                self.frame[idxr] = u32::from_be_bytes([0xFF, rl, gr, br]);
            }
        }
    }

}


#[derive(Clone)]
pub struct LCD {
    pub buffer: LCDBuffer,
    pub palette_idx: i16,
    pub mode_3d_idx: i16,
}

impl LCD {
    pub fn new() -> Self {
        Self {
            buffer: LCDBuffer::new(),
            palette_idx: 0,
            mode_3d_idx: 0,
        }
    }

    pub fn set_palette(&mut self, index: i16) {
        self.palette_idx = index.rem_euclid(palette::DMG_PALETTES.len() as i16);
    }

    pub fn set_3d_mode(&mut self, index: i16) {
        self.mode_3d_idx = index.rem_euclid(3);
    }

    pub fn to_color_dmg(&self, val: u8, palette: u8) -> u32 {
        let color_idx = match val {
            0 => (palette & 0x03) >> 0,
            1 => (palette & 0x0C) >> 2,
            2 => (palette & 0x30) >> 4,
            3 => (palette & 0xC0) >> 6,
            _ => panic!("Color ID {} not supported", val)
        };
        palette::DMG_PALETTES[self.palette_idx as usize].1[color_idx as usize]
    }

    pub fn to_color_cgb(&self, val: u8, palette: &[u8]) -> u32 {
        // Get 15bit color from palette
        let color15 =  match val {
            0 => u16::from_le_bytes([palette[0], palette[1]]),
            1 => u16::from_le_bytes([palette[2], palette[3]]),
            2 => u16::from_le_bytes([palette[4], palette[5]]),
            3 => u16::from_le_bytes([palette[6], palette[7]]),
            _ => panic!("Color ID {} not supported", val)
        };
        let (r5, g5, b5) = (color15 & 0x1F, (color15 >> 5) & 0x1F, (color15 >> 10) & 0x1F);
        // Convert to 32bit using color correction
        let r8 = (((r5 * 13 + g5 * 2 + b5) >> 1) & 0xFF) as u8;
        let g8 = (((g5 * 3 + b5) << 1) & 0xFF) as u8;
        let b8 = (((r5 * 3 + g5 * 2 + b5 * 11) >> 1) & 0xFF) as u8;
        // Merge into a single 32bit value 
        (0xFF << 24) | (r8 as u32) << 16 | (g8 as u32) << 8 | (b8 as u32)
    }

    pub fn w_dmg(&mut self, x: u8, y: u8, val: u8, palette: u8, is_foreground: bool) {
        self.buffer.w(x, y, self.to_color_dmg(val, palette), is_foreground);
    }

    pub fn w_cgb(&mut self, x: u8, y: u8, val: u8, palette: &[u8], is_foreground: bool) {
        self.buffer.w(x, y, self.to_color_cgb(val, palette), is_foreground);
    }

    pub fn w_rewind_symbol(&mut self) {
        // Draw two left triangles on top-right corner
        let (size, px, py) = (5, LCDW as u8 - 12, 2);
        for i in 0..2 {
            for y in 0..(size * 2) - 1 {
                let x_start = if y < size { size - y - 1 } else { y - size + 1 };
                for x in x_start..size {
                    self.buffer.w(px + x + (i * size), py + y, 0xffff0000, true);
                }
            }
        }
    }

    pub fn postprocess(&mut self) {
        match self.mode_3d_idx {
            0 => (),
            1 => self.buffer.draw_anaglyph_3d(2, 6),
            2 => self.buffer.draw_drop_shadow(2, 2),
            val => panic!("3D mode {} not supported", val),
        }
    }

}


pub mod palette{
    // Color mappings for DMG
    pub const DMG_PALETTES: [(&str, [u32; 4]); 13] = [
        ( "Default", [0xffc5dbd4, 0xff778e98, 0xff41485d, 0xff221e31]),
        (  "Autumn", [0xffdad3af, 0xffd58863, 0xffc23a73, 0xff2c1e74]),
        (   "Retro", [0xff9bbc0f, 0xff8bac0f, 0xff306230, 0xff0f380f]),
        (   "Earth", [0xfff5f29e, 0xffacb965, 0xffb87652, 0xff774346]),
        (  "Hollow", [0xfffafbf6, 0xffc6b7be, 0xff565a75, 0xff0f0f1b]),
        (    "Mist", [0xffc4f0c2, 0xff5ab9a8, 0xff1e606e, 0xff2d1b00]),
        ("Coldfire", [0xfff6c6a8, 0xffd17c7c, 0xff5b768d, 0xff46425e]),
        (    "Link", [0xffffffb5, 0xff7bc67b, 0xff6b8c42, 0xff5a3921]),
        (    "Pink", [0xfff7bef7, 0xffe78686, 0xff7733e7, 0xff2c2c96]),
        (    "Mint", [0xfffbffe0, 0xff95c798, 0xff856d52, 0xff40332f]),
        ( "Nuclear", [0xffe2f3e4, 0xff94e344, 0xff46878f, 0xff332c50]),
        (  "Rustic", [0xffa96868, 0xffedb4a1, 0xff764462, 0xff2c2137]),
        (    "Wish", [0xff8be5ff, 0xff608fcf, 0xff7550e8, 0xff622e4c]),
    ];
}
