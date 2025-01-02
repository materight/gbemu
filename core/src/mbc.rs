use std::rc::Rc;

pub const DMG_BOOT_ROM: &[u8] = include_bytes!("./boot_dmg.bin");
pub const CGB_BOOT_ROM: &[u8] = include_bytes!("./boot_cgb.bin");

#[derive(Clone)]
pub struct MBC {
    rom: Rc<Vec<u8>>,
    ram: Vec<u8>,
    mbc_type: Box<dyn MBCType>,

    force_dmg: bool,
    pub boot_rom_unmounted: bool,
}

impl MBC {
    pub fn new(rom: &[u8], force_dmg: bool) -> Self {
        let mbc_type = rom[0x0147];
        let ram_size = match rom[0x0149] {
            0 | 1 => 0,
            2 => 8 * 1024,
            3 => 32 * 1024,
            4 => 128 * 1024,
            5 => 64 * 1024,
            v => panic!("RAM size {:#04x} not supported", v),
        };
        Self {
            rom: Rc::new(rom.to_vec()),
            ram: vec![0; ram_size],
            mbc_type: new_mbc(mbc_type),
            force_dmg: force_dmg,
            boot_rom_unmounted: false,
        }
    }

    pub fn r(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00FF if self.force_dmg && !self.boot_rom_unmounted => DMG_BOOT_ROM[addr as usize],
            0x0000..=0x00FF | 0x0200..=0x08FF if !self.force_dmg && !self.boot_rom_unmounted => CGB_BOOT_ROM[addr as usize],
            _ => self.mbc_type.r(addr, &self.rom, &self.ram),
        }
    }

    pub fn w(&mut self, addr: u16, val: u8) {
        self.mbc_type.w(addr, val, &self.rom, &mut self.ram)
    }

    pub fn title(&self) -> String {
        let title_size = if self.cgb_mode() { 11 } else { 16 };
        let mut title = String::with_capacity(title_size);
        for c in self.rom[0x0134..][..title_size].iter() {
            if *c == 0 {
                break;
            }
            title.push(*c as char);
        }
        title
    }

    pub fn cgb_mode(&self) -> bool {
        let mode = self.rom[0x143];
        !self.force_dmg && mode & 0x80 != 0
    }

    pub fn checksum(&self) -> u16 {
        u16::from_le_bytes([self.rom[0x014E], self.rom[0x014F]])
    }

    pub fn save(&self) -> &[u8] {
        &self.ram
    }

    pub fn load(&mut self, save: &[u8]) {
        let ram_size = self.ram.len();
        self.ram.copy_from_slice(&save[..std::cmp::min(ram_size, save.len())]);
    }
}

pub trait MBCType: MBCTypeClone {
    fn r(&self, addr: u16, rom: &[u8], ram: &[u8]) -> u8;
    fn w(&mut self, addr: u16, val: u8, rom: &[u8], ram: &mut [u8]);
}

pub trait MBCTypeClone {
    fn clone_box(&self) -> Box<dyn MBCType>;
}

impl<T> MBCTypeClone for T
where
    T: 'static + MBCType + Clone,
{
    fn clone_box(&self) -> Box<dyn MBCType> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn MBCType> {
    fn clone(&self) -> Box<dyn MBCType> {
        self.clone_box()
    }
}

fn bank_addr(addr: u16, bank_nr: u16, base: u16, size: u16) -> usize {
    (addr - base) as usize + bank_nr as usize * size as usize
}

fn mask_bank_nr(bank_nr: u16, size: usize) -> u16 {
    bank_nr & ((size - 1) >> 14) as u16
}

fn new_mbc(mbc_type: u8) -> Box<dyn MBCType> {
    match mbc_type {
        0x00 => Box::new(MBC0::default()),
        0x01..=0x03 => Box::new(MBC1::default()),
        0x0F..=0x13 => Box::new(MBC3::default()),
        0x19..=0x1E => Box::new(MBC5::default()),
        v => panic!("MBC type {:#04x} not supported", v),
    }
}

#[derive(Default, Clone, Copy)]
struct MBC0;
impl MBCType for MBC0 {
    fn r(&self, addr: u16, rom: &[u8], _: &[u8]) -> u8 {
        match addr {
            0x0000..=0x7FFF => rom[addr as usize],
            _ => 0x00,
        }
    }

    fn w(&mut self, _: u16, _: u8, _: &[u8], _: &mut [u8]) {}
}

#[derive(Default, Clone, Copy)]
struct MBC1 {
    rom_bank: u8,
    ram_bank: u8,
    ram_enabled: bool,
    mode: bool,
}
impl MBC1 {
    fn default() -> Self {
        Self {
            rom_bank: 1,
            ..Default::default()
        }
    }

    fn ram_addr(&self, addr: u16, ram: &[u8]) -> usize {
        if ram.len() <= 8 * 1024 {
            addr as usize % ram.len()
        } else if !self.mode {
            addr as usize - 0xA000
        } else {
            bank_addr(addr, self.ram_bank as u16, 0xA000, 0x2000)
        }
    }

    fn high_bank(&self, rom_size: usize, zero: bool) -> u8 {
        let mut bank_nr = if !zero {
            mask_bank_nr(self.rom_bank as u16, rom_size) as u8
        } else {
            0
        };
        if rom_size >= 64 * 16 * 1024 {
            if self.ram_bank & 0x01 != 0 {
                bank_nr |= 1 << 5
            } else {
                bank_nr &= !(1 << 5)
            }
        }
        if rom_size >= 128 * 16 * 1024 {
            if self.ram_bank & 0x02 != 0 {
                bank_nr |= 1 << 6
            } else {
                bank_nr &= !(1 << 6)
            }
        }
        bank_nr
    }
}
impl MBCType for MBC1 {
    fn r(&self, addr: u16, rom: &[u8], ram: &[u8]) -> u8 {
        match addr {
            0x0000..=0x3FFF => {
                if self.mode {
                    rom[bank_addr(addr, self.high_bank(rom.len(), true) as u16, 0, 0x4000)]
                } else {
                    rom[addr as usize]
                }
            }
            0x4000..=0x7FFF => rom[bank_addr(addr, self.high_bank(rom.len(), false) as u16, 0x4000, 0x4000)],
            0xA000..=0xBFFF => {
                if self.ram_enabled {
                    ram[self.ram_addr(addr, &ram)]
                } else {
                    0xFF
                }
            }
            _ => 0x00,
        }
    }

    fn w(&mut self, addr: u16, val: u8, rom: &[u8], ram: &mut [u8]) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = val & 0x0F == 0x0A,
            0x2000..=0x3FFF => {
                self.rom_bank = if val & 0x1F != 0 {
                    mask_bank_nr(val as u16, rom.len()) as u8
                } else {
                    1
                }
            }
            0x4000..=0x5FFF => self.ram_bank = val & 0x03,
            0x6000..=0x7FFF => self.mode = val & 0x01 != 0,
            0xA000..=0xBFFF => {
                if self.ram_enabled {
                    ram[self.ram_addr(addr, &ram)] = val
                }
            }
            _ => (),
        }
    }
}

