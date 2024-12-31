use crate::utils::pack_bits;

pub const AUDIO_FREQUENCY: u32 = 44_100;
const CPU_CLOCK: u32 = 4_194_304;
const SAMPLE_PERIOD: u16 = (CPU_CLOCK / AUDIO_FREQUENCY) as u16; // CPU clock / host audio buffer
const NOISE_DIVISORS: [u16; 8] = [8, 16, 32, 48, 64, 80, 96, 112];
const SQUARE_WAVES_DUTY: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

#[derive(Copy, Clone, Default)]
struct ChGlobal {
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
    fn r(&self, addr: u16) -> u8 {
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
            _ => panic!("Address {:#06x} not part of global channel", addr),
        }
    }

    fn w(&mut self, addr: u16, val: u8) {
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
            _ => panic!("Address {:#06x} not part of global channel", addr),
        }
    }

    fn update(&mut self, ch1: &ChPulse, ch2: &ChPulse, ch3: &ChWave, ch4: &ChNoise) {
        self.ch1_on = ch1.enabled && ch1.dac_enabled;
        self.ch2_on = ch2.enabled && ch2.dac_enabled;
        self.ch3_on = ch3.enabled && ch3.dac_enabled;
        self.ch4_on = ch4.enabled && ch4.dac_enabled;
    }

    fn mix(&self, ch1: f32, ch2: f32, ch3: f32, ch4: f32) -> (f32, f32) {
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
struct ChPulse {
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
    sweep_enabled: bool,
    sweep_timer: u8,
    length_timer: u8,
    envelope_timer: u8,
    frequency_timer: u16,
    frequency_shadow: u16,
    duty_wave_position: u8,
}
impl ChPulse {
    fn r(&self, addr: u16) -> u8 {
        match addr {
            0xFF10 => (self.sweep_period << 4) | (self.sweep_direction as u8) << 3 | self.sweep_shift,
            0xFF11 => (self.duty_wave << 6) | self.length_load,
            0xFF12 => (self.initial_volume << 4) | (self.envelope_direction as u8) << 3 | self.envelope_period,
            0xFF13 => self.frequency as u8,
            0xFF14 => (self.trigger as u8) << 7 | (self.length_enabled as u8) << 6 | ((self.frequency >> 8) as u8),
            _ => panic!("Address {:#06x} not part of pulse channel", addr),
        }
    }

    fn w(&mut self, addr: u16, val: u8) {
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

                    self.frequency_shadow = self.frequency;
                    self.sweep_timer = self.sweep_period;
                    if self.sweep_timer == 0 {
                        self.sweep_timer = 8;
                    }
                    self.sweep_enabled = self.sweep_period > 0 || self.sweep_shift > 0;
                    if self.sweep_shift > 0 {
                        self.compute_sweep();
                    }
                }
            }
            _ => panic!("Address {:#06x} not part of pulse channel", addr),
        }
    }

    fn compute_sweep(&mut self) -> u16 {
        let mut frequency_new = self.frequency_shadow >> self.sweep_shift;
        if self.sweep_direction {
            frequency_new = self.frequency_shadow - frequency_new;
        } else {
            frequency_new = self.frequency_shadow + frequency_new;
        }
        if frequency_new > 2047 {
            self.enabled = false;
        }
        frequency_new
    }

    fn step(&mut self, ticks: u32) -> f32 {
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
                if self.sweep_timer > 0 {
                    self.sweep_timer -= 1;
                }
                if self.sweep_timer == 0 {
                    self.sweep_timer = self.sweep_period;
                    if self.sweep_timer == 0 {
                        self.sweep_timer = 8;
                    }
                    if self.sweep_enabled && self.sweep_period > 0 {
                        let frequency_new = self.compute_sweep();
                        if frequency_new <= 2047 && self.sweep_shift > 0 {
                            self.frequency = frequency_new;
                            self.frequency_shadow = frequency_new;
                            self.compute_sweep();
                        }
                    }
                }
            }

            // Move to the next duty step
            if self.frequency_timer > 0 {
                self.frequency_timer -= 1;
            }
            if self.frequency_timer == 0 {
                self.frequency_timer = (2048 - self.frequency) * 4;
                self.duty_wave_position = (self.duty_wave_position + 1) % 8;
            }

            // Get sample from current duty wave position
            let sample = self.volume * SQUARE_WAVES_DUTY[self.duty_wave as usize][self.duty_wave_position as usize];

            // Normalize from [0, 15] to [-1.0, 1.0]
            (sample as f32 / 7.5) - 1.0
        } else {
            0.0
        }
    }
}

