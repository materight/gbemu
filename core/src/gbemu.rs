use std::collections::VecDeque;

use crate::cpu::CPU;
use crate::debug;
use crate::joypad::Joypad;
use crate::lcd::LCD;

const REWIND_FREQ: usize = 2;
const REWIND_MAX_LEN: usize = 20; // In seconds
const MAX_NUM_STATES: usize = (60 / REWIND_FREQ) * REWIND_MAX_LEN;

pub struct GBEmu {
    cpu: CPU,
    lcd: LCD,

    frame_count: usize,
    states: VecDeque<CPU>,
    last_state_frame: usize,
}

impl GBEmu {
    pub fn new(rom: &[u8], force_dmg: bool) -> Self {
        Self {
            cpu: CPU::new(rom, force_dmg),
            lcd: LCD::new(),
            frame_count: 0,
            states: VecDeque::with_capacity(MAX_NUM_STATES),
            last_state_frame: 0,
        }
    }

    pub fn step(&mut self) -> Option<&LCD> {
        // Save state once every frame
        if self.frame_count % REWIND_FREQ == 0 && self.last_state_frame != self.frame_count {
            self.states.push_back(self.cpu.clone());
            if self.states.len() >= MAX_NUM_STATES {
                self.states.pop_front();
            }
            self.last_state_frame = self.frame_count
        }

        // Tick cpu and the rest of the devices
        let elapsed_ticks = self.cpu.step();
        let frame_ready = self.cpu.mmu.step(&mut self.lcd, elapsed_ticks);

        if frame_ready {
            self.frame_count += 1;
            Some(&self.lcd)
        } else {
            None
        }
    }

    pub fn set_joypad(&mut self, joypad: &Joypad) {
        self.cpu.mmu.joypad = *joypad;
    }

    pub fn audio_buffer(&self) -> &[f32] {
        &self.cpu.mmu.apu.buffer
    }

    pub fn clear_audio_buffer(&mut self) {
        self.cpu.mmu.apu.buffer.clear();
    }

    pub fn can_rewind(&self) -> bool {
        !self.states.is_empty()
    }

    pub fn rewind(&mut self) -> Option<&LCD> {
        if let Some(last_state) = self.states.pop_back() {
            self.cpu = last_state;
            // Tick until a new frame is ready
            let mut frame_ready = false;
            while !frame_ready {
                let elapsed_ticks = self.cpu.step();
                frame_ready = self.cpu.mmu.step(&mut self.lcd, elapsed_ticks);
            }
            self.cpu.mmu.joypad.reset();
            self.lcd.w_rewind_symbol();
            Some(&self.lcd)
        } else {
            None
        }
    }

    pub fn draw_tilemap(&self, out: &mut [u8]) {
        debug::draw_tilemap(&self.cpu.mmu.ppu, out);
    }

    pub fn current_palette(&self) -> i16 {
        self.lcd.palette_idx
    }

    pub fn set_palette(&mut self, palette_idx: i16) {
        self.lcd.set_palette(palette_idx);
    }

    pub fn current_shader(&self) -> i16 {
        self.lcd.shader_idx
    }

    pub fn set_shader(&mut self, shader_idx: i16) {
        self.lcd.set_shader(shader_idx);
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
