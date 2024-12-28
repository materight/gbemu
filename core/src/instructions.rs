use crate::registers::{CC, R16, R8};

/*
 OP codes source: https://gbdev.io/pandocs/CPU_Instruction_Set.html
*/

// Define mapping of register ID to address (index of register int the array)
pub const ADDR_R8: [R8; 8] = [R8::B, R8::C, R8::D, R8::E, R8::H, R8::L, R8::HL, R8::A];
pub const ADDR_R16: [R16; 4] = [R16::BC, R16::DE, R16::HL, R16::SP];
pub const ADDR_R16_STK: [R16; 4] = [R16::BC, R16::DE, R16::HL, R16::AF];
pub const ADDR_R16_MEM: [R16; 2] = [R16::BC, R16::DE];
pub const ADDR_CC: [CC; 4] = [CC::NZ, CC::Z, CC::NC, CC::C];
pub const ADDR_3: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7]; // General 3bit address

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    INVALID,

    // Block 0
    NOP,

    LD_R16_I16(R16),
    LD_R16_A(R16),
    LD_HLID_A(bool), // HL with INC/DEC
    LD_A_R16(R16),
    LD_A_HLID(bool),
    LD_I16_SP,

    INC_R16(R16),
    DEC_R16(R16),
    ADD_HL_R16(R16),

    INC_R8(R8),
    DEC_R8(R8),
    LD_R8_I8(R8),

    RLCA,
    RRCA,
    RLA,
    RRA,
    DAA,
    CPL,
    SCF,
    CCF,

    JR_I8,
    JR_CC_I8(CC),

    STOP,

    // Block 1
    LD_R8_R8(R8, R8),
    HALT,

    // Block 2
    ADD_A_R8(R8),
    ADC_A_R8(R8),
    SUB_A_R8(R8),
    SBC_A_R8(R8),
    AND_A_R8(R8),
    XOR_A_R8(R8),
    OR_A_R8(R8),
    CP_A_R8(R8),

    // Block 3
    ADD_A_I8,
    ADC_A_I8,
    SUB_A_I8,
    SBC_A_I8,
    AND_A_I8,
    XOR_A_I8,
    OR_A_I8,
    CP_A_I8,

    RET_CC(CC),
    RET,
    RETI,
    JP_CC_I16(CC),
    JP_I16,
    JP_HL,
    CALL_CC_I16(CC),
    CALL_I16,
    RST(u8),

    POP_R16(R16),
    PUSH_R16(R16),

    CB_PREFIX,

    LDH_C_A,
    LDH_I8_A,
    LD_I16_A,
    LDH_A_C,
    LDH_A_I8,
    LD_A_I16,

    ADD_SP_I8,
    LD_HL_SPI8,
    LD_SP_HL,

    DI,
    EI,

    // CB prefix instructions
    CB_RLC_R8(R8),
    CB_RRC_R8(R8),
    CB_RL_R8(R8),
    CB_RR_R8(R8),
    CB_SLA_R8(R8),
    CB_SRA_R8(R8),
    CB_SWAP_R8(R8),
    CB_SRL_R8(R8),

    CB_BIT_R8(u8, R8),
    CB_RES_R8(u8, R8),
    CB_SET_R8(u8, R8),
}

pub type Instruction = (Op, u8, u8);

#[rustfmt::skip]
macro_rules! add_cycles {
    ($inst:expr, $reg:expr) => {
        if format!("{:?}", $reg) == "HL" {
            // TODO: DEC [hl] requires 3 cycles
            if format!("{:?}", $inst).starts_with("CB") && !format!("{:?}", $inst).starts_with("CB_BIT_R8")
            { 2 } else { 1 }
        } else { 0 }
    };
}

#[rustfmt::skip]
macro_rules! set_op {
    ($op:expr, $code:expr, $inst:ident, $extra_bytes:expr, $cycles:expr) => {
        assert!(matches!($op[$code].0, Op::INVALID) || matches!($op[$code].0, Op::LD_R8_R8(R8::HL, R8::HL)), "Op at {:#04x} already set to {:?}", $code, $op[$code].0);
        $op[$code] = (Op::$inst, $extra_bytes, $cycles);
    };
    ($op:expr, $code:expr, $inst:ident, $extra_bytes:expr, $cycles:expr, $reg:expr, $shift:expr) => {
        for (i, reg) in $reg.iter().enumerate() {
            let (code, inst) = (i << $shift | $code, Op::$inst(*reg));
            assert!(matches!($op[code].0, Op::INVALID), "Op at {:#04x} already set to {:?}", code, $op[code].0);
            $op[code] = (inst, $extra_bytes, $cycles + add_cycles!(inst, reg));
        }
    };
    ($op:expr, $code:expr, $inst:ident, $extra_bytes:expr, $cycles:expr, $rega:expr, $shifta:expr, $regb:expr, $shiftb:expr) => {
        for (i, rega) in $rega.iter().enumerate() {
            for (j, regb) in $regb.iter().enumerate() {
                let (code, inst) = (i << $shifta | j << $shiftb | $code,  Op::$inst(*rega, *regb));
                assert!(matches!($op[code].0, Op::INVALID), "Op at {:#04x} already set to {:?}", code, $op[code].0);
                $op[code] = (inst, $extra_bytes, $cycles + add_cycles!(inst, rega) + add_cycles!(inst, regb));
            }
        }
    };
}

