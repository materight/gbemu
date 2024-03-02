use std::collections::VecDeque;

use crate::instructions::{Instruction, Op, OPMAP_SIZE, load_opmaps};
use crate::registers::{Registers, CC, R16, R8};
use crate::utils::{Get, Set};
use crate::mmu::MMU;
use crate::debug;

// Interrupts  as (bit masks, address), in order of priority
pub const INT_VBLANK: (u8, u16) = (0x01, 0x0040);
pub const INT_STAT:   (u8, u16) = (0x02, 0x0048);
pub const INT_TIMER:  (u8, u16) = (0x04, 0x0050);
pub const INT_SERIAL: (u8, u16) = (0x08, 0x0058);
pub const INT_JOYPAD: (u8, u16) = (0x10, 0x0060);


pub struct CPU {
    pub reg: Registers,
    pub mmu: MMU,
    ime: bool,
    halt: bool,

    opmap: [Instruction; OPMAP_SIZE],
    opmap_cb: [Instruction; OPMAP_SIZE],

    prev_op: Op,

    // Debugging helper
    opcode_history: VecDeque<Op>,
}


impl CPU {
    pub fn new(rom: &[u8], force_dmg: bool) -> Self {
        let (opmap, opmap_cb) = load_opmaps();
        Self {
            reg: Registers::new(),
            mmu: MMU::new(rom, force_dmg),
            ime: false,
            halt: false,
            opmap,
            opmap_cb,
            prev_op: Op::INVALID,
            opcode_history: VecDeque::new(),
        }
    }

    fn fetch(&mut self) -> u8 {
        let val = self.mmu.r(self.reg.pc);
        self.reg.pc = self.reg.pc.wrapping_add(1);
        val
    }

