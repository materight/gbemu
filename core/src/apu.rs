
pub struct APU {
    nr1: [u8; 5],
    nr2: [u8; 5],
    nr3: [u8; 5],
    nr4: [u8; 5],
    nr5: [u8; 5],
    ram: [u8; 0x10]
}

impl APU {
    pub fn new() -> Self {
        Self {
            nr1: [0; 5],
            nr2: [0; 5],
            nr3: [0; 5],
            nr4: [0; 5],
            nr5: [0; 5],
            ram: [0; 0x10],
        }
    }

    pub fn r(&self, addr: u16) -> u8 {
        match addr {
            0xFF10..=0xFF14 => self.nr1[(addr - 0xFF10) as usize],
            0xFF15..=0xFF19 => self.nr2[(addr - 0xFF15) as usize],
            0xFF1A..=0xFF1E => self.nr3[(addr - 0xFF1A) as usize],
            0xFF1F..=0xFF23 => self.nr4[(addr - 0xFF1F) as usize],
            0xFF24..=0xFF28 => self.nr5[(addr - 0xFF24) as usize],
            0xFF30..=0xFF3F => self.ram[(addr - 0xFF30) as usize],
            _ => panic!("Address {:#06x} not part of APU", addr),
        }
    }

    pub fn w(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF10..=0xFF14 => self.nr1[(addr - 0xFF10) as usize] = val,
            0xFF15..=0xFF19 => self.nr2[(addr - 0xFF15) as usize] = val,
            0xFF1A..=0xFF1E => self.nr3[(addr - 0xFF1A) as usize] = val,
            0xFF1F..=0xFF23 => self.nr4[(addr - 0xFF1F) as usize] = val,
            0xFF24..=0xFF28 => self.nr5[(addr - 0xFF24) as usize] = val,
            0xFF30..=0xFF3F => self.ram[(addr - 0xFF30) as usize] = val,
            _ => panic!("Address {:#06x} not part of APU", addr),
        }
    }
}
