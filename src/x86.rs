use self::tcg::{TCGOp, TCGOpcode, TCGvType, TCG};
use super::tcg;

#[derive(PartialEq, Debug)]
#[allow(non_camel_case_types)]
enum X86Opcode {
    MOV_EV_IV = 0xc7,
    MOV_GV_EV = 0x8b,
    MOV_EV_GV = 0x89,
    ADD_EV_IV = 0x81,
    ADD_GV_EV = 0x03,
    ADD_EAX_IV = 0x05,
    SUB_GV_EV = 0x2b,
    AND_GV_EV = 0x23,
    OR_GV_EV = 0x0b,
    XOR_GV_EV = 0x33,
    AND_EAX_IV = 0x25,
    OR_EAX_IV = 0x0d,
    XOR_EAX_IV = 0x35,
}

#[derive(PartialEq, Debug)]
#[allow(non_camel_case_types)]
enum X86ModRM {
    MOD_00 = 0x05,
    MOD_01 = 0x45,
    MOD_10 = 0x85,
    MOD_11 = 0xc5,
}

macro_rules! conv_gpr_offset {
    ($gpr_addr: expr) => {
        ($gpr_addr as u32) * 8
    };
}

pub struct TCGX86;

impl TCGX86 {
    fn tcg_modrm_out(op: X86Opcode, modrm: X86ModRM, mc: &mut Vec<u8>) {
        Self::tcg_out(((modrm as u32) << 16) | (op as u32) << 8 | 0x48, 3, mc);
    }

    fn tcg_gen_rrr(op: X86Opcode, tcg: &tcg::TCGOp, mc: &mut Vec<u8>) {
        let arg0 = tcg.arg0.unwrap();
        let arg1 = tcg.arg1.unwrap();
        let arg2 = tcg.arg2.unwrap();

        assert_eq!(arg0.t, TCGvType::Register);
        assert_eq!(arg1.t, TCGvType::Register);
        assert_eq!(arg1.t, TCGvType::Register);

        // mov    reg_offset(%rbp),%eax
        Self::tcg_modrm_out(X86Opcode::MOV_GV_EV, X86ModRM::MOD_10, mc);
        Self::tcg_out(conv_gpr_offset!(arg1.value), 4, mc);

        // add    reg_offset(%rbp),%eax
        Self::tcg_modrm_out(op, X86ModRM::MOD_10, mc);
        Self::tcg_out(conv_gpr_offset!(arg2.value), 4, mc);

        // mov    %eax,reg_offset(%rbp)
        Self::tcg_modrm_out(X86Opcode::MOV_EV_GV, X86ModRM::MOD_10, mc);
        Self::tcg_out(conv_gpr_offset!(arg0.value), 4, mc);
    }

    fn tcg_gen_rri(op: X86Opcode, tcg: &tcg::TCGOp, mc: &mut Vec<u8>) {
        let arg0 = tcg.arg0.unwrap();
        let arg1 = tcg.arg1.unwrap();
        let arg2 = tcg.arg2.unwrap();

        assert_eq!(arg0.t, TCGvType::Register);
        assert_eq!(arg1.t, TCGvType::Register);
        assert_eq!(arg2.t, TCGvType::Immediate);

        // mov    reg_offset(%rbp),%eax
        Self::tcg_modrm_out(X86Opcode::MOV_GV_EV, X86ModRM::MOD_10, mc);
        Self::tcg_out(conv_gpr_offset!(arg1.value), 4, mc);

        // add    imm16,%eax
        Self::tcg_out(op as u32, 1, mc);
        Self::tcg_out(arg2.value as u32, 4, mc);

        // mov    %eax,reg_offset(%rbp)
        Self::tcg_modrm_out(X86Opcode::MOV_EV_GV, X86ModRM::MOD_10, mc);
        Self::tcg_out(conv_gpr_offset!(arg0.value), 4, mc);
    }

    fn tcg_out(inst: u32, byte_len: usize, v: &mut Vec<u8>) {
        for (i, be) in inst.to_le_bytes().iter().enumerate() {
            if i < byte_len {
                println!("register = {:02x}", &be);
                v.push(*be);
            }
        }
    }
}

