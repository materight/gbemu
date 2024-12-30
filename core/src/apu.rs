use crate::utils::pack_bits;

pub const AUDIO_FREQUENCY: u32 = 44_100;
const CPU_CLOCK: u32 = 4_194_304;
const SAMPLE_PERIOD: u16 = (CPU_CLOCK / AUDIO_FREQUENCY) as u16; // CPU clock / host audio buffer
const SQUARE_WAVES_DUTY: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

#[derive(Copy, Clone, Default)]
pub struct ChGlobal {
    // NR50
    volume_left: u8,
    volume_right: u8,
    // NR51
    ch1_left: bool,
    ch2_left: bool,
    ch3_left: bool,
    ch4_left: bool,
    ch1_right: bool,
    ch2_right: bool,
    ch3_right: bool,
    ch4_right: bool,
    // NR52
    audio_on: bool,
    ch1_on: bool,
    ch2_on: bool,
    ch3_on: bool,
    ch4_on: bool,
}
impl ChGlobal {
    pub fn r(&self, addr: u16) -> u8 {
        match addr {
            0xFF24 => (self.volume_left << 4) | self.volume_right,
            0xFF25 => pack_bits(&[
                self.ch4_left,
                self.ch3_left,
                self.ch2_left,
                self.ch1_left,
                self.ch4_right,
                self.ch3_right,
                self.ch2_right,
                self.ch1_right,
            ]),
            0xFF26 => (self.audio_on as u8) << 7 | pack_bits(&[self.ch4_on, self.ch3_on, self.ch2_on, self.ch1_on]),
            _ => 0,
        }
    }

    pub fn w(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF24 => {
                self.volume_left = (val & 0b0111_0000) >> 4;
                self.volume_right = val & 0b0000_0111;
            }
            0xFF25 => {
                self.ch4_left = val & 0b1000_0000 != 0;
                self.ch3_left = val & 0b0100_0000 != 0;
                self.ch2_left = val & 0b0010_0000 != 0;
                self.ch1_left = val & 0b0001_0000 != 0;
                self.ch4_right = val & 0b0000_1000 != 0;
                self.ch3_right = val & 0b0000_0100 != 0;
                self.ch2_right = val & 0b0000_0010 != 0;
                self.ch1_right = val & 0b0000_0001 != 0;
            }
            0xFF26 => {
                self.audio_on = val & 0b1000_0000 != 0;
            }
            _ => (),
        }
    }

    pub fn update(&mut self, ch1: &ChSquare, ch2: &ChSquare) {
        self.ch1_on = ch1.enabled;
        self.ch2_on = ch2.enabled;
    }

    pub fn mix(&self, ch1: f32, ch2: f32, ch3: f32, ch4: f32) -> (f32, f32) {
        let mut sample_left: f32 = 0.0;
        sample_left += if self.ch1_left { ch1 } else { 0.0 };
        sample_left += if self.ch2_left { ch2 } else { 0.0 };
        sample_left += if self.ch3_left { ch3 } else { 0.0 };
        sample_left += if self.ch4_left { ch4 } else { 0.0 };
        sample_left *= (self.volume_left as f32 + 1.0) / 8.0;
        sample_left /= 4.0; // Normalize to [-1.0, 1.0]

        let mut sample_right: f32 = 0.0;
        sample_right += if self.ch1_right { ch1 } else { 0.0 };
        sample_right += if self.ch2_right { ch2 } else { 0.0 };
        sample_right += if self.ch3_right { ch3 } else { 0.0 };
        sample_right += if self.ch4_right { ch4 } else { 0.0 };
        sample_right *= (self.volume_right as f32 + 1.0) / 8.0;
        sample_right /= 4.0;

        (sample_left, sample_right)
    }
}