    pub fn step(&mut self) -> u16 {
        let mut opcycles = 0;

        // Handle interrupts, if any
        opcycles += self.handle_interrupts();

        // If thee CPU is in halted state, skip execution
        if self.halt {
            opcycles += 1;
        } else {
            // Load next OP from memory
            let mut opcode_byte = self.fetch();
            let (mut opcode, mut extra_bytes, mut instr_opcycles) = self.opmap[opcode_byte as usize];
            opcycles += instr_opcycles;
            
            // Read next instuction if the CB preifx is parsed
            if opcode == Op::CB_PREFIX {
                opcode_byte = self.fetch();
                (opcode, extra_bytes, instr_opcycles) = self.opmap_cb[opcode_byte as usize];
                opcycles += instr_opcycles;
            }

            // Load additional bytes if needed
            let xbyte: Option<u8> = if extra_bytes > 0 {Some(self.fetch())} else {None};
            let xword: Option<u16> = if extra_bytes > 1 {Some(u16::from_le_bytes([xbyte.unwrap(), self.fetch()]))} else {None};

            // Debug messages
            if debug::enabled() && self.mmu.mbc.boot_rom_unmounted {
                debug::print_cpu_status(&self, opcode_byte, opcode, extra_bytes, xbyte, xword);
                self.opcode_history.push_front(opcode);
                if self.opcode_history.len() > 8 { self.opcode_history.pop_back(); }
                if [Op::CP_A_I8, Op::JR_CC_I8(CC::NZ), Op::LDH_A_I8, Op::CP_A_I8].iter().rev().enumerate().all(|(i, item)| self.opcode_history.get(i).unwrap_or(&Op::INVALID) == item) {
                    println!("Found target trace at {:#06x}", self.reg.pc - 1);
                }
            }

            // Run corresponding instruction
            match opcode {
                Op::INVALID => panic!("Received INVALID instruction"),
                Op::NOP => (),
                Op::LD_R16_A(r) =>       self.mmu.w(self.r(r), self.reg.a),
                Op::LD_I16_A =>          self.mmu.w(xword.unwrap(), self.reg.a),
                Op::LD_HLID_A(sign) => { self.mmu.w(self.r(R16::HL), self.reg.a); self.inc16_(R16::HL, sign) },
                Op::LDH_C_A =>           self.mmu.w(0xFF00 | self.reg.c as u16, self.reg.a),
                Op::LDH_I8_A =>          self.mmu.w(0xFF00 | xbyte.unwrap() as u16, self.reg.a),
                Op::LD_R16_I16(r) =>     self.w(r, xword.unwrap()),
                Op::LD_A_R16(r) =>       self.reg.a = self.mmu.r(self.r(r)),
                Op::LD_A_I16 =>          self.reg.a = self.mmu.r(xword.unwrap()),
                Op::LD_A_HLID(sign) => { self.reg.a = self.mmu.r(self.r(R16::HL)); self.inc16_(R16::HL, sign) },
                Op::LDH_A_C =>           self.reg.a = self.mmu.r(0xFF00 | self.reg.c as u16),
                Op::LDH_A_I8 =>          self.reg.a = self.mmu.r(0xFF00 | xbyte.unwrap() as u16),
                Op::LD_I16_SP =>         self.mmu.ww(xword.unwrap(), self.r(R16::SP)),
                Op::LD_HL_SPI8 =>      { let res = self.add16i8(R16::SP, xbyte.unwrap()); self.w(R16::HL, res) },
                Op::LD_SP_HL =>          self.w(R16::SP, self.r(R16::HL)),
                Op::LD_R8_I8(r) =>       self.w(r, xbyte.unwrap()),
                Op::LD_R8_R8(r1, r2) =>  self.w(r1, self.r(r2)),

                Op::INC_R8(r) =>      self.inc8_(r),
                Op::DEC_R8(r) =>      self.dec8_(r),
                Op::INC_R16(r) =>     self.inc16_(r, true),
                Op::DEC_R16(r) =>     self.inc16_(r, false),
                Op::ADD_HL_R16(r) =>  self.add16_(R16::HL, self.r(r)),
                Op::ADD_SP_I8 =>      self.add16i8_(R16::SP, xbyte.unwrap()),
                Op::ADD_A_R8(r) =>    self.add8_(R8::A, self.r(r), false),
                Op::ADD_A_I8 =>       self.add8_(R8::A, xbyte.unwrap(), false),
                Op::ADC_A_R8(r) =>    self.add8_(R8::A, self.r(r), true),
                Op::ADC_A_I8 =>       self.add8_(R8::A, xbyte.unwrap(), true),
                Op::SUB_A_R8(r) =>    self.sub8_(R8::A, self.r(r), false),
                Op::SUB_A_I8 =>       self.sub8_(R8::A, xbyte.unwrap(), false),
                Op::SBC_A_R8(r) =>    self.sub8_(R8::A, self.r(r), true),
                Op::SBC_A_I8 =>       self.sub8_(R8::A, xbyte.unwrap(), true),
                Op::AND_A_R8(r) =>    self.and8_(R8::A, self.r(r)),
                Op::AND_A_I8 =>       self.and8_(R8::A, xbyte.unwrap()),
                Op::XOR_A_R8(r) =>    self.xor8_(R8::A, self.r(r)),
                Op::XOR_A_I8 =>       self.xor8_(R8::A, xbyte.unwrap()),
                Op::OR_A_R8(r) =>     self.or8_(R8::A, self.r(r)),
                Op::OR_A_I8 =>        self.or8_(R8::A, xbyte.unwrap()),
                Op::CP_A_R8(r) => _ = self.sub8(R8::A, self.r(r), false),
                Op::CP_A_I8 =>    _ = self.sub8(R8::A, xbyte.unwrap(), false),

                Op::RLCA =>  self.rot_(R8::A, true, false, false),
                Op::RRCA =>  self.rot_(R8::A, false, false, false),
                Op::RLA =>   self.rot_(R8::A, true, true, false),
                Op::RRA =>   self.rot_(R8::A, false, true, false),
                Op::DAA =>   self.daa_(),
                Op::CPL => { self.reg.f.n = true;  self.reg.f.h = true;  self.reg.a = !self.reg.a; },
                Op::SCF => { self.reg.f.n = false; self.reg.f.h = false; self.reg.f.c = true; },
                Op::CCF => { self.reg.f.n = false; self.reg.f.h = false; self.reg.f.c = !self.reg.f.c; },

                Op::PUSH_R16(r) =>     self.push(self.r(r)),
                Op::POP_R16(r) =>      self.pop(r),
                Op::JP_I16 =>          self.jp(xword.unwrap()),
                Op::JP_HL =>           self.jp(self.r(R16::HL)),
                Op::JR_I8 =>           self.jr(xbyte.unwrap()),
                Op::CALL_I16 =>        self.call(xword.unwrap()),
                Op::RST(tgt) =>        self.call((tgt as u16) << 3),
                Op::RET =>             self.pop(R16::PC),
                Op::RETI =>          { self.ime = true; self.pop(R16::PC) },
                Op::JP_CC_I16(cc) =>   if self.r(cc) { self.jp(xword.unwrap()); opcycles += 1; },
                Op::JR_CC_I8(cc) =>    if self.r(cc) { self.jr(xbyte.unwrap()); opcycles += 1; },
                Op::CALL_CC_I16(cc) => if self.r(cc) { self.call(xword.unwrap()); opcycles += 3; },
                Op::RET_CC(cc) =>      if self.r(cc) { self.pop(R16::PC); opcycles += 3; },

                Op::STOP => (),
                Op::HALT => self.halt = true,
                Op::DI =>   self.ime = false,
                Op::EI =>   (),

                Op::CB_PREFIX =>     panic!("CB prefix not handled"),
                Op::CB_RLC_R8(r) =>  self.rot_(r, true, false, true),
                Op::CB_RRC_R8(r) =>  self.rot_(r, false, false, true),
                Op::CB_RL_R8(r) =>   self.rot_(r, true, true, true),
                Op::CB_RR_R8(r) =>   self.rot_(r, false, true, true),
                Op::CB_SLA_R8(r) =>  self.shift_(r, true, true),
                Op::CB_SRA_R8(r) =>  self.shift_(r, false, true),
                Op::CB_SRL_R8(r) =>  self.shift_(r, false, false),
                Op::CB_SWAP_R8(r) => self.swap_(r),
                Op::CB_BIT_R8(bit, r) => self.bit_(bit, r),
                Op::CB_RES_R8(bit, r) => self.res_(bit, r),
                Op::CB_SET_R8(bit, r) => self.set_(bit, r),
            }
            // Set IME to true if previous instruction was EI
            if self.prev_op == Op::EI { self.ime = true; }
            self.prev_op = opcode;
        }

        // Return adjusted T-cycles based on the CPU speep mode
        let tcycles_multiplier = if self.mmu.double_speed { 2 } else { 4 };
        opcycles as u16 * tcycles_multiplier
    }