impl TCG for TCGX86 {
    fn tcg_gen_addi(tcg: &tcg::TCGOp, mc: &mut Vec<u8>) {
        let arg0 = tcg.arg0.unwrap();
        let arg1 = tcg.arg1.unwrap();
        let arg2 = tcg.arg2.unwrap();

        assert_eq!(arg0.t, TCGvType::Register);
        assert_eq!(arg1.t, TCGvType::Register);

        if arg0.value == 0 {
            // if destination is x0, skip generate host machine code.
            return;
        }

        if arg2.t == tcg::TCGvType::Immediate {
            if arg1.value == 0 {
                // if source register is x0, just generate immediate value.
                // movl   imm,reg_addr(%rbp)
                Self::tcg_modrm_out(X86Opcode::MOV_EV_IV, X86ModRM::MOD_10, mc);
                Self::tcg_out(conv_gpr_offset!(arg0.value), 4, mc);
                Self::tcg_out(arg2.value as u32, 4, mc);
                return;
            }

            Self::tcg_gen_rri(X86Opcode::ADD_EAX_IV, tcg, mc);
            return;
        } else {
            if arg1.value == 0 {
                // if source register is x0, just mov gpr value.
                // movl   reg_addr(%rbp),%eax
                Self::tcg_modrm_out(X86Opcode::MOV_EV_GV, X86ModRM::MOD_10, mc);
                Self::tcg_out(conv_gpr_offset!(arg2.value), 4, mc);
                // movl   %eax,reg_addr(%rbp)
                Self::tcg_modrm_out(X86Opcode::MOV_EV_GV, X86ModRM::MOD_10, mc);
                Self::tcg_out(conv_gpr_offset!(arg0.value), 4, mc);
                return;
            }
            Self::tcg_gen_rrr(X86Opcode::ADD_GV_EV, tcg, mc);
            return;
        }
    }

    fn tcg_gen_sub(tcg: &TCGOp, mc: &mut Vec<u8>) {
        let arg0 = tcg.arg0.unwrap();

        if arg0.value == 0 {
            // if destination is x0, skip generate host machine code.
            return;
        }
        Self::tcg_gen_rrr(X86Opcode::SUB_GV_EV, tcg, mc);
    }

    fn tcg_gen_and(tcg: &TCGOp, mc: &mut Vec<u8>) {
        let arg0 = tcg.arg0.unwrap();
        let arg1 = tcg.arg1.unwrap();
        let arg2 = tcg.arg2.unwrap();

        assert_eq!(arg0.t, TCGvType::Register);
        assert_eq!(arg1.t, TCGvType::Register);

        if arg0.value == 0 {
            // if destination is x0, skip generate host machine code.
            return;
        }
        if arg2.t == tcg::TCGvType::Immediate {
            if arg1.value == 0 {
                // if source register is x0, just generate immediate value.
                // movl   imm,reg_addr(%rbp)
                Self::tcg_modrm_out(X86Opcode::MOV_EV_IV, X86ModRM::MOD_10, mc);
                Self::tcg_out(conv_gpr_offset!(arg0.value), 4, mc);
                Self::tcg_out(arg2.value as u32, 4, mc);
                return;
            }

            Self::tcg_gen_rri(X86Opcode::AND_EAX_IV, tcg, mc);
            return;
        } else {
            if arg1.value == 0 {
                // if source register is x0, just mov gpr value.
                // movl   reg_addr(%rbp),%eax
                Self::tcg_modrm_out(X86Opcode::MOV_EV_GV, X86ModRM::MOD_10, mc);
                Self::tcg_out(conv_gpr_offset!(arg2.value), 4, mc);
                // movl   %eax,reg_addr(%rbp)
                Self::tcg_modrm_out(X86Opcode::MOV_EV_GV, X86ModRM::MOD_10, mc);
                Self::tcg_out(conv_gpr_offset!(arg0.value), 4, mc);
                return;
            }
            Self::tcg_gen_rrr(X86Opcode::AND_GV_EV, tcg, mc);
            return;
        }
    }

