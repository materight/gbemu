use crate::apu::APU;
use crate::clock::Clock;
use crate::joypad::Joypad;
use crate::lcd::LCDBuffer;
use crate::mbc::MBC;
use crate::ppu::{PPUMode, PPU};

const WRAM_SIZE: usize = 0x8000;
const HRAM_SIZE:usize = 0x0080;

#[allow(non_snake_case)]
#[derive(Clone)]
pub struct MMU {
    pub mbc: MBC,
    wram: [u8; WRAM_SIZE],
    hram: [u8; HRAM_SIZE],
    pub ppu: PPU,
    pub clock: Clock,
    pub apu: APU,

    pub IF: u8,
    pub IE: u8,
    joypad: Joypad,
    joyp: u8,

    pub double_speed: bool,
    wbank: u8,
    hdma: [u8; 4],
    hdma_mode: Option<bool>,
    hdma_len: u8,
    hdma_last_ly: Option<u8>,
}

impl MMU {

    pub fn new(rom: &[u8], force_dmg: bool) -> Self { 
        let mbc = MBC::new(&rom, force_dmg);
        let gcb_mode = mbc.cgb_mode();
        Self {
            mbc: mbc,
            wram: [0; WRAM_SIZE],
            hram: [0; HRAM_SIZE],
            ppu: PPU::new(gcb_mode),
            clock: Clock::new(),
            apu: APU::new(),
            IF: 0, IE: 0,
            joypad: Joypad::default(),
            joyp: 0,
            double_speed: false,
            wbank: 1,
            hdma: [0xFF; 4],
            hdma_mode: None,
            hdma_len: 0,
            hdma_last_ly: None,
        }
    }

