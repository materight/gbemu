use crate::cpu::CPU;
use crate::joypad::Joypad;
use crate::lcd::LCDBuffer;


pub struct GBEmu {
    pub cpu: CPU,
}

impl GBEmu {
    pub fn new(rom: &[u8]) -> Self {
        Self { cpu: CPU::new(rom) }
    }

    pub fn step(&mut self, joypad: &Joypad) -> Option<&LCDBuffer> {
        self.cpu.mmu.set_joypad(joypad);
        let elapsed_ticks = self.cpu.fetch_execute();
        let frame_buffer = self.cpu.mmu.ppu_execute(elapsed_ticks);
        frame_buffer
    }

    pub fn set_palette(&mut self, palette_idx: usize) {
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

