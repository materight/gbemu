pub trait Get<R, T> {
    fn r(&self, r: R) -> T;
}

pub trait Set<R, T> {
    fn w(&mut self, r: R, val: T);
}

macro_rules! byte_register {
    ($name:ident { $($field:ident),* }) => {

        #[derive(Debug, PartialEq, Copy, Clone)]
        pub struct $name {
            $(pub $field: bool),*
        }

        impl $name {
            pub fn w(&mut self, value: u8) {
                let mut bit = 8;
                $( bit -= 1; self.$field = value & (1 << bit) != 0; )*
            }
        }

        impl From<u8> for $name {
            fn from(value: u8) -> Self {
                let mut bit = 8;
                $name {
                    $( $field: { bit -= 1; value & (1 << bit) != 0 } ),*
                }
            }
        }

        impl From<&$name> for u8 {
            fn from(value: &$name) -> Self {
                let mut res = 0;
                let mut bit = 8;
                $( bit -= 1; if value.$field { res |= 1 << bit; } )*
                res
            }
        }

    };
}

pub fn pack_bits(bools: &[bool]) -> u8 {
    bools.iter().rev().enumerate().fold(0, |acc, (i, &b)| acc | ((b as u8) << i))
}

pub(crate) use byte_register;