#[derive(Copy, Clone, Default)]
pub struct ChSquare {
    // NR10
    sweep_period: u8,
    sweep_direction: bool,
    sweep_shift: u8,
    // NR11
    duty_wave: u8,
    length_load: u8,
    // NR12
    initial_volume: u8,
    envelope_direction: bool,
    envelope_period: u8,
    // NR13
    frequency: u16,
    // NR14
    trigger: bool,
    length_enabled: bool,
    // Internal
    enabled: bool,
    dac_enabled: bool,
    volume: u8,
    swwep_enabled: bool,
    sweep_counter: u8,
    length_timer: u8,
    envelope_timer: u8,
    frequency_timer: u16,
    duty_step: u8,
}
impl ChSquare {
    pub fn r(&self, addr: u16) -> u8 {
        match addr {
            0xFF10 => (self.sweep_period << 4) | (self.sweep_direction as u8) << 3 | self.sweep_shift,
            0xFF11 => (self.duty_wave << 6) | self.length_load,
            0xFF12 => (self.initial_volume << 4) | (self.envelope_direction as u8) << 3 | self.envelope_period,
            0xFF13 => self.frequency as u8,
            0xFF14 => (self.trigger as u8) << 7 | (self.length_enabled as u8) << 6 | ((self.frequency >> 8) as u8),
            _ => 0,
        }
    }

    pub fn w(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF10 => {
                self.sweep_period = (val & 0b0111_0000) >> 4;
                self.sweep_direction = val & 0b0000_1000 != 0;
                self.sweep_shift = val & 0b0000_0111;
            }
            0xFF11 => {
                self.duty_wave = (val & 0b1100_0000) >> 6;
                self.length_load = val & 0b0011_1111;
                self.length_timer = 64 - self.length_load;
            }
            0xFF12 => {
                self.initial_volume = (val & 0b1111_0000) >> 4;
                self.envelope_direction = val & 0b0000_1000 != 0;
                self.envelope_period = val & 0b0000_0111;

                self.dac_enabled = self.initial_volume != 0 || self.envelope_direction;
                self.volume = self.initial_volume;
                self.envelope_timer = self.envelope_period;
            }
            0xFF13 => {
                self.frequency = (self.frequency & 0xFF00) | (val as u16);
            }
            0xFF14 => {
                self.trigger = val & 0b1000_0000 != 0;
                self.length_enabled = val & 0b0100_0000 != 0;
                self.frequency = (self.frequency & 0x00FF) | (((val & 0b0000_0111) as u16) << 8);

                if self.trigger {
                    self.enabled = true;
                    if self.length_timer == 0 {
                        self.length_timer = 64;
                    }
                    self.frequency_timer = (2048 - self.frequency) * 4;
                    self.envelope_timer = self.envelope_period;
                    self.volume = self.initial_volume;
                }
            }
            _ => (),
        }
    }

    pub fn step(&mut self, ticks: u32) -> f32 {
        if self.enabled && self.dac_enabled {
            // Clock length timer at 256Hz
            if ticks % (CPU_CLOCK / 265) == 0 {
                if self.length_enabled && self.length_timer > 0 {
                    self.length_timer -= 1;
                    if self.length_timer == 0 {
                        self.enabled = false;
                    }
                }
            }

            // Clock envelope timer at 64Hz
            if ticks % (CPU_CLOCK / 64) == (CPU_CLOCK / 512) * 7 {
                if self.envelope_period > 0 {
                    if self.envelope_timer > 0 {
                        self.envelope_timer -= 1;
                    }
                    if self.envelope_timer == 0 {
                        self.envelope_timer = self.envelope_period;
                        if self.envelope_timer == 0 {
                            self.envelope_timer = 8;
                        }
                        if self.envelope_direction && self.volume < 15 {
                            self.volume += 1;
                        } else if !self.envelope_direction && self.volume > 0 {
                            self.volume -= 1;
                        }
                    }
                }
            }

            // Clock sweep timer at 128Hz
            if ticks % (CPU_CLOCK / 128) == (CPU_CLOCK / 512) * 2 {
                
            }

            // Move to the next duty step
            if self.frequency_timer > 0 {
                self.frequency_timer -= 1;
            }
            if self.frequency_timer == 0 {
                self.frequency_timer = (2048 - self.frequency) * 4;
                self.duty_step = (self.duty_step + 1) % 8;
            }

            // Get sample and normalize from [0, 15] to [-1.0, 1.0]
            let sample = self.volume * SQUARE_WAVES_DUTY[self.duty_wave as usize][self.duty_step as usize];
            (sample as f32 / 7.5) - 1.0
        } else {
            0.0
        }
    }
}

