use crate::cpu::CPU;
use crate::joypad::Joypad;
use crate::lcd::LCDBuffer;


pub struct GBEmu {
    pub cpu: CPU,
}

impl GBEmu {
    pub fn new(rom: &[u8], force_dmg: bool) -> Self {
        Self { cpu: CPU::new(rom, force_dmg) }
    }

    pub fn step(&mut self, joypad: &Joypad) -> Option<&LCDBuffer> {
        self.cpu.mmu.set_joypad(joypad);
        let elapsed_ticks = self.cpu.step();
        let frame_buffer = self.cpu.mmu.step(elapsed_ticks);
        frame_buffer
    }

    pub fn current_palette(&self) -> i16 {
        self.cpu.mmu.ppu.lcd.palette_idx
    }

    pub fn set_palette(&mut self, palette_idx: i16) {
        self.cpu.mmu.ppu.lcd.set_palette(palette_idx);
    }

    pub fn rom_title(&self) -> String {
        self.cpu.mmu.mbc.title()
    }

    pub fn rom_checksum(&self) -> u16 {
        self.cpu.mmu.mbc.checksum()
    }

    pub fn save(&self) -> &[u8] {
        self.cpu.mmu.mbc.save()
    }

    pub fn load_save(&mut self, save: &[u8]) {
        self.cpu.mmu.mbc.load(save)
    }
}