pub const OPMAP_SIZE: usize = 256;

pub fn load_opmaps() -> ([Instruction; OPMAP_SIZE], [Instruction; OPMAP_SIZE]) {
    let mut op = [(Op::INVALID, 0, 0); OPMAP_SIZE];

    // Block 0
    set_op!(op, 0x00, NOP, 0, 1);
    set_op!(op, 0x01, LD_R16_I16, 2, 3, ADDR_R16, 4);
    set_op!(op, 0x02, LD_R16_A, 0, 2, ADDR_R16_MEM, 4);
    set_op!(op, 0x22, LD_HLID_A, 0, 2, [true, false], 4);
    set_op!(op, 0x0A, LD_A_R16, 0, 2, ADDR_R16_MEM, 4);
    set_op!(op, 0x2A, LD_A_HLID, 0, 2, [true, false], 4);
    set_op!(op, 0x08, LD_I16_SP, 2, 5);

    set_op!(op, 0x03, INC_R16, 0, 2, ADDR_R16, 4);
    set_op!(op, 0x0B, DEC_R16, 0, 2, ADDR_R16, 4);
    set_op!(op, 0x09, ADD_HL_R16, 0, 2, ADDR_R16, 4);

    set_op!(op, 0x04, INC_R8, 0, 1, ADDR_R8, 3);
    set_op!(op, 0x05, DEC_R8, 0, 1, ADDR_R8, 3);
    set_op!(op, 0x06, LD_R8_I8, 1, 2, ADDR_R8, 3);

    set_op!(op, 0x07, RLCA, 0, 1);
    set_op!(op, 0x0F, RRCA, 0, 1);
    set_op!(op, 0x17, RLA, 0, 1);
    set_op!(op, 0x1F, RRA, 0, 1);
    set_op!(op, 0x27, DAA, 0, 1);
    set_op!(op, 0x2F, CPL, 0, 1);
    set_op!(op, 0x37, SCF, 0, 1);
    set_op!(op, 0x3F, CCF, 0, 1);

    set_op!(op, 0x18, JR_I8, 1, 3);
    set_op!(op, 0x20, JR_CC_I8, 1, 2, ADDR_CC, 3);

    set_op!(op, 0x10, STOP, 1, 0);

    // Block 1
    set_op!(op, 0x40, LD_R8_R8, 0, 1, ADDR_R8, 3, ADDR_R8, 0);
    set_op!(op, 0x76, HALT, 0, 0);

    // Block 2
    set_op!(op, 0x80, ADD_A_R8, 0, 1, ADDR_R8, 0);
    set_op!(op, 0x88, ADC_A_R8, 0, 1, ADDR_R8, 0);
    set_op!(op, 0x90, SUB_A_R8, 0, 1, ADDR_R8, 0);
    set_op!(op, 0x98, SBC_A_R8, 0, 1, ADDR_R8, 0);
    set_op!(op, 0xA0, AND_A_R8, 0, 1, ADDR_R8, 0);
    set_op!(op, 0xA8, XOR_A_R8, 0, 1, ADDR_R8, 0);
    set_op!(op, 0xB0, OR_A_R8, 0, 1, ADDR_R8, 0);
    set_op!(op, 0xB8, CP_A_R8, 0, 1, ADDR_R8, 0);

    // Block 3
    set_op!(op, 0xC6, ADD_A_I8, 1, 2);
    set_op!(op, 0xCE, ADC_A_I8, 1, 2);
    set_op!(op, 0xD6, SUB_A_I8, 1, 2);
    set_op!(op, 0xDE, SBC_A_I8, 1, 2);
    set_op!(op, 0xE6, AND_A_I8, 1, 2);
    set_op!(op, 0xEE, XOR_A_I8, 1, 2);
    set_op!(op, 0xF6, OR_A_I8, 1, 2);
    set_op!(op, 0xFE, CP_A_I8, 1, 2);

    set_op!(op, 0xC0, RET_CC, 0, 2, ADDR_CC, 3);
    set_op!(op, 0xC9, RET, 0, 4);
    set_op!(op, 0xD9, RETI, 0, 4);
    set_op!(op, 0xC2, JP_CC_I16, 2, 3, ADDR_CC, 3);
    set_op!(op, 0xC3, JP_I16, 2, 4);
    set_op!(op, 0xE9, JP_HL, 0, 1);
    set_op!(op, 0xC4, CALL_CC_I16, 2, 3, ADDR_CC, 3);
    set_op!(op, 0xCD, CALL_I16, 2, 6);
    set_op!(op, 0xC7, RST, 0, 4, ADDR_3, 3);

    set_op!(op, 0xC1, POP_R16, 0, 3, ADDR_R16_STK, 4);
    set_op!(op, 0xC5, PUSH_R16, 0, 4, ADDR_R16_STK, 4);

    set_op!(op, 0xCB, CB_PREFIX, 0, 1);

    set_op!(op, 0xE2, LDH_C_A, 0, 2);
    set_op!(op, 0xE0, LDH_I8_A, 1, 3);
    set_op!(op, 0xEA, LD_I16_A, 2, 4);
    set_op!(op, 0xF2, LDH_A_C, 0, 2);
    set_op!(op, 0xF0, LDH_A_I8, 1, 3);
    set_op!(op, 0xFA, LD_A_I16, 2, 4);

    set_op!(op, 0xE8, ADD_SP_I8, 1, 4);
    set_op!(op, 0xF8, LD_HL_SPI8, 1, 3);
    set_op!(op, 0xF9, LD_SP_HL, 0, 2);

    set_op!(op, 0xF3, DI, 0, 1);
    set_op!(op, 0xFB, EI, 0, 1);

    // Initialize lookup table for CB-prefixed instructions
    let mut cb_op = [(Op::INVALID, 0, 0); OPMAP_SIZE];

    set_op!(cb_op, 0x00, CB_RLC_R8, 0, 1, ADDR_R8, 0);
    set_op!(cb_op, 0x08, CB_RRC_R8, 0, 1, ADDR_R8, 0);
    set_op!(cb_op, 0x10, CB_RL_R8, 0, 1, ADDR_R8, 0);
    set_op!(cb_op, 0x18, CB_RR_R8, 0, 1, ADDR_R8, 0);
    set_op!(cb_op, 0x20, CB_SLA_R8, 0, 1, ADDR_R8, 0);
    set_op!(cb_op, 0x28, CB_SRA_R8, 0, 1, ADDR_R8, 0);
    set_op!(cb_op, 0x30, CB_SWAP_R8, 0, 1, ADDR_R8, 0);
    set_op!(cb_op, 0x38, CB_SRL_R8, 0, 1, ADDR_R8, 0);

    set_op!(cb_op, 0x40, CB_BIT_R8, 0, 1, ADDR_3, 3, ADDR_R8, 0);
    set_op!(cb_op, 0x80, CB_RES_R8, 0, 1, ADDR_3, 3, ADDR_R8, 0);
    set_op!(cb_op, 0xC0, CB_SET_R8, 0, 1, ADDR_3, 3, ADDR_R8, 0);

    (op, cb_op)
}

