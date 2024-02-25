pub const LCDW: usize = 160;
pub const LCDH: usize = 144;
pub const LCD_BUFFER_SIZE: usize = LCDW * LCDH;

pub type LCDBuffer =  [u32; LCD_BUFFER_SIZE];

pub struct LCD {
    pub buffer: LCDBuffer,
    color_nr: [u8; LCD_BUFFER_SIZE],
    pub palette: palette::Palette
}

impl LCD {
    pub fn new() -> Self {
        Self {
            buffer: [0; LCD_BUFFER_SIZE],
            color_nr: [0; LCD_BUFFER_SIZE],
            palette: palette::PALETTES[1].1
        }
    }

    fn get_idx(x: u8, y: u8) -> usize {
        (x as usize) + (y as usize) * LCDW
    }

    pub fn to_color(&self, val: u8, palette: u8) -> u32 {
        let color_idx = match val {
            0 => (palette & 0x03) >> 0,
            1 => (palette & 0x0C) >> 2,
            2 => (palette & 0x30) >> 4,
            3 => (palette & 0xC0) >> 6,
            _ => panic!("Color ID {} not supported", val)
        };
        self.palette[color_idx as usize]
    }

    pub fn w(&mut self, x: u8, y: u8, val: u8, palette: u8) {
        let idx = LCD::get_idx(x, y);
        self.buffer[idx] = self.to_color(val, palette);
        self.color_nr[idx] = val;
    }

    pub fn r(&mut self, x: u8, y: u8) -> u8 {
        self.color_nr[LCD::get_idx(x, y)]
    }

    pub fn set_palette(&mut self, idx: usize) {
        self.palette = palette::PALETTES[idx].1;
    }
}


pub mod palette{
    pub type Palette = [u32; 4];

    pub const PALETTES: [(&str, Palette); 3] = [
        ("Default", [0xff9bbc0f, 0xff8bac0f, 0xff306230, 0xff0f380f]),
        (  "Light", [0xffc5dbd4, 0xff778e98, 0xff41485d, 0xff221e31]),
        (  "Coral", [0xffffd0a4, 0xfff4949c, 0xff7c9aac, 0xff68518a]),
    ];
}
