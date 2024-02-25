use crate::lcd::{LCD, LCDH, LCDW, LCDBuffer};
use crate::cpu::{INT_STAT, INT_VBLANK};
use crate::utils::byte_register;

const VRAM_SIZE: usize = 0x1800;
const BGMAP_SIZE: usize = 0x0800;
const OAM_SIZE: usize = 0x9F00;

const SCANLINE_TICKS: u16 = 456;
const LY_MAX: u8 = 154;


byte_register!(LCDControl { lcd_enable, window_mode, window_enable, tile_mode, bg_mode, obj_size, obj_enable, bg_enable });
byte_register!(LCDStatus { _7, lyc_int, mode2_int, mode1_int, mode0_int, ly_eq_lyc, ppu_mode_1, ppu_mode_0 });

byte_register!(OBJFlags { bg_priority, y_flip, x_flip, obp, bank, gcbp_2, gcbp_1, gcbp_0 });

pub struct PPU {
    pub lcd: LCD,

    vram: [u8; VRAM_SIZE],
    bgmap: [u8; BGMAP_SIZE],
    oam: [u8; OAM_SIZE],

    lcdc: LCDControl,   // LCD control register
    lcdstat: LCDStatus, // LCD status register
    scy: u8,  // Background Y coord
    scx: u8,  // Background X coord
    ly: u8,   // LCD Y scanline coord
    lyc: u8,  // LCD Y scanline coord comparison
    bgp: u8,  // Background/window palette
    obp0: u8, // Object palette
    obp1: u8, // Object palette
    wy: u8,   // Window Y coord
    wx: u8,   // Window X coord
    wly: u8,   // Count lines with window pixels in it

    scanline_ticks: u16,
}

impl PPU {
    pub fn new() -> Self {
        Self {
            lcd: LCD::new(),
            vram: [0; VRAM_SIZE], oam: [0; OAM_SIZE], bgmap: [0; BGMAP_SIZE],
            lcdc: LCDControl::from(0), lcdstat: LCDStatus::from(0),
            scy: 0, scx: 0, ly: 0, lyc: 0,
            bgp: 0, obp0: 0, obp1: 0, wy: 0, wx: 0, wly: 0,
            scanline_ticks: 0,
        }
    }


    pub fn r(&self, addr: u16) -> u8 {
        match addr {
            /* VRAM */
            0x8000..=0x97FF => self.vram[addr as usize - 0x8000],
            /* Tilemaps */
            0x9800..=0x9FFF => self.bgmap[addr as usize - 0x9800],
            /* OAM */
            0xFE00..=0xFE9F => self.oam[addr as usize - 0xFE00],
            /* Registers */
            0xFF40 => u8::from(&self.lcdc),
            0xFF41 => u8::from(&self.lcdstat),
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            0xFF4C..=0xFF6C => 0xFF,
            _ => panic!("Address {:#06x} not part of PPU", addr),
        }
    }


    pub fn w(&mut self, addr: u16, val: u8) {
        match addr {
            /* VRAM */
            0x8000..=0x97FF => self.vram[addr as usize - 0x8000] = val,
            /* Tilemap 1 and 2 */
            0x9800..=0x9FFF => self.bgmap[addr as usize - 0x9800] = val,
            /* OAM */
            0xFE00..=0xFE9F => self.oam[addr as usize - 0xFE00] = val,
            /* Registers */
            0xFF40 => self.lcdc.w(val),
            0xFF41 => self.lcdstat.w(val & 0xF8), // Mask r/o bits
            0xFF42 => self.scy = val,
            0xFF43 => self.scx = val,
            0xFF44 => (), // LY r/o
            0xFF45 => self.lyc = val,
            0xFF47 => self.bgp = val,
            0xFF48 => self.obp0 = val,
            0xFF49 => self.obp1 = val,
            0xFF4A => self.wy = val,
            0xFF4B => self.wx = val,
            0xFF4C..=0xFF6C => (),
            _ => panic!("Address {:#06x} not part of PPU", addr),
        }
    }

    fn rtilemap(&self, x: u8, y: u8, mode: bool) -> u8 {
        let addr = x as u16 + (y as u16 * 32) + if mode { 0x9C00 } else { 0x9800 };
        self.r(addr)
    }

    fn rtile(&self, tile_nr: u8, row_idx: u8, is_obj: bool) -> u16 {
        let tile_addr = if self.lcdc.tile_mode || is_obj { 
            0x8000 + (tile_nr as u16) * 16
        } else {
            (0x9000 as i32 + (tile_nr as i8 as i32) * 16) as u16
        };
        let row_addr = tile_addr + row_idx as u16 * 2;
        let (row_l, row_h) = (self.r(row_addr) as u16, self.r(row_addr + 1) as u16);
        let mut row_data = 0;
        for i in 0..8 {
            row_data |= ((row_l >> i) & 0x01) << i*2;
            row_data |= ((row_h >> i) & 0x01)<< i*2 + 1;
        }
        row_data
    }

    fn rpx(tile: u16, index: u8) -> u8 {
        ((tile >> ((7 - index) * 2)) & 0x03) as u8
    }

    fn set_ly(&mut self, ly: u8) -> u8 {
        self.ly = ly;
        if ly == 0 { self.wly = 0; }
        self.lcdstat.ly_eq_lyc = self.ly == self.lyc;
        let mut interrupts = 0;
        if self.lcdstat.lyc_int && self.lcdstat.ly_eq_lyc { interrupts |= INT_STAT.0 }
        if self.ly == LCDH as u8 {
            // Mode 1 (VBlank)
            interrupts |= INT_VBLANK.0;
            (self.lcdstat.ppu_mode_1, self.lcdstat.ppu_mode_0) = (false, true);
            if self.lcdstat.mode1_int { interrupts |= INT_STAT.0; }
        }
        interrupts
    }