#[derive(Copy, Clone, Default)]
struct ChWave {
    // NR30
    dac_enabled: bool,
    // NR31
    length_load: u8,
    // NR32
    volume: u8,
    // NR33
    frequency: u16,
    // NR34
    trigger: bool,
    length_enabled: bool,
    // Internal
    enabled: bool,
    length_timer: u16,
    frequency_timer: u16,
    wave_ram: [u8; 0x10],
    wave_position: u8,
}
impl ChWave {
    fn r(&self, addr: u16) -> u8 {
        match addr {
            0xFF1A => (self.dac_enabled as u8) << 7,
            0xFF1B => self.length_load,
            0xFF1C => self.volume << 5,
            0xFF1D => self.frequency as u8,
            0xFF1E => (self.trigger as u8) << 7 | (self.length_enabled as u8) << 6 | ((self.frequency >> 8) as u8),
            0xFF30..=0xFF3F => self.wave_ram[(addr - 0xFF30) as usize],
            _ => panic!("Address {:#06x} not part of wave channel", addr),
        }
    }

    fn w(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF1A => {
                self.dac_enabled = val & 0b1000_0000 != 0;
            }
            0xFF1B => {
                self.length_load = val;
                self.length_timer = 256 - self.length_load as u16;
            }
            0xFF1C => {
                self.volume = (val & 0b0110_0000) >> 5;
            }
            0xFF1D => {
                self.frequency = (self.frequency & 0xFF00) | val as u16;
            }
            0xFF1E => {
                self.trigger = val & 0b1000_0000 != 0;
                self.length_enabled = val & 0b0100_0000 != 0;
                self.frequency = (self.frequency & 0x00FF) | (((val & 0b0000_0111) as u16) << 8);

                if self.trigger {
                    self.enabled = true;
                    if self.length_timer == 0 {
                        self.length_timer = 256;
                    }
                    self.frequency_timer = (2048 - self.frequency) * 2;
                    self.wave_position = 0;
                }
            }
            0xFF30..=0xFF3F => self.wave_ram[(addr - 0xFF30) as usize] = val,
            _ => panic!("Address {:#06x} not part of wave channel", addr),
        }
    }

    fn step(&mut self, ticks: u32) -> f32 {
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

            // Move to the next wave step
            if self.frequency_timer > 0 {
                self.frequency_timer -= 1;
            }
            if self.frequency_timer == 0 {
                self.frequency_timer = (2048 - self.frequency) * 2;
                self.wave_position = (self.wave_position + 1) % 32;
            }

            // Get sample from high or low nibble based on position
            let mut sample = self.wave_ram[self.wave_position as usize / 2];
            if self.wave_position % 2 == 0 {
                sample = sample & 0xF0 >> 4
            } else {
                sample = sample & 0x0F
            };

            // Shift by volume
            if self.volume == 0 {
                sample = 0;
            } else {
                sample = sample >> (self.volume - 1);
            }

            // Normalize sample from [0, 15] to [-1.0, 1.0]
            (sample as f32 / 7.5) - 1.0
        } else {
            0.0
        }
    }
}

#[derive(Copy, Clone, Default)]
struct ChNoise {
    // NR41
    length_load: u8,
    // NR42
    initial_volume: u8,
    envelope_direction: bool,
    envelope_period: u8,
    // NR43
    lfsr_shift: u8,
    lfsr_width: bool,
    lfsr_divisor_code: u8,
    // NR44
    trigger: bool,
    length_enabled: bool,
    // Internal
    enabled: bool,
    dac_enabled: bool,
    volume: u8,
    length_timer: u8,
    frequency_timer: u16,
    envelope_timer: u8,
    lfsr: u16,
}
impl ChNoise {
    fn r(&self, addr: u16) -> u8 {
        match addr {
            0xFF1F => 0,  // Unused
            0xFF20 => self.length_load,
            0xFF21 => (self.initial_volume << 4) | (self.envelope_direction as u8) << 3 | self.envelope_period,
            0xFF22 => (self.lfsr_shift << 4) | (self.lfsr_width as u8) << 3 | self.lfsr_divisor_code,
            0xFF23 => (self.trigger as u8) << 7 | (self.length_enabled as u8) << 6,
            _ => panic!("Address {:#06x} not part of noise channel", addr),
        }
    }