    fn handle_interrupts(&mut self) -> u8 {
        // Check for enabled interrupts in order of priority
        for (int_flag, int_addr) in [INT_VBLANK, INT_STAT, INT_TIMER, INT_SERIAL, INT_JOYPAD] {
            if self.mmu.IE & int_flag != 0 && self.mmu.IF & int_flag != 0 {
                self.halt = false;
                if self.ime {
                    if debug::enabled() { println!("INT {:#04x}", int_addr); }
                    self.ime = false;
                    self.mmu.IF &= !int_flag;
                    self.call(int_addr);
                    return 5;
                }
                return 0;
            }
        }
        0
    }

    fn add8(&mut self, rid: R8, r2: u8, wc: bool) -> u8{
        let r1: u8 = self.r(rid);
        let c = if wc && self.reg.f.c { 1 } else { 0 };
        let res = r1.wrapping_add(r2).wrapping_add(c);
        self.reg.f.z = res == 0;
        self.reg.f.h = (r1 & 0x0F) + (r2 & 0x0F) + c > 0x0F;
        self.reg.f.n = false;
        self.reg.f.c = (r1 as u16) + (r2 as u16) + (c as u16) > 0xFF;
        res
    }

    fn add8_(&mut self, rid: R8, r2: u8, wc: bool) {
        let res = self.add8(rid, r2, wc);
        self.w(rid, res);
    }

    fn add16(&mut self, rid: R16, r2: u16) -> u16 {
        let r1 = self.r(rid);
        let res = r1.wrapping_add(r2);
        self.reg.f.h = (r1 & 0x07FF) + (r2 & 0x07FF) > 0x07FF;
        self.reg.f.n = false;
        self.reg.f.c = r1 > 0xFFFF - r2;
        res
    }

    fn add16_(&mut self, rid: R16, r2: u16) {
        let res = self.add16(rid, r2);
        self.w(rid, res);
    }

    fn add16i8(&mut self, rid: R16, r2: u8) -> u16 {
        let r1 = self.r(rid);
        let r2 = r2 as i8 as i16 as u16;
        let res = r1.wrapping_add(r2);
        self.reg.f.z = false;
        self.reg.f.h = (r1 & 0x000F) + (r2 & 0x000F) > 0x000F;
        self.reg.f.n = false;
        self.reg.f.c = (r1 & 0x00FF) + (r2 & 0x00FF) > 0x00FF;
        res
    }

    fn add16i8_(&mut self, rid: R16, r2: u8) {
        let res = self.add16i8(rid, r2);
        self.w(rid, res);
    }

    fn sub8(&mut self, rid: R8, r2: u8, wc: bool) -> u8{
        let r1 = self.r(rid);
        let c = if wc && self.reg.f.c { 1 } else { 0 };
        let res = r1.wrapping_sub(r2).wrapping_sub(c);
        self.reg.f.z = res == 0;
        self.reg.f.h = (r1 & 0x0F) < (r2 & 0x0F) + c;
        self.reg.f.n = true;
        self.reg.f.c = (r1 as u16) < (r2 as u16) + (c as u16);
        res
    }

    fn sub8_(&mut self, rid: R8, r2: u8, wc: bool) {
        let res = self.sub8(rid, r2, wc);
        self.w(rid, res);
    }

    fn inc8_(&mut self, rid: R8) {
        let r = self.r(rid);
        let res = r.wrapping_add(1);
        self.reg.f.z = res == 0;
        self.reg.f.n = false;
        self.reg.f.h = (r & 0x0F) + 1 > 0x0F;
        self.w(rid, res);
    }