    fn tcg_gen_or(tcg: &TCGOp, mc: &mut Vec<u8>) {
        let arg0 = tcg.arg0.unwrap();
        let arg1 = tcg.arg1.unwrap();
        let arg2 = tcg.arg2.unwrap();

        assert_eq!(arg0.t, TCGvType::Register);
        assert_eq!(arg1.t, TCGvType::Register);

        if arg0.value == 0 {
            // if destination is x0, skip generate host machine code.
            return;
        }
        if arg2.t == tcg::TCGvType::Immediate {
            if arg1.value == 0 {
                // if source register is x0, just generate immediate value.
                // movl   imm,reg_addr(%rbp)
                Self::tcg_modrm_out(X86Opcode::MOV_EV_IV, X86ModRM::MOD_10, mc);
                Self::tcg_out(conv_gpr_offset!(arg0.value), 4, mc);
                Self::tcg_out(arg2.value as u32, 4, mc);
                return;
            }

            Self::tcg_gen_rri(X86Opcode::OR_EAX_IV, tcg, mc);
            return;
        } else {
            if arg1.value == 0 {
                // if source register is x0, just mov gpr value.
                // movl   reg_addr(%rbp),%eax
                Self::tcg_modrm_out(X86Opcode::MOV_EV_GV, X86ModRM::MOD_10, mc);
                Self::tcg_out(conv_gpr_offset!(arg2.value), 4, mc);
                // movl   %eax,reg_addr(%rbp)
                Self::tcg_modrm_out(X86Opcode::MOV_EV_GV, X86ModRM::MOD_10, mc);
                Self::tcg_out(conv_gpr_offset!(arg0.value), 4, mc);
                return;
            }
            Self::tcg_gen_rrr(X86Opcode::OR_GV_EV, tcg, mc);
            return;
        }
    }

    fn tcg_gen_xor(tcg: &TCGOp, mc: &mut Vec<u8>) {
        let arg0 = tcg.arg0.unwrap();
        let arg1 = tcg.arg1.unwrap();
        let arg2 = tcg.arg2.unwrap();

        assert_eq!(arg0.t, TCGvType::Register);
        assert_eq!(arg1.t, TCGvType::Register);

        if arg0.value == 0 {
            // if destination is x0, skip generate host machine code.
            return;
        }

        if arg2.t == tcg::TCGvType::Immediate {
            if arg1.value == 0 {
                // if source register is x0, just generate immediate value.
                // movl   imm,reg_addr(%rbp)
                Self::tcg_modrm_out(X86Opcode::MOV_EV_IV, X86ModRM::MOD_10, mc);
                Self::tcg_out(conv_gpr_offset!(arg0.value), 4, mc);
                Self::tcg_out(arg2.value as u32, 4, mc);
                return;
            }

            Self::tcg_gen_rri(X86Opcode::XOR_EAX_IV, tcg, mc);
            return;
        } else {
            if arg1.value == 0 {
                // if source register is x0, just mov gpr value.
                // movl   reg_addr(%rbp),%eax
                Self::tcg_modrm_out(X86Opcode::MOV_EV_GV, X86ModRM::MOD_10, mc);
                Self::tcg_out(conv_gpr_offset!(arg2.value), 4, mc);
                // movl   %eax,reg_addr(%rbp)
                Self::tcg_modrm_out(X86Opcode::MOV_EV_GV, X86ModRM::MOD_10, mc);
                Self::tcg_out(conv_gpr_offset!(arg0.value), 4, mc);
                return;
            }
            Self::tcg_gen_rrr(X86Opcode::XOR_GV_EV, tcg, mc);
            return;
        }
    }

    fn tcg_gen_ret(tcg: &TCGOp, mc: &mut Vec<u8>) {
        let op = tcg.op.unwrap();
        let arg0 = tcg.arg0.unwrap();
        let arg1 = tcg.arg1.unwrap();

        assert_eq!(arg0.t, TCGvType::Register);
        assert_eq!(arg1.t, TCGvType::Register);
        assert_eq!(op, TCGOpcode::JMP);

        if arg0.t == tcg::TCGvType::Register
            && arg0.value == 0
            && arg1.t == tcg::TCGvType::Register
            && arg1.value == 1
        {
            // mov 0x50(%rbp), eax  0x50 is location of x10
            let raw_mc: u32 = 0x50_458b;
            Self::tcg_out(raw_mc, 3, mc);
            return;
        }
        panic!("This function is not supported!")
    }
}