    fn w(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF1F => (),  // Unused
            0xFF20 => {
                self.length_load = val & 0b0011_1111;
                self.length_timer = 64 - self.length_load;
            }
            0xFF21 => {
                self.initial_volume = (val & 0b1111_0000) >> 4;
                self.envelope_direction = val & 0b0000_1000 != 0;
                self.envelope_period = val & 0b0000_0111;

                self.dac_enabled = self.initial_volume != 0 || self.envelope_direction;
                self.volume = self.initial_volume;
                self.envelope_timer = self.envelope_period;
            }
            0xFF22 => {
                self.lfsr_shift = (val & 0b1111_0000) >> 4;
                self.lfsr_width = val & 0b0000_1000 != 0;
                self.lfsr_divisor_code = val & 0b0000_0111;
            }
            0xFF23 => {
                self.trigger = val & 0b1000_0000 != 0;
                self.length_enabled = val & 0b0100_0000 != 0;

                if self.trigger {
                    self.enabled = true;
                    if self.length_timer == 0 {
                        self.length_timer = 64;
                    }
                    self.frequency_timer = NOISE_DIVISORS[self.lfsr_divisor_code as usize] << self.lfsr_shift;
                    self.envelope_timer = self.envelope_period;
                    self.volume = self.initial_volume;
                    self.lfsr = 0x7FFF;
                }
            }
            _ => panic!("Address {:#06x} not part of noise channel", addr),
        }
    }

    fn step(&mut self, ticks: u32) -> f32 {
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

            // Move to the next wave step
            if self.frequency_timer > 0 {
                self.frequency_timer -= 1;
            }
            if self.frequency_timer == 0 {
                self.frequency_timer = NOISE_DIVISORS[self.lfsr_divisor_code as usize] << self.lfsr_shift;

                // Xor the last two bits of the LFSR, shift LFSR and put the xor result in the first bit
                let xor_bit = (self.lfsr & 0x1) ^ ((self.lfsr >> 1) & 0x1);
                self.lfsr >>= 1;
                self.lfsr |= xor_bit << 15;
                if self.lfsr_width {
                    self.lfsr &= 0xFF7F;
                    self.lfsr |= xor_bit << 7;
                }
            }

            // Get sample from current LSFR bit
            let sample = self.volume * ((self.lfsr & 0x1) == 0) as u8;

            // Normalize sample from [0, 15] to [-1.0, 1.0]
            (sample as f32 / 7.5) - 1.0
        } else {
            0.0
        }
    }
}

#[derive(Clone)]
pub struct APU {
    ch_global: ChGlobal,
    ch1: ChPulse,
    ch2: ChPulse,
    ch3: ChWave,
    ch4: ChNoise,

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
            ch1: ChPulse::default(),
            ch2: ChPulse::default(),
            ch3: ChWave::default(),
            ch4: ChNoise::default(),
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
            0xFF1A..=0xFF1E => self.ch3.r(addr),
            0xFF1F..=0xFF23 => self.ch4.r(addr),
            0xFF24..=0xFF26 => self.ch_global.r(addr),
            0xFF27..=0xFF2F => 0, // Unused
            0xFF30..=0xFF3F => self.ch3.r(addr),
            _ => panic!("Address {:#06x} not part of APU", addr),
        }
    }

    pub fn w(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF10..=0xFF14 => self.ch1.w(addr, val),
            0xFF15..=0xFF19 => self.ch2.w(addr - 0x0005, val),
            0xFF1A..=0xFF1E => self.ch3.w(addr, val),
            0xFF1F..=0xFF23 => self.ch4.w(addr, val),
            0xFF24..=0xFF26 => self.ch_global.w(addr, val),
            0xFF27..=0xFF2F => (), // Unused
            0xFF30..=0xFF3F => self.ch3.w(addr, val),
            _ => panic!("Address {:#06x} not part of APU", addr),
        }
        self.ch_global.update(&self.ch1, &self.ch2, &self.ch3, &self.ch4);
    }

    pub fn step(&mut self, elapsed_ticks: u16) {
        // The APU produces 1 sample per CPU cycle at 4.19MHZ, but the host audio buffer only supports 44.1KHz, so we need to saubsample by avg
        for _ in 0..elapsed_ticks {
            self.ticks = self.ticks.wrapping_add(1);

            let ch1_sample = self.ch1.step(self.ticks);
            let ch2_sample = self.ch2.step(self.ticks);
            let ch3_sample = self.ch3.step(self.ticks);
            let ch4_sample = self.ch4.step(self.ticks);

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