    fn dec8_(&mut self, rid: R8) {
        let r = self.r(rid);
        let res = r.wrapping_sub(1);
        self.reg.f.z = res == 0;
        self.reg.f.n = true;
        self.reg.f.h = (r & 0x0F) < 1;
        self.w(rid, res);
    }

    fn inc16_(&mut self, rid: R16, sign: bool) {
        if sign {
            self.w(rid, self.r(rid).wrapping_add(1));
        } else {
            self.w(rid, self.r(rid).wrapping_sub(1));
        }
    }

    fn xor8_(&mut self, rid: R8, r2: u8) {
        let res = self.r(rid) ^ r2;
        self.reg.f.z = res == 0;
        self.reg.f.n = false;
        self.reg.f.h = false;
        self.reg.f.c = false;
        self.w(rid, res);
    }

    fn or8_(&mut self, rid: R8, r2: u8) {
        let res = self.r(rid) | r2;
        self.reg.f.z = res == 0;
        self.reg.f.n = false;
        self.reg.f.h = false;
        self.reg.f.c = false;
        self.w(rid, res);
    }

    fn and8_(&mut self, rid: R8, r2: u8) {
        let res = self.r(rid) & r2;
        self.reg.f.z = res == 0;
        self.reg.f.n = false;
        self.reg.f.h = true;
        self.reg.f.c = false;
        self.w(rid, res);
    }

    fn rot_(&mut self, rid: R8, left: bool, through_carry: bool, cb: bool) {
        let r = self.r(rid);
        let res: u8;
        if left {
            res = if through_carry {r << 1 | if self.reg.f.c {0x01} else {0}} else {r.rotate_left(1)};
            self.reg.f.c = r & 0x80 != 0; // Most significant bit
        } else {
            res = if through_carry {r >> 1 | if self.reg.f.c {0x80} else {0}} else {r.rotate_right(1)};
            self.reg.f.c = r & 0x01 != 0; // Least significant bit
        }
        self.reg.f.z = if rid == R8::A && !cb { false } else { res == 0 };
        self.reg.f.n = false;
        self.reg.f.h = false;
        self.w(rid, res);
    }
    
    fn shift_(&mut self, rid: R8, left: bool, arithmetic: bool) {
        let r = self.r(rid);
        let res = if left {
            self.reg.f.c = r & 0x80 != 0;
            r << 1
        } else {
            self.reg.f.c = r & 0x01 != 0;
            if arithmetic { r >> 1 | (r & 0x80) } else { r >> 1 }
        };
        self.reg.f.z = res == 0;
        self.reg.f.n = false;
        self.reg.f.h = false;
        self.w(rid, res);
    }

    fn swap_(&mut self, rid: R8) {
        let r = self.r(rid);
        self.reg.f.z = r == 0;
        self.reg.f.n = false;
        self.reg.f.h = false;
        self.reg.f.c = false;
        self.w(rid, (r >> 4) | (r << 4))
    }

    fn bit_(&mut self, bit: u8, rid: R8) {
        self.reg.f.z = self.r(rid) & (1 << bit) == 0;
        self.reg.f.n = false;
        self.reg.f.h = true;
    }

    fn res_(&mut self, bit: u8, rid: R8) {
        self.w(rid, self.r(rid) & !(1 << bit));
    }

    fn set_(&mut self, bit: u8, rid: R8) {
        self.w(rid, self.r(rid) | (1 << bit));
    }

    fn daa_(&mut self) {
        let mut a: u8 = self.reg.a;
        if self.reg.f.n {
            if self.reg.f.c { a = a.wrapping_sub(0x60); }
            if self.reg.f.h { a = a.wrapping_sub(0x06); }
        } else {
            if self.reg.f.c || a > 0x99 { a = a.wrapping_add(0x60); self.reg.f.c = true; }
            if self.reg.f.h || (a & 0x0F) > 0x09 { a = a.wrapping_add(0x06); }
        }
        self.reg.f.z = a == 0;
        self.reg.f.h = false;
        self.reg.a = a;
    }

    fn push(&mut self, val: u16) {
        self.reg.sp -= 2;
        self.mmu.ww(self.reg.sp, val);
    }

    fn pop(&mut self, rid: R16) {
        self.w(rid, self.mmu.rw(self.reg.sp));
        self.reg.sp += 2;
    }

    fn jr(&mut self, offset: u8) {
        self.jp(((self.reg.pc as i32) + (offset as i8 as i32)) as u16);
    }

    fn jp(&mut self, addr: u16) {
        self.reg.pc = addr;
    }

    fn call(&mut self, addr: u16) {
        self.push(self.reg.pc);
        self.jp(addr);
    }

}




