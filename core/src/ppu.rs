use crate::cpu::{INT_STAT, INT_VBLANK};
use crate::lcd::{LCD, LCDH, LCDW};
use crate::utils::pack_bits;

#[rustfmt::skip::macros(byte_register)]
mod ppu_registers {
    use crate::utils::byte_register;

    byte_register!(LCDControl { lcd_enable, window_mode, window_enable, tile_mode, bg_mode, obj_size, obj_enable, bg_enable });
    byte_register!(LCDStatus { _7, lyc_int, mode2_int, mode1_int, mode0_int, ly_eq_lyc, ppu_mode_1, ppu_mode_0 });

    byte_register!(OBJFlags { bg_priority, y_flip, x_flip, obp, bank, cgbp2, cgbp1, cgbp0 });
    byte_register!(BGFlags { bg_priority, y_flip, x_flip, _4, bank, cgbp2, cgbp1, cgbp0 });
}

use ppu_registers::*;

const VRAM_SIZE: usize = 0x4000;
const OAM_SIZE: usize = 0x9F00;

const SCANLINE_TICKS: u16 = 456;
const LY_MAX: u8 = 154;

#[derive(PartialEq, Eq)]
pub struct PPUMode(bool, bool);
impl PPUMode {
    pub const HBLANK: PPUMode = PPUMode(false, false);
    pub const VBLANK: PPUMode = PPUMode(false, true);
    pub const OAM: PPUMode = PPUMode(true, false);
    pub const DRAW: PPUMode = PPUMode(true, true);
}

#[derive(Clone)]
pub struct PPU {
    pub lcd: LCD,
    pub vram: [u8; VRAM_SIZE],
    oam: [u8; OAM_SIZE],

    lcdc: LCDControl,   // LCD control register
    lcdstat: LCDStatus, // LCD status register
    scy: u8,            // Background Y coord
    scx: u8,            // Background X coord
    pub ly: u8,         // LCD Y scanline coord
    lyc: u8,            // LCD Y scanline coord comparison
    bgp: u8,            // Background/window palette
    obp0: u8,           // Object palette
    obp1: u8,           // Object palette
    wy: u8,             // Window Y coord
    wx: u8,             // Window X coord
    wly: u8,            // Count lines with window pixels in it

    cgb_mode: bool,      // Wether the current ROM supports CGB features
    vbank: bool,         // VRAM bank (CGB)
    opri: bool,          // Object priority mode (CGB)
    bgpi: u8,            // BG palette index (CGB)
    obpi: u8,            // OBJ palette index (CGB)
    bgpalette: [u8; 64], // BG palette RAM (CGB)
    obpalette: [u8; 64], // OBJ palette RAM (CGB)

    // Emulator internal state
    scanline_ticks: u16,
    scanline_bg_colors: [u8; LCDW], // BG color indexes
    scanline_bg_pri: [bool; LCDW],  // BG priorities values
}

impl PPU {
    pub fn new(cgb_mode: bool) -> Self {
        Self {
            lcd: LCD::new(),
            vram: [0; VRAM_SIZE],
            oam: [0; OAM_SIZE],
            lcdc: LCDControl::from(0),
            lcdstat: LCDStatus::from(0),
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0,
            obp0: 0,
            obp1: 0,
            wy: 0,
            wx: 0,
            wly: 0,
            cgb_mode: cgb_mode,
            vbank: false,
            opri: true,
            bgpi: 0,
            obpi: 0,
            bgpalette: [0xFF; 64],
            obpalette: [0xFF; 64],
            scanline_ticks: 0,
            scanline_bg_colors: [0; LCDW],
            scanline_bg_pri: [false; LCDW],
        }
    }

    pub fn vram_addr(addr: u16, vbank: bool) -> usize {
        addr as usize - 0x8000 + (vbank as usize * 0x2000)
    }