#[derive(Default, Clone, Copy)]
struct MBC3 {
    rom_bank: u8,
    ram_bank: u8,
    ram_enabled: bool,
    rtc_mapped: bool,
}
impl MBC3 {
    fn default() -> Self {
        Self {
            rom_bank: 1,
            ..Default::default()
        }
    }
}
impl MBCType for MBC3 {
    fn r(&self, addr: u16, rom: &[u8], ram: &[u8]) -> u8 {
        match addr {
            0x0000..=0x3FFF => rom[addr as usize],
            0x4000..=0x7FFF => rom[bank_addr(addr, self.rom_bank as u16, 0x4000, 0x4000)],
            0xA000..=0xBFFF => {
                if !self.ram_enabled {
                    0xFF
                } else if self.rtc_mapped {
                    0x00
                } else {
                    ram[bank_addr(addr, self.ram_bank as u16, 0xA000, 0x2000)]
                }
            }
            _ => 0x00,
        }
    }

    fn w(&mut self, addr: u16, val: u8, _: &[u8], ram: &mut [u8]) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = val & 0x0F == 0x0A,
            0x2000..=0x3FFF => self.rom_bank = if val & 0x7F != 0 { val & 0x7F } else { 1 },
            0x4000..=0x5FFF => match val & 0x0F {
                0x00..=0x03 => {
                    self.rtc_mapped = false;
                    self.ram_bank = val & 0x03
                }
                0x08..=0x0C => self.rtc_mapped = true,
                _ => (),
            },
            0xA000..=0xBFFF => {
                if self.ram_enabled {
                    ram[bank_addr(addr, self.ram_bank as u16, 0xA000, 0x2000)] = val
                }
            }
            _ => (),
        }
    }
}

#[derive(Default, Clone, Copy)]
struct MBC5 {
    rom_bank: u16,
    ram_bank: u8,
    ram_enabled: bool,
}
impl MBC5 {
    fn default() -> Self {
        Self {
            rom_bank: 1,
            ..Default::default()
        }
    }
}
impl MBCType for MBC5 {
    fn r(&self, addr: u16, rom: &[u8], ram: &[u8]) -> u8 {
        match addr {
            0x0000..=0x3FFF => rom[addr as usize],
            0x4000..=0x7FFF => rom[bank_addr(addr, self.rom_bank, 0x4000, 0x4000)],
            0xA000..=0xBFFF => {
                if self.ram_enabled {
                    ram[bank_addr(addr, self.ram_bank as u16, 0xA000, 0x2000)]
                } else {
                    0xFF
                }
            }
            _ => 0x00,
        }
    }

    fn w(&mut self, addr: u16, val: u8, rom: &[u8], ram: &mut [u8]) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = val & 0x0F == 0x0A,
            0x2000..=0x2FFF => self.rom_bank = mask_bank_nr((self.rom_bank & 0xFF00) | (((val as u16) << 0) & 0x00FF), rom.len()),
            0x3000..=0x3FFF => self.rom_bank = mask_bank_nr((self.rom_bank & 0x00FF) | (((val as u16) << 8) & 0x0100), rom.len()),
            0x4000..=0x5FFF => self.ram_bank = val & 0x0F,
            0xA000..=0xBFFF => {
                if self.ram_enabled {
                    ram[bank_addr(addr, self.ram_bank as u16, 0xA000, 0x2000)] = val
                }
            }
            _ => (),
        }
    }
}
