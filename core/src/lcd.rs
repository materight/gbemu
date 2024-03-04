pub const LCDW: usize = 160;
pub const LCDH: usize = 144;
pub const LCD_BUFFER_SIZE: usize = LCDW * LCDH;

pub type LCDBuffer =  [u32; LCD_BUFFER_SIZE];

pub struct LCD {
    pub buffer: LCDBuffer,
    pub palette_idx: i16,
}

impl LCD {
    pub fn new() -> Self {
        Self {
            buffer: [0; LCD_BUFFER_SIZE],
            palette_idx: 0,
        }
    }

    fn get_idx(x: u8, y: u8) -> usize {
        (x as usize) + (y as usize) * LCDW
    }

    pub fn set_palette(&mut self, index: i16) {
        self.palette_idx = index.rem_euclid(palette::DMG_PALETTES.len() as i16);
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

    pub fn w_dmg(&mut self, x: u8, y: u8, val: u8, palette: u8) {
        let idx = LCD::get_idx(x, y);
        self.buffer[idx] = self.to_color_dmg(val, palette);
    }

    pub fn w_cgb(&mut self, x: u8, y: u8, val: u8, palette: &[u8]) {
        let idx = LCD::get_idx(x, y);
        self.buffer[idx] = self.to_color_cgb(val, palette);
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