    pub fn r(&self, addr: u16) -> u8 {
        match addr {
            /* VRAM */
            0x8000..=0x9FFF => self.vram[PPU::vram_addr(addr, self.vbank)],
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
            0xFF4F => self.vbank as u8 | 0xFE,
            0xFF68 => self.bgpi,
            0xFF69 => self.bgpalette[(self.bgpi & 0x3F) as usize],
            0xFF6A => self.obpi,
            0xFF6B => self.obpalette[(self.obpi & 0x3F) as usize],
            0xFF6C => self.opri as u8,
            0xFF4C..=0xFF67 => 0xFF,
            _ => panic!("Address {:#06x} not part of PPU", addr),
        }
    }

    pub fn w(&mut self, addr: u16, val: u8) {
        match addr {
            /* VRAM */
            0x8000..=0x9FFF => self.vram[PPU::vram_addr(addr, self.vbank)] = val,
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
            0xFF4F => self.vbank = val & 0x01 != 0,
            0xFF68 => self.bgpi = val,
            0xFF69 => PPU::wpalette(&mut self.bgpalette, &mut self.bgpi, val),
            0xFF6A => self.obpi = val,
            0xFF6B => PPU::wpalette(&mut self.obpalette, &mut self.obpi, val),
            0xFF6C => self.opri = val & 0x01 != 0,
            0xFF4C..=0xFF67 => (),
            _ => panic!("Address {:#06x} not part of PPU", addr),
        }
    }

    fn rtilemap(&self, x: u8, y: u8, mode: bool, vbank: bool) -> u8 {
        let addr = x as u16 + (y as u16 * 32) + if mode { 0x9C00 } else { 0x9800 };
        self.vram[PPU::vram_addr(addr, vbank)]
    }

    fn rtile(&self, tile_nr: u8, row_idx: u8, is_obj: bool, vbank: bool) -> u16 {
        let tile_addr = if self.lcdc.tile_mode || is_obj {
            0x8000 + (tile_nr as u16) * 16
        } else {
            (0x9000i32 + (tile_nr as i8 as i32) * 16) as u16
        };
        let row_addr = PPU::vram_addr(tile_addr + row_idx as u16 * 2, vbank);
        let (row_l, row_h) = (self.vram[row_addr] as u16, self.vram[row_addr + 1] as u16);
        let mut row_data = 0;
        for i in 0..8 {
            row_data |= ((row_l >> i) & 0x01) << (i * 2);
            row_data |= ((row_h >> i) & 0x01) << (i * 2 + 1);
        }
        row_data
    }

    fn rpx(tile: u16, index: u8, flip: bool) -> u8 {
        let index = if flip { 7 - index } else { index };
        ((tile >> ((7 - index) * 2)) & 0x03) as u8
    }

    fn rpalette(palette: &[u8], addr: u8) -> &[u8] {
        let addr = 8 * addr as usize;
        &palette[addr..addr + 8]
    }

    fn wpalette(palette: &mut [u8], index: &mut u8, val: u8) {
        palette[(*index & 0x3F) as usize] = val;
        if *index & 0x80 != 0 {
            *index = (*index & 0x80) | ((*index).wrapping_add(1) & 0x3F)
        }
    }

    fn set_ly(&mut self, ly: u8) -> u8 {
        self.ly = ly;
        if ly == 0 {
            self.wly = 0;
        }
        self.lcdstat.ly_eq_lyc = self.ly == self.lyc;
        let mut interrupts = 0;
        if self.lcdstat.lyc_int && self.lcdstat.ly_eq_lyc {
            interrupts |= INT_STAT.0
        }
        interrupts
    }

    pub fn mode(&self) -> PPUMode {
        PPUMode(self.lcdstat.ppu_mode_1, self.lcdstat.ppu_mode_0)
    }

