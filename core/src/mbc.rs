use std::cmp::min;

pub const BOOT_ROM: [u8; 256] = [
    0x31, 0xFE, 0xFF, 0xAF, 0x21, 0xFF, 0x9F, 0x32, 0xCB, 0x7C, 0x20, 0xFB, 0x21, 0x26, 0xFF, 0x0E, 0x11, 0x3E, 0x80, 0x32, 0xE2, 0x0C, 0x3E, 0xF3, 0xE2, 0x32, 0x3E, 0x77, 0x77, 0x3E, 0xFC, 0xE0, 
    0x47, 0x11, 0x04, 0x01, 0x21, 0x10, 0x80, 0x1A, 0xCD, 0x95, 0x00, 0xCD, 0x96, 0x00, 0x13, 0x7B, 0xFE, 0x34, 0x20, 0xF3, 0x11, 0xD8, 0x00, 0x06, 0x08, 0x1A, 0x13, 0x22, 0x23, 0x05, 0x20, 0xF9, 
    0x3E, 0x19, 0xEA, 0x10, 0x99, 0x21, 0x2F, 0x99, 0x0E, 0x0C, 0x3D, 0x28, 0x08, 0x32, 0x0D, 0x20, 0xF9, 0x2E, 0x0F, 0x18, 0xF3, 0x67, 0x3E, 0x64, 0x57, 0xE0, 0x42, 0x3E, 0x91, 0xE0, 0x40, 0x04, 
    0x1E, 0x02, 0x0E, 0x0C, 0xF0, 0x44, 0xFE, 0x90, 0x20, 0xFA, 0x0D, 0x20, 0xF7, 0x1D, 0x20, 0xF2, 0x0E, 0x13, 0x24, 0x7C, 0x1E, 0x83, 0xFE, 0x62, 0x28, 0x06, 0x1E, 0xC1, 0xFE, 0x64, 0x20, 0x06, 
    0x7B, 0xE2, 0x0C, 0x3E, 0x87, 0xE2, 0xF0, 0x42, 0x90, 0xE0, 0x42, 0x15, 0x20, 0xD2, 0x05, 0x20, 0x4F, 0x16, 0x20, 0x18, 0xCB, 0x4F, 0x06, 0x04, 0xC5, 0xCB, 0x11, 0x17, 0xC1, 0xCB, 0x11, 0x17, 
    0x05, 0x20, 0xF5, 0x22, 0x23, 0x22, 0x23, 0xC9, 0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D, 0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 
    0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99, 0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E, 0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C, 
    0x21, 0x04, 0x01, 0x11, 0xA8, 0x00, 0x1A, 0x13, 0xBE, 0x20, 0xFE, 0x23, 0x7D, 0xFE, 0x34, 0x20, 0xF5, 0x06, 0x19, 0x78, 0x86, 0x23, 0x05, 0x20, 0xFB, 0x86, 0x20, 0xFE, 0x3E, 0x01, 0xE0, 0x50,
];



pub struct MBC {
    rom: Vec<u8>,
    ram: Vec<u8>,
    mbc_type: Box<dyn MBCType>,
    pub boot_rom_unmounted: bool,
}

impl MBC {
    pub fn new(rom: &[u8]) -> Self {
        let mbc_type = rom[0x0147];
        let ram_size = match rom[0x0149] {
            0 | 1 => 0, 2 => 8 * 1024, 3 => 32 * 1024, 4 => 128 * 1024, 5 => 64 * 1024,
            v => panic!("RAM size {:#06x} not supported", v),
        };
        Self {
            rom: rom.to_vec(),
            ram: vec![0; ram_size],
            mbc_type: new_mbc(mbc_type),
            boot_rom_unmounted: false,
        }
    }

    pub fn r(&self, addr: u16) -> u8 {
        if !self.boot_rom_unmounted && addr <= 0x00FF {
            BOOT_ROM[addr as usize]
        } else {
            self.mbc_type.r(addr, &self.rom, &self.ram)
        }
    }

    pub fn w(&mut self, addr: u16, val: u8) {
        self.mbc_type.w(addr, val, &self.rom, &mut self.ram)
    }

    pub fn title(&self) -> String {
        let mut title = String::with_capacity(11);
        for c in self.rom[0x0134..=0x0143].iter() {
            if *c == 0 { break }
            title.push(*c as char);
        }
        title
    }

    pub fn checksum(&self) -> u16 {
        u16::from_le_bytes([self.rom[0x014E], self.rom[0x014F]])
    }

    pub fn save(&self) -> &[u8] {
        &self.ram
    }

    pub fn load(&mut self, save: &[u8]) {
        let ram_size = self.ram.len();
        self.ram.copy_from_slice(&save[..min(ram_size, save.len())]);
    }
}




pub trait MBCType {
    fn r(&self, addr: u16, rom: &[u8], ram: &[u8]) -> u8;
    fn w(&mut self, addr: u16, val: u8, rom: &[u8], ram: &mut [u8]);
}

