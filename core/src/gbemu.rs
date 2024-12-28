use std::collections::VecDeque;

use crate::cpu::CPU;
use crate::debug;
use crate::joypad::Joypad;
use crate::lcd::LCDBuffer;

const REWIND_FREQ: usize = 2;
const REWIND_MAX_LEN: usize = 20; // In seconds
const MAX_NUM_STATES: usize = (60 / REWIND_FREQ) * REWIND_MAX_LEN;

pub struct GBEmu {
    cpu: CPU,

    frame_count: usize,
    states: VecDeque<CPU>,
    last_state_frame: usize,
}

impl GBEmu {
    pub fn new(rom: &[u8], force_dmg: bool) -> Self {
        Self {
            cpu: CPU::new(rom, force_dmg),
            frame_count: 0,
            states: VecDeque::with_capacity(MAX_NUM_STATES),
            last_state_frame: 0,
        }
    }

    pub fn step(&mut self, joypad: &Joypad) -> Option<&LCDBuffer> {
        // Save state once every frame
        if self.frame_count % REWIND_FREQ == 0 && self.last_state_frame != self.frame_count {
            self.states.push_back(self.cpu.clone());
            if self.states.len() >= MAX_NUM_STATES {
                self.states.pop_front();
            }
            self.last_state_frame = self.frame_count
        }
        // Tick cpu and the rest of the devices
        self.cpu.mmu.set_joypad(joypad);
        let elapsed_ticks = self.cpu.step();
        let frame_buffer = self.cpu.mmu.step(elapsed_ticks);
        if frame_buffer.is_some() {
            self.frame_count += 1
        }
        frame_buffer
    }

    pub fn can_rewind(&self) -> bool {
        self.states.len() > 0
    }

    pub fn rewind(&mut self) -> Option<&LCDBuffer> {
        if let Some(last_state) = self.states.pop_back() {
            self.cpu = last_state;
            self.cpu.mmu.ppu.lcd.w_rewind_symbol();
            Some(&self.cpu.mmu.ppu.lcd.buffer)
        } else {
            None
        }
    }

    pub fn draw_tilemap(&self) -> Vec<u8> {
        debug::draw_tilemap(&self.cpu.mmu.ppu)
    }

    pub fn current_palette(&self) -> i16 {
        self.cpu.mmu.ppu.lcd.palette_idx
    }

    pub fn set_palette(&mut self, palette_idx: i16) {
        self.cpu.mmu.ppu.lcd.set_palette(palette_idx);
    }

    pub fn current_3d_mode(&self) -> i16 {
        self.cpu.mmu.ppu.lcd.mode_3d_idx
    }

    pub fn set_3d_mode(&mut self, mode_idx: i16) {
        self.cpu.mmu.ppu.lcd.set_3d_mode(mode_idx);
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
