use crate::clock::Clock;
use crate::joypad::Joypad;
use crate::lcd::LCDBuffer;
use crate::mbc::MBC;
use crate::ppu::PPU;

const WRAM_SIZE: usize = 0x4000;
const HRAM_SIZE:usize = 0x0080;

#[allow(non_snake_case)]
pub struct MMU {
    pub mbc: MBC,
    wram: [u8; WRAM_SIZE],
    hram: [u8; HRAM_SIZE],
    pub ppu: PPU,
    pub clock: Clock,

    pub IF: u8,
    pub IE: u8,
    joypad: Joypad, joyp: u8,
}

impl MMU {

    pub fn new(rom: &[u8]) -> Self { 
        Self {
            mbc: MBC::new(&rom),
            wram: [0; WRAM_SIZE],
            hram: [0; HRAM_SIZE],
            ppu: PPU::new(),
            clock: Clock::new(),
            IF: 0, IE: 0,
            joypad: Joypad::default(), joyp: 0,
        }
    }

    pub fn r(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF /*  ROM   */ => self.mbc.r(addr),
            0x8000..=0x9FFF /*  VRAM  */ => self.ppu.r(addr),
            0xA000..=0xBFFF /* ExtRAM */ => self.mbc.r(addr),
            0xC000..=0xDFFF /*  WRAM  */ => self.wram[(addr - 0xC000) as usize],
            0xE000..=0xFDFF /* Mirror */ => self.wram[(addr - 0xE000) as usize],
            0xFE00..=0xFE9F /*  OAM   */ => self.ppu.r(addr),

            0xFEA0..=0xFEFF /*  N/A   */ => 0xFF,
            0xFF00          /* Joypad */ => self.joypad.get(self.joyp),
            0xFF01..=0xFF02 /* Serial */ => 0xFF,
            0xFF04..=0xFF07 /* Clock  */ => self.clock.r(addr),
            0xFF0F          /*   IF   */ => self.IF,
            0xFF10..=0xFF3F /* Audio  */ => 0xFF, // TODO
            0xFF46          /*  DMA   */ => 0xFF,
            0xFF50          /*Boot ROM*/ => self.mbc.boot_rom_unmounted as u8,
            0xFF40..=0xFF6C /* VRAM R */ => self.ppu.r(addr),
            0xFF6D..=0xFF7F /*  I/O   */ => { /*println!("TODO: read registers for {:#06x}", addr);*/ 0x37 },

            0xFF80..=0xFFFE /*  HRAM  */ => self.hram[(addr - 0xFF80) as usize],
            0xFFFF          /*   IE   */ => self.IE,

            0xFF03 | 0xFF08..=0xFF0E  /* Unused */=> 0xFF, 
        }
    }

    pub fn w(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF /*  ROM   */ => self.mbc.w(addr, val),
            0x8000..=0x9FFF /*  VRAM  */ => self.ppu.w(addr, val),
            0xA000..=0xBFFF /* ExtRAM */ => self.mbc.w(addr, val),
            0xC000..=0xDFFF /*  WRAM  */ => self.wram[(addr - 0xC000) as usize] = val,
            0xE000..=0xFDFF /* Mirror */ => self.wram[(addr - 0xE000) as usize] = val,
            0xFE00..=0xFE9F /*  OAM   */ => self.ppu.w(addr, val),

            0xFEA0..=0xFEFF /*  N/A   */ => (),
            0xFF00          /* Joypad */ => self.joyp = val,
            0xFF01..=0xFF02 /* Serial */ => {}/*{print!("{}", val as char)}*/,
            0xFF04..=0xFF07 /* Clock  */ => self.clock.w(addr, val),
            0xFF0F          /*   IF   */ => self.IF = val,
            0xFF10..=0xFF3F /* Audio  */ => (), // TODO
            0xFF46          /*  DMA   */ => self.dma(val),
            0xFF50          /*Boot ROM*/ => self.mbc.boot_rom_unmounted = val != 0,
            0xFF40..=0xFF6C /* VRAM R */ => self.ppu.w(addr, val),
            0xFF6D..=0xFF7F /*  I/O   */ => {/*println!("TODO: registers for {:#06x}", addr)*/},

            0xFF80..=0xFFFE /*  HRAM  */ => self.hram[(addr - 0xFF80) as usize] = val,
            0xFFFF          /*   IE   */ => self.IE = val,

            0xFF03 | 0xFF08..=0xFF0E  /* Unused */=> (), 
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
        let src: u16 = (src as u16) << 8;
        for i in 0..=0x9F {
            self.w(0xFE00 + i, self.r(src + i));
        }
    }

    pub fn set_joypad(&mut self, joypad: &Joypad) {
        self.joypad = joypad.clone();
    }

    pub fn ppu_execute(&mut self, cpu_elapsed_ticks: u8) -> Option<&LCDBuffer> {
        let (frame_buffer, ppu_interrupts) = self.ppu.execute(cpu_elapsed_ticks);
        self.IF |= ppu_interrupts;
        frame_buffer
    }
}
