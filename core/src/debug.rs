use std::fmt::Write;

use crate::cpu::CPU;
use crate::instructions::Op;
use crate::lcd::LCD;
use crate::ppu::PPU;
use crate::registers::R8;
use crate::utils::Get;

static mut DEBUG_ENABLED: bool = false;

pub const TILE_NCOLS: usize = 32;
pub const TILE_NROWS: usize = 768 / TILE_NCOLS;
pub const TILEW: usize = TILE_NCOLS * 8;
pub const TILEH: usize = TILE_NROWS * 8;

pub fn set_enabled(val: bool) {
    unsafe { DEBUG_ENABLED = val }
}

pub fn enabled() -> bool {
    unsafe { DEBUG_ENABLED }
}

pub fn print_cpu_status(cpu: &CPU, opcode_byte: u8, opcode: Op, extra_bytes: u8, xbyte: Option<u8>, xword: Option<u16>) {
    let mut log = String::new();
    // Print OP
    write!(log, "{:#06x}: [{:#04x}] ", cpu.reg.pc - extra_bytes as u16 - 1, opcode_byte).unwrap();
    write!(log, "{:?}", opcode).unwrap();
    match extra_bytes {
        1 => write!(log, "[{:#04x}]", xbyte.unwrap()).unwrap(),
        2 => write!(log, "[{:#06x}]", xword.unwrap()).unwrap(),
        _ => (),
    }
    for _ in 0..(40 - log.len() as i32) {
        write!(log, " ").unwrap()
    }
    // Print registers
    for r in [R8::A, R8::B, R8::C, R8::D, R8::E, R8::H, R8::L] {
        write!(log, "{:?}={:#04x} ", r, cpu.r(r)).unwrap();
    }
    write!(
        log,
        "Z={} N={} H={} C={} ",
        cpu.reg.f.z as u8, cpu.reg.f.n as u8, cpu.reg.f.h as u8, cpu.reg.f.c as u8
    )
    .unwrap();
    write!(log, "SP={:#06x} ", cpu.reg.sp).unwrap();
    println!("{}", log);
}

pub fn draw_tilemap(ppu: &PPU, out: &mut [u8]) {
    for tile_nr in 0..768 {
        for row_idx in 0..8 {
            let vbank = tile_nr >= 384;
            let tile_row_addr = row_idx as u16 * 2 + 0x8000 + ((tile_nr % 384) as u16) * 16;
            let tile_row_l = ppu.vram[PPU::vram_addr(tile_row_addr, vbank)];
            let tile_row_h = ppu.vram[PPU::vram_addr(tile_row_addr + 1, vbank)];
            for i in 0..8 {
                let px = (tile_row_l >> (7 - i) & 1) | ((tile_row_h >> (7 - i) & 1) << 1);
                let (x, y) = ((tile_nr % TILE_NCOLS) * 8 + i, (tile_nr / TILE_NCOLS) * 8 + row_idx);
                let color = LCD::to_color_dmg(px, 0b11100100, 0);
                let idx = 4 * (x + y * TILE_NCOLS * 8);
                out[idx..idx + 4].copy_from_slice(&color.to_be_bytes());
            }
        }
    }
}