#[derive(Clone)]
pub struct APU {
    ch_global: ChGlobal,
    ch1: ChSquare,
    ch2: ChSquare,
    nr3: [u8; 5],
    nr4: [u8; 5],
    nr5: [u8; 5],
    ram: [u8; 0x10],

    ticks: u32,

    sample_left_sum: f32,
    sample_right_sum: f32,
    sample_count: u16,

    pub buffer: Vec<f32>,
}

impl APU {
    pub fn new() -> Self {
        Self {
            ch_global: ChGlobal::default(),
            ch1: ChSquare::default(),
            ch2: ChSquare::default(),
            nr3: [0; 5],
            nr4: [0; 5],
            nr5: [0; 5],
            ram: [0; 0x10],
            ticks: 0,
            sample_left_sum: 0.0,
            sample_right_sum: 0.0,
            sample_count: 0,
            buffer: Vec::with_capacity(AUDIO_FREQUENCY as usize * 2),
        }
    }

    pub fn r(&self, addr: u16) -> u8 {
        match addr {
            0xFF10..=0xFF14 => self.ch1.r(addr),
            0xFF15..=0xFF19 => self.ch2.r(addr - 0x0005),
            0xFF1A..=0xFF1E => self.nr3[(addr - 0xFF1A) as usize],
            0xFF1F..=0xFF23 => self.nr4[(addr - 0xFF1F) as usize],
            0xFF24..=0xFF26 => self.ch_global.r(addr),
            0xFF30..=0xFF3F => self.ram[(addr - 0xFF30) as usize],
            _ => panic!("Address {:#06x} not part of APU", addr),
        }
    }

    pub fn w(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF10..=0xFF14 => self.ch1.w(addr, val),
            0xFF15..=0xFF19 => self.ch2.w(addr - 0x0005, val),
            0xFF1A..=0xFF1E => self.nr3[(addr - 0xFF1A) as usize] = val,
            0xFF1F..=0xFF23 => self.nr4[(addr - 0xFF1F) as usize] = val,
            0xFF24..=0xFF26 => self.ch_global.w(addr, val),
            0xFF30..=0xFF3F => self.ram[(addr - 0xFF30) as usize] = val,
            _ => panic!("Address {:#06x} not part of APU", addr),
        }
        self.ch_global.update(&self.ch1, &self.ch2);
    }

    pub fn step(&mut self, elapsed_ticks: u16) {

        // The APU produces 1 sample per CPU cycle at 4.19MHZ, but the host audio buffer only supports 44.1KHz, so we need to saubsample by avg
        for _ in 0..elapsed_ticks {
            self.ticks = self.ticks.wrapping_add(1);

            let ch1_sample = self.ch1.step(self.ticks);
            let ch2_sample = self.ch2.step(self.ticks);
            let ch3_sample = 0.0;
            let ch4_sample = 0.0;

            let (sample_left, sample_right) = self.ch_global.mix(ch1_sample, ch2_sample, ch3_sample, ch4_sample);

            self.sample_left_sum += sample_left;
            self.sample_right_sum += sample_right;
            self.sample_count += 1;
            if self.sample_count >= SAMPLE_PERIOD as u16 {
                self.buffer.push(self.sample_left_sum / self.sample_count as f32);
                self.buffer.push(self.sample_right_sum / self.sample_count as f32);
                self.sample_left_sum = 0.0;
                self.sample_right_sum = 0.0;
                self.sample_count = 0;
            }
        }
    }
}