    pub fn r(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF /*  ROM   */ => self.mbc.r(addr),
            0x8000..=0x9FFF /*  VRAM  */ => self.ppu.r(addr),
            0xA000..=0xBFFF /* ExtRAM */ => self.mbc.r(addr),
            0xC000..=0xCFFF /*  WRAM  */ => self.wram[(addr - 0xC000) as usize],
            0xD000..=0xDFFF /* WRAM bk*/ => self.wram[(addr - 0xD000 + (self.wbank as u16 * 0x1000)) as usize],
            0xE000..=0xFDFF /* Mirror */ => self.wram[(addr - 0xE000) as usize],
            0xFE00..=0xFE9F /*  OAM   */ => self.ppu.r(addr),

            0xFEA0..=0xFEFF /*  N/A   */ => 0xFF,
            0xFF00          /* Joypad */ => self.joypad.get(self.joyp),
            0xFF01..=0xFF02 /* Serial */ => 0xFF,
            0xFF04..=0xFF07 /* Clock  */ => self.clock.r(addr),
            0xFF0F          /*   IF   */ => self.IF,
            0xFF10..=0xFF3F /*  APU   */ => self.apu.r(addr),
            0xFF46          /*  DMA   */ => 0xFF,
            0xFF4D          /* Speed  */ => (self.double_speed as u8) << 7,
            0xFF50          /*Boot ROM*/ => self.mbc.boot_rom_unmounted as u8,
            0xFF51..=0xFF54 /*  HDMA  */ => self.hdma[(addr - 0xFF51) as usize],
            0xFF55          /*  HDMA  */ => self.hdma_len | if self.hdma_mode == Some(true) { 0x00 } else { 0x80 },
            0xFF40..=0xFF6C /* VRAM R */ => self.ppu.r(addr),
            0xFF70          /* WBank  */ => self.wbank,

            0xFF80..=0xFFFE /*  HRAM  */ => self.hram[(addr - 0xFF80) as usize],
            0xFFFF          /*   IE   */ => self.IE,

            0xFF03 | 0xFF08..=0xFF0E | 0xFF6D..=0xFF7F /* Unused */=> 0xFF,
        }
    }

    pub fn w(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF /*  ROM   */ => self.mbc.w(addr, val),
            0x8000..=0x9FFF /*  VRAM  */ => self.ppu.w(addr, val),
            0xA000..=0xBFFF /* ExtRAM */ => self.mbc.w(addr, val),
            0xC000..=0xCFFF /*  WRAM  */ => self.wram[(addr - 0xC000) as usize] = val,
            0xD000..=0xDFFF /* WRAM BK*/ => self.wram[(addr - 0xD000 + (self.wbank as u16 * 0x1000)) as usize] = val,
            0xE000..=0xFDFF /* Mirror */ => self.wram[(addr - 0xE000) as usize] = val,
            0xFE00..=0xFE9F /*  OAM   */ => self.ppu.w(addr, val),

            0xFEA0..=0xFEFF /*  N/A   */ => (),
            0xFF00          /* Joypad */ => self.joyp = val,
            0xFF01..=0xFF02 /* Serial */ => (),
            0xFF04..=0xFF07 /* Clock  */ => self.clock.w(addr, val),
            0xFF0F          /*   IF   */ => self.IF = val,
            0xFF10..=0xFF3F /*  APU   */ => self.apu.w(addr, val),
            0xFF46          /*  DMA   */ => self.dma(val),
            0xFF4D          /* Speed  */ => if val & 0x01 != 0 { self.double_speed = !self.double_speed },
            0xFF50          /*Boot ROM*/ => self.mbc.boot_rom_unmounted = val != 0,
            0xFF51..=0xFF54 /*  HDMA  */ => self.hdma[(addr - 0xFF51) as usize] = val,
            0xFF55          /*  HDMA  */ => self.wvdma(val),
            0xFF40..=0xFF6C /* VRAM R */ => self.ppu.w(addr, val),
            0xFF70          /* WBank  */ => self.wbank = if val & 0x07 == 0 { 0x01 } else { val & 0x07 },

            0xFF80..=0xFFFE /*  HRAM  */ => self.hram[(addr - 0xFF80) as usize] = val,
            0xFFFF          /*   IE   */ => self.IE = val,

            0xFF03 | 0xFF08..=0xFF0E | 0xFF6D..=0xFF7F /* Unused */=> (),
        }
    }

    pub fn rw(&self, addr: u16) -> u16 {
        u16::from_le_bytes([self.r(addr), self.r(addr + 1)])
    }

    pub fn ww(&mut self, addr: u16, val: u16) {
        let [bl, bh] = val.to_le_bytes();
        self.w(addr, bl);
        self.w(addr + 1, bh);
    }

    fn dma(&mut self, src: u8) {
        let src = (src as u16) << 8;
        for i in 0..=0x9F {
            self.w(0xFE00 + i, self.r(src + i));
        }
    }

    fn wvdma(&mut self, val: u8) {
        let mode = val & 0x80 != 0;
        if self.hdma_mode == None { // Start VDMA
            self.hdma_mode = Some(mode);
            self.hdma_len = val & 0x7F;
            self.hdma_last_ly = if mode { Some(self.ppu.ly) } else { None };
        } else if !mode { // Interrupt VDMA
            self.hdma_mode = None;
            self.hdma_last_ly = None;
        }
    }

    fn step_hdma(&mut self) {
        let src = u16::from_be_bytes([self.hdma[0], self.hdma[1]]) & 0xFFF0;
        let dst = u16::from_be_bytes([self.hdma[2], self.hdma[3]]) & 0x1FF0 | 0x8000;
        for i in 0..0x10 {
            self.w(dst + i, self.r(src + i));
        }
        [self.hdma[0], self.hdma[1]] = (src + 0x10).to_be_bytes();
        [self.hdma[2], self.hdma[3]] = (dst + 0x10).to_be_bytes();
        if self.hdma_len > 0 {
            self.hdma_len -= 1;
        } else {
            self.hdma_len = 0x7F;
            self.hdma_mode = None;
            self.hdma_last_ly = None;
        }
    }

    fn step_vdma(&mut self) -> u16 {
        if self.hdma_mode == Some(true) { // HDMA: single block per HBlank
            if self.ppu.mode() == PPUMode::HBLANK && self.hdma_last_ly != Some(self.ppu.ly)  {
                self.hdma_last_ly = Some(self.ppu.ly);
                self.step_hdma();
                8 * 4
            } else { 0 }
        } else if self.hdma_mode == Some(false) { // GDMA: immediate transfer
            while self.hdma_mode != None {
                self.step_hdma();
            } 0
        } else { 0 } // Disabled
    }

    pub fn set_joypad(&mut self, joypad: &Joypad) {
        self.joypad = joypad.clone();
    }

    pub fn step(&mut self, mut elapsed_ticks: u16) -> Option<&LCDBuffer> {
        // Perform HDMA/GDMA transfer if needed
        elapsed_ticks += self.step_vdma();

        // Update internal clock. In double speed mode, the clock also run at double speed.
        self.IF |= self.clock.step(elapsed_ticks * if self.double_speed { 2 } else { 1 }); 

        // Update PPU status
        let (frame_buffer, ppu_interrupts) = self.ppu.step(elapsed_ticks);
        self.IF |= ppu_interrupts;
        frame_buffer
    }
}