    fn update_mode(&mut self) -> (u8, Option<PPUMode>) {
        let current_mode: PPUMode = match self.scanline_ticks {
            _ if self.ly >= LCDH as u8 => PPUMode::VBLANK,
            0..=79 => PPUMode::OAM,
            80..=253 => PPUMode::DRAW,
            _ => PPUMode::HBLANK,
        };
        if self.mode() != current_mode {
            (self.lcdstat.ppu_mode_1, self.lcdstat.ppu_mode_0) = (current_mode.0, current_mode.1); // (0, 1) since bits are little endian
            let interrupts = match current_mode {
                PPUMode::HBLANK if self.lcdstat.mode0_int => INT_STAT.0,
                PPUMode::OAM if self.lcdstat.mode2_int => INT_STAT.0,
                PPUMode::VBLANK => INT_VBLANK.0 | if self.lcdstat.mode1_int { INT_STAT.0 } else { 0 },
                _ => 0,
            };
            (interrupts, Some(current_mode))
        } else {
            (0, None)
        }
    }

    pub fn step(&mut self, elapsed_ticks: u16) -> (Option<&LCD>, u8) {
        // Wait until the LCD is enabled to start PPU and reset PPU status.
        if !self.lcdc.lcd_enable {
            self.set_ly(0);
            self.scanline_ticks = 0;
            (self.lcdstat.ppu_mode_1, self.lcdstat.ppu_mode_0) = (PPUMode::HBLANK.0, PPUMode::HBLANK.1);
            return (None, 0);
        }
        let mut interrupts: u8 = 0;
        // Set current mode and trigger interrupt if needed.
        self.scanline_ticks += elapsed_ticks;
        let (mode_interrupts, new_mode) = self.update_mode();
        interrupts |= mode_interrupts;
        // Draw single scanline when the PPU enters HBlank
        if new_mode == Some(PPUMode::HBLANK) {
            // Draw background
            if self.lcdc.bg_enable || self.cgb_mode {
                for lx in 0..(LCDW as u8 / 8 + 1) {
                    let tilemap_x = ((self.scx / 8) + lx) % 32;
                    let tilemap_y = self.scy.wrapping_add(self.ly);
                    let tile_nr = self.rtilemap(tilemap_x, tilemap_y / 8, self.lcdc.bg_mode, false);
                    let flags = BGFlags::from(self.rtilemap(tilemap_x, tilemap_y / 8, self.lcdc.bg_mode, true));
                    let tile_row = if !flags.y_flip { tilemap_y % 8 } else { 7 - tilemap_y % 8 };
                    let tile = self.rtile(tile_nr, tile_row, false, flags.bank);
                    for i in 0..8 {
                        let x = (lx * 8) as i16 - (self.scx % 8) as i16 + i as i16;
                        if x < 0 || x >= LCDW as i16 {
                            continue;
                        }
                        let px = PPU::rpx(tile, i, flags.x_flip);
                        self.scanline_bg_colors[x as usize] = px;
                        self.scanline_bg_pri[x as usize] = flags.bg_priority;
                        if self.cgb_mode {
                            let cgbp = pack_bits(&[flags.cgbp2, flags.cgbp1, flags.cgbp0]);
                            let palette = PPU::rpalette(&self.bgpalette, cgbp);
                            self.lcd.w_cgb(x as u8, self.ly, px, palette, false);
                        } else {
                            self.lcd.w_dmg(x as u8, self.ly, px, self.bgp, false);
                        }
                    }
                }
            }
            // Draw window
            let wx = self.wx as i16 - 7;
            if self.lcdc.window_enable && (self.lcdc.bg_enable || self.cgb_mode) && self.wy <= self.ly && wx < LCDH as i16 {
                for lx in 0..(LCDW as u8 / 8 + 1) {
                    let tile_nr = self.rtilemap(lx, self.wly / 8, self.lcdc.window_mode, false);
                    let flags = BGFlags::from(self.rtilemap(lx, self.wly / 8, self.lcdc.window_mode, true));
                    let tile_row = if !flags.y_flip { self.wly % 8 } else { 7 - self.wly % 8 };
                    let tile = self.rtile(tile_nr, tile_row, false, flags.bank);
                    for i in 0..8 {
                        let x = (lx * 8) as i16 + wx + i as i16;
                        if x < 0 || x >= LCDW as i16 {
                            continue;
                        }
                        let px = PPU::rpx(tile, i, flags.x_flip);
                        self.scanline_bg_colors[x as usize] = px;
                        self.scanline_bg_pri[x as usize] = flags.bg_priority;
                        if self.cgb_mode {
                            let cgbp = pack_bits(&[flags.cgbp2, flags.cgbp1, flags.cgbp0]);
                            let palette = PPU::rpalette(&self.bgpalette, cgbp);
                            self.lcd.w_cgb(x as u8, self.ly, px, palette, true);
                        } else {
                            self.lcd.w_dmg(x as u8, self.ly, px, self.bgp, true);
                        }
                    }
                }
                self.wly += 1;
            }
            // Draw OBJs
            if self.lcdc.obj_enable {
                let obj_h = if self.lcdc.obj_size { 16 } else { 8 };
                // Select firt 10 objects to be drawn and sort them by priority
                let mut selected_objs = Vec::with_capacity(10);
                for i in 0..40 {
                    let obj_y = self.r(0xFE00 + i * 4) as i16 - 16;
                    if obj_y <= (self.ly as i16) && (self.ly as i16) < obj_y + obj_h && obj_y < LCDH as i16 {
                        let obj_x = self.r(0xFE00 + i * 4 + 1) as i16 - 8;
                        selected_objs.push((i, obj_x, obj_y));
                        if selected_objs.len() >= 10 {
                            break;
                        }
                    }
                }
                // Sort by priority (higher priorities are drawn later so they overwrite lower priorities)
                if self.cgb_mode {
                    selected_objs.sort_by(|(ai, _, _), (bi, _, _)| ai.cmp(&bi).reverse());
                } else {
                    selected_objs.sort_by(|(ai, ax, _), (bi, bx, _)| ax.cmp(&bx).reverse().then(ai.cmp(&bi).reverse()));
                }
                // Draw selected objects
                for (i, obj_x, obj_y) in selected_objs {
                    let tile_nr = self.r(0xFE00 + i * 4 + 2) & if obj_h == 16 { 0xFE } else { 0xFF }; // Last bit is ignored in 8x16 mode
                    let flags = OBJFlags::from(self.r(0xFE00 + i * 4 + 3));
                    let tile_row = if !flags.y_flip {
                        self.ly as i16 - obj_y
                    } else {
                        (obj_h - 1) - (self.ly as i16 - obj_y)
                    };
                    let tile = self.rtile(tile_nr, tile_row as u8, true, flags.bank);
                    // Write pixel by pixel to buffer
                    for i in 0..8 {
                        let x = obj_x + i as i16;
                        if x < 0 || x >= LCDW as i16 {
                            continue;
                        }
                        let px = PPU::rpx(tile, i, flags.x_flip);
                        // Skip pixel if transparent or if piority is set to BG and BG is not transparent
                        let bg_has_priority = self.scanline_bg_colors[x as usize] != 0
                            && if self.cgb_mode {
                                self.lcdc.bg_enable && (flags.bg_priority || self.scanline_bg_pri[x as usize])
                            } else {
                                flags.bg_priority
                            };
                        if px == 0 || bg_has_priority {
                            continue;
                        }
                        // Draw
                        if self.cgb_mode {
                            let cgbp = pack_bits(&[flags.cgbp2, flags.cgbp1, flags.cgbp0]);
                            let palette = PPU::rpalette(&self.obpalette, cgbp);
                            self.lcd.w_cgb(x as u8, self.ly, px, palette, true);
                        } else {
                            self.lcd
                                .w_dmg(x as u8, self.ly, px, if flags.obp { self.obp1 } else { self.obp0 }, true);
                        }
                    }
                }
            }
        } else if self.scanline_ticks > SCANLINE_TICKS {
            // Go to new line when a scanline is done
            self.scanline_ticks %= SCANLINE_TICKS;
            interrupts |= self.set_ly(self.ly + 1);
        }

        // Return frame to be drawn when the last scanline has been reached
        let frame = if self.ly >= LY_MAX {
            interrupts |= self.set_ly(0);
            Some(&self.lcd)
        } else {
            None
        };

        (frame, interrupts)
    }
}
