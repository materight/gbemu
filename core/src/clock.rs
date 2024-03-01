use crate::cpu::INT_TIMER;

pub struct Clock {
    sysclock: u16,
    prev_edge_bit: bool, // Used to correctly compute a "falling edge" in the clock

    tima: u8,
    tma: u8,
    tac: u8,
}
impl Clock {
    pub fn new() -> Self { Self {sysclock: 0, prev_edge_bit: false, tima: 0, tma: 0, tac: 0} }

    pub fn div(&self) -> u8 {
        (self.sysclock >> 8) as u8
    }

    pub fn r(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => self.div(),
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac,
            _ => panic!("Address {:#06x} not part of clock", addr),
        }
    }

    pub fn w(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF04 => self.sysclock = 0,
            0xFF05 => self.tima = val,
            0xFF06 => self.tma = val,
            0xFF07 => self.tac = val,
            _ => panic!("Address {:#06x} not part of clock", addr),
        }
    }

    pub fn step(&mut self, elapsed_ticks: u8) -> u8 {
        let mut interrupts = 0;
        let tima_enabled = self.tac & 0x04 != 0;
        let tima_bit = match self.tac & 0x03 {
            0x00 => 1024 / 2, 0x01 => 16 / 2, 0x02 => 64 / 2, 0x03 => 256 / 2,
            v => panic!("TAC {:#04x} unsupported", v),
        };
        for _ in 0..elapsed_ticks {
            self.sysclock = self.sysclock.wrapping_add(1);
            let current_edge_bit = tima_enabled && (self.sysclock & tima_bit != 0);
            if self.prev_edge_bit && !current_edge_bit {
                if self.tima < 0xFF {
                    self.tima = self.tima.wrapping_add(1);
                } else {
                    self.tima = self.tma;
                    interrupts |= INT_TIMER.0;
                }
            }
            self.prev_edge_bit = current_edge_bit
        }
        interrupts
    }
}