#[cfg(test)]
mod test {
    use super::load_opmaps;
    use super::{Op, R8};

    #[test]
    fn complete_op_table() {
        // Check that all the instructions are loaded properly
        let (op, cb_op) = load_opmaps();
        assert_eq!(op.len(), 256);
        assert_eq!(cb_op.len(), 256);
        // Check that the addional op cycles are set correctly
        assert!(matches!(op[0x76].0, Op::HALT));
        assert!(matches!(op[0xF0].0, Op::LDH_A_I8));
        assert!(matches!(op[0x06].0, Op::LD_R8_I8(R8::B)));
        assert_eq!(op[0x06].2, 2);
        assert!(matches!(op[0x36].0, Op::LD_R8_I8(R8::HL)));
        assert_eq!(op[0x36].2, 3);
        assert!(matches!(cb_op[0x07].0, Op::CB_RLC_R8(R8::A)));
        assert_eq!(cb_op[0x07].2, 1);
        assert!(matches!(cb_op[0x06].0, Op::CB_RLC_R8(R8::HL)));
        assert_eq!(cb_op[0x06].2, 3);
        assert!(matches!(cb_op[0x5F].0, Op::CB_BIT_R8(3, R8::A)));
        assert_eq!(cb_op[0x5F].2, 1);
        assert!(matches!(cb_op[0x5E].0, Op::CB_BIT_R8(3, R8::HL)));
        assert_eq!(cb_op[0x5E].2, 2);
        // Check that the only INVALID instructions left are the correct ones
        let expected_op_invalid: Vec<usize> = vec![0xD3, 0xDB, 0xDD, 0xE3, 0xE4, 0xEB, 0xEC, 0xED, 0xF4, 0xFC, 0xFD];
        let mut op_invalid: Vec<usize> = Vec::new();
        for i in 0..op.len() {
            if matches!(op[i].0, Op::INVALID) {
                op_invalid.push(i);
            }
            assert!(!matches!(cb_op[i].0, Op::INVALID));
        }
        assert_eq!(op_invalid, expected_op_invalid);
    }
}
