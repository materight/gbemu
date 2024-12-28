#[derive(Default, Clone, Copy)]
pub struct Joypad {
    // Buttons status, 1 = pressed
    pub a: bool,
    pub b: bool,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub start: bool,
    pub select: bool,
}

impl Joypad {
    pub fn get(&self, joyp: u8) -> u8 {
        let joyp = joyp & 0x20; // Zero-out other values
        joyp | if joyp & 0x20 == 0 {
            // Buttons
            (!self.start as u8) << 3 | (!self.select as u8) << 2 | (!self.b as u8) << 1 | (!self.a as u8)
        } else if joyp & 0x10 == 0 {
            // D-pad
            (!self.down as u8) << 3 | (!self.up as u8) << 2 | (!self.left as u8) << 1 | (!self.right as u8)
        } else {
            0x0F
        }
    }
}