fn bank_addr(addr: u16, bank_nr: u8, base: u16, size: u16) -> usize {
    (addr - base) as usize + bank_nr as usize * size as usize
}

fn new_mbc(mbc_type: u8) -> Box<dyn MBCType> {
    match mbc_type {
        0x00 =>        Box::new(MBC0::default()),
        0x01..=0x03 => Box::new(MBC1::default()),
        0x0F..=0x13 => Box::new(MBC3::default()),
        v => panic!("MBC type {:#06x} not supported", v)
    }
}



#[derive(Default)]
struct MBC0;
impl MBCType for MBC0 {
    fn r(&self, addr: u16, rom: &[u8], _: &[u8]) -> u8 { 
        match addr {
            0x0000..=0x7FFF => rom[addr as usize],
            _ => 0x00,
        }
    }

    fn w(&mut self, _: u16, _: u8, _: &[u8], _: &mut [u8]) { }
}



#[derive(Default)]
struct MBC1 {
    rom_bank: u8,
    ram_bank: u8,
    ram_enabled: bool,
    mode: bool,
}
impl MBC1 {
    fn default() -> Self { Self { rom_bank: 1, ..Default::default() } }

    fn ram_addr(&self, addr:u16, ram: &[u8]) -> usize {
        if ram.len() <= 8 * 1024 { addr as usize % ram.len() }
        else if !self.mode { addr as usize - 0xA000 }
        else { bank_addr(addr, self.ram_bank, 0xA000, 0x2000) }
    }

    fn high_bank(&self, rom_size: usize, zero: bool) -> u8 {
        let mut bank_nr = if !zero { self.rom_bank & (rom_size - 1 >> 14) as u8 } else { 0 };
        if rom_size >= 64 * 16 * 1024  { if self.ram_bank & 0x01 != 0 { bank_nr |= 1 << 5 } else { bank_nr &= !(1 << 5)} }
        if rom_size >= 128 * 16 * 1024 { if self.ram_bank & 0x02 != 0 { bank_nr |= 1 << 6 } else { bank_nr &= !(1 << 6)} }
        bank_nr
    }
}
impl MBCType for MBC1 {

    fn r(&self, addr: u16, rom: &[u8], ram: &[u8]) -> u8 { 
        match addr {
            0x0000..=0x3FFF => if self.mode { rom[bank_addr(addr, self.high_bank(rom.len(), true), 0, 0x4000)] } else { rom[addr as usize] },
            0x4000..=0x7FFF => rom[bank_addr(addr, self.high_bank(rom.len(), false), 0x4000, 0x4000)],
            0xA000..=0xBFFF => if self.ram_enabled { ram[self.ram_addr(addr, &ram)] } else { 0xFF },
            _ => 0x00,
        }
    }

    fn w(&mut self, addr: u16, val: u8, rom: &[u8], ram: &mut [u8]) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = val & 0x0F == 0x0A,
            0x2000..=0x3FFF => self.rom_bank = if val & 0x1F != 0 { val & (rom.len() - 1 >> 14) as u8 } else { 1 },
            0x4000..=0x5FFF => self.ram_bank = val & 0x03,
            0x6000..=0x7FFF => self.mode = val & 0x01 != 0,
            0xA000..=0xBFFF => if self.ram_enabled { ram[self.ram_addr(addr, &ram)] = val },
            _ => ()
        }
    }
}



#[derive(Default)]
struct MBC3 {
    rom_bank: u8,
    ram_bank: u8,
    ram_enabled: bool,
    rtc_mapped: bool,
}
impl MBC3 {
    fn default() -> Self { Self { rom_bank: 1, ..Default::default() } }
}
impl MBCType for MBC3 {

    fn r(&self, addr: u16, rom: &[u8], ram: &[u8]) -> u8 { 
        match addr {
            0x0000..=0x3FFF => rom[addr as usize],
            0x4000..=0x7FFF => rom[bank_addr(addr, self.rom_bank, 0x4000, 0x4000)],
            0xA000..=0xBFFF => 
                if !self.ram_enabled    { 0xFF }
                else if self.rtc_mapped { 0x00 }
                else                    { ram[bank_addr(addr, self.ram_bank, 0xA000, 0x2000)] },
            _ => 0x00,
        }
    }

    fn w(&mut self, addr: u16, val: u8, _: &[u8], ram: &mut [u8]) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = val & 0x0F == 0x0A,
            0x2000..=0x3FFF => self.rom_bank = if val & 0x7F != 0 { val & 0x7F } else { 1 },
            0x4000..=0x5FFF => match val & 0x0F {
                0x00..=0x03 => { self.rtc_mapped = false; self.ram_bank = val & 0x03 },
                0x08..=0x0C => { self.rtc_mapped = true },
                _ => (),
            }
            0xA000..=0xBFFF => if self.ram_enabled { ram[bank_addr(addr, self.ram_bank, 0xA000, 0x2000)] = val },
            _ => ()
        }
    }
}