    fn inc_ticks(&mut self, elapsed_ticks: u8) -> u8 {
        let mut interrupts = 0;
        if self.ly < LCDH as u8 {
            if self.scanline_ticks <= SCANLINE_TICKS && self.scanline_ticks + elapsed_ticks as u16 > SCANLINE_TICKS {
                // Mode 2 (OAM scan)
                (self.lcdstat.ppu_mode_1, self.lcdstat.ppu_mode_0) = (true, false);
                if self.lcdstat.mode2_int { interrupts |= INT_STAT.0; }
            } else if self.scanline_ticks <= 369 && self.scanline_ticks + elapsed_ticks as u16 > 369 {
                // Mode 0 (HBlank)
                (self.lcdstat.ppu_mode_1, self.lcdstat.ppu_mode_0) = (false, false);
                if self.lcdstat.mode0_int { interrupts |= INT_STAT.0; }
            }
        }
        self.scanline_ticks += elapsed_ticks as u16;
        interrupts
    }

    pub fn execute(&mut self, elapsed_ticks: u8) -> (Option<&LCDBuffer>, u8) {
        // Wait until the LCD is enabled to start PPU and reset PPU status.
        if !self.lcdc.lcd_enable {
            self.set_ly(0);
            self.scanline_ticks = 0;
            (self.lcdstat.ppu_mode_1, self.lcdstat.ppu_mode_0) = (false, false);
            return (None, 0)
        }
        let mut interrupts: u8 = 0;
        // Set current mode and trigger interrupt if needed.
        interrupts |= self.inc_ticks(elapsed_ticks);
        // Draw single scanline
        if self.scanline_ticks > SCANLINE_TICKS {
            if self.ly < LCDH as u8 {
                // Draw BG/window
                if self.lcdc.bg_enable {
                    for lx in 0..(LCDW as u8 / 8 + 1) {
                        let tilemap_x = ((self.scx / 8) + lx) % 32;
                        let tilemap_y = self.scy.wrapping_add(self.ly);
                        let tile_nr = self.rtilemap(tilemap_x, tilemap_y / 8, self.lcdc.bg_mode);
                        let tile = self.rtile(tile_nr, tilemap_y % 8, false);
                        for i in 0..8 {
                            let x = (lx * 8) as i16 - (self.scx % 8) as i16 + i as i16;
                            if x < 0 || x >= LCDW as i16 { continue }
                            self.lcd.w(x as u8, self.ly, PPU::rpx(tile, i), self.bgp);
                        }
                    }
                }
                // Draw window
                if self.lcdc.window_enable && self.wy <= self.ly {
                    let wx = self.wx as i16 - 7 ;
                    for lx in 0..(LCDW as u8 / 8 + 1) {
                        let tile_nr = self.rtilemap(lx, self.wly / 8, self.lcdc.window_mode);
                        let tile = self.rtile(tile_nr, self.wly % 8, false);
                        for i in 0..8 {
                            let x = (lx * 8) as i16 + wx + i as i16;
                            if x < 0 || x >= LCDW as i16 { continue }
                            self.lcd.w(x as u8, self.wly + self.wy, PPU::rpx(tile, i), self.bgp);
                        }
                    }
                    self.wly += 1;
                }
                // Draw OBJs
                if self.lcdc.obj_enable {
                    for i in 0..40 {
                        let obj_h = if self.lcdc.obj_size { 16 } else { 8 };
                        let obj_y = self.r(0xFE00 + i * 4) as i16 - 16;
                        if !(obj_y <= (self.ly as i16) && (self.ly as i16) < obj_y + obj_h && obj_y < LCDH as i16) { continue }
                        let obj_x = self.r(0xFE00 + i * 4 + 1) as i16 - 8;
                        let tile_nr = self.r(0xFE00 + i * 4 + 2);
                        let flags = OBJFlags::from(self.r(0xFE00 + i * 4 + 3));
                        let tile_row = if !flags.y_flip { self.ly as i16 - obj_y } else { (obj_h - 1) - (self.ly as i16 - obj_y) };
                        let tile = self.rtile(tile_nr, tile_row as u8, true);
                        // Write pixel by pixel to buffer
                        for i in 0..8 {
                            let x = obj_x + i as i16;
                            if x < 0 || x >= LCDW as i16 { continue }
                            let px = if !flags.x_flip { PPU::rpx(tile, i) } else { PPU::rpx(tile, 7 - i) };
                            // Skip pixel if transparent or if piority is set to BG and BG is not transparent
                            if px == 0 || (flags.bg_priority && self.lcd.r(x as u8, self.ly) != 0) { continue }
                            self.lcd.w(x as u8, self.ly, px, if flags.obp {self.obp1} else {self.obp0});
                        }
                    }
                }
            }
            self.scanline_ticks %= SCANLINE_TICKS;
            interrupts |= self.set_ly(self.ly + 1);
        }

        // Return frame to be drawn when the last scanline has been reached
        let frame = if self.ly >= LY_MAX {
            interrupts |= self.set_ly(0);
            Some(&self.lcd.buffer)
        } else {
            None
        };

        (frame, interrupts)
    }

}

