use crate::cpu::CPU;
use crate::utils::{byte_register, Get, Set};
use std::convert::{From, Into};

// Internal registers representations
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum R8 {
    B,
    C,
    D,
    E,
    H,
    L,
    HL,
    A,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum R16 {
    BC,
    DE,
    HL,
    AF,
    SP,
    PC,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CC {
    NZ,
    Z,
    NC,
    C,
}

#[derive(Copy, Clone)]
pub struct Registers {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub f: FlagsRegister,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

impl Registers {
    pub fn new() -> Self {
        // Set status after boot sequence as default (except for PC)
        Self {
            a: 0x01,
            b: 0xFF,
            c: 0x13,
            d: 0x00,
            e: 0xC1,
            f: FlagsRegister::from(0),
            h: 0x84,
            l: 0x03,
            sp: 0xFFFE,
            pc: 0x0000,
        }
    }
}

byte_register!(FlagsRegister {
    z, // Zero
    n, // Substraction
    h, // Half carry
    c  // Carry
});

impl Get<R8, u8> for CPU {
    fn r(&self, r: R8) -> u8 {
        match r {
            R8::B => self.reg.b,
            R8::C => self.reg.c,
            R8::D => self.reg.d,
            R8::E => self.reg.e,
            R8::H => self.reg.h,
            R8::HL => self.mmu.r(u16::from_be_bytes([self.reg.h, self.reg.l])),
            R8::L => self.reg.l,
            R8::A => self.reg.a,
        }
    }
}

impl Set<R8, u8> for CPU {
    fn w(&mut self, r: R8, val: u8) {
        match r {
            R8::B => self.reg.b = val,
            R8::C => self.reg.c = val,
            R8::D => self.reg.d = val,
            R8::E => self.reg.e = val,
            R8::H => self.reg.h = val,
            R8::HL => self.mmu.w(u16::from_be_bytes([self.reg.h, self.reg.l]), val),
            R8::L => self.reg.l = val,
            R8::A => self.reg.a = val,
        }
    }
}

impl Get<R16, u16> for CPU {
    fn r(&self, r: R16) -> u16 {
        match r {
            R16::BC => u16::from_be_bytes([self.reg.b, self.reg.c]),
            R16::DE => u16::from_be_bytes([self.reg.d, self.reg.e]),
            R16::HL => u16::from_be_bytes([self.reg.h, self.reg.l]),
            R16::AF => u16::from_be_bytes([self.reg.a, u8::from(&self.reg.f)]),
            R16::SP => self.reg.sp,
            R16::PC => self.reg.pc,
        }
    }
}

impl Set<R16, u16> for CPU {
    fn w(&mut self, r: R16, val: u16) {
        match r {
            R16::BC => [self.reg.b, self.reg.c] = val.to_be_bytes(),
            R16::DE => [self.reg.d, self.reg.e] = val.to_be_bytes(),
            R16::HL => [self.reg.h, self.reg.l] = val.to_be_bytes(),
            R16::AF => {
                let [a, f] = val.to_be_bytes();
                self.reg.a = a;
                self.reg.f = f.into();
            }
            R16::SP => self.reg.sp = val,
            R16::PC => self.reg.pc = val,
        }
    }
}

impl Get<CC, bool> for CPU {
    fn r(&self, cc: CC) -> bool {
        match cc {
            CC::NZ => !self.reg.f.z,
            CC::Z => self.reg.f.z,
            CC::NC => !self.reg.f.c,
            CC::C => self.reg.f.c,
        }
    }
}
