use super::*;
#[test]
fn observed_instructions_boundaries_and_nonstandard_formats() {
    for (word, text) in [
        (0x00520183, "lb gp,(5)tp"),
        (0xfff201a3, "sb t6,(-29)tp"),
        (0x40525193, "srai gp,tp,1029"),
        (0xfe524193, "xori gp,tp,5"),
        (0x02525193, ""),
        (0x025231b3, "mulhu gp,tp,t0"),
        (0x405211b3, ""),
        (0x00520130, "add sp,tp,t0"),
        (0xfe520fe3, "beq tp,t0,-2"),
        (0x805201e3, "beq tp,t0,-2046"),
        (0xfffff1ef, "jal gp,-2"),
        (0x800001ef, "jal gp,-1048576"),
        (0x800001b7, "lui gp,-524288"),
        (0xfffff197, "auipc gp,-1"),
        (0x800251f3, "csrrwi gp,2048,4"),
        (0xfff211f3, "csrrw gp,4095,tp"),
        (0x10200173, "sret"),
        (0x00100070, "break"),
        (0x10500073, "wfi"),
        (0x100f, "fence.I"),
        (0xffffffff, ""),
        (0, "Nop/Invalid Instr"),
    ] {
        assert_eq!(decode(word), text, "{word:08x}");
    }
    for (i, name) in REGISTERS.iter().enumerate() {
        assert_eq!(
            decode(((i as u32) << 7) | ((i as u32) << 15) | ((i as u32) << 20) | 0x33),
            format!("add {name},{name},{name}")
        );
    }
}
#[test]
fn observed_branch_and_jump_bit_weights() {
    for (bit, offset) in [
        (7, 2048),
        (8, 2),
        (9, 4),
        (10, 8),
        (11, 16),
        (25, 32),
        (26, 64),
        (27, 128),
        (28, 256),
        (29, 512),
        (30, 1024),
        (31, -4096),
    ] {
        assert_eq!(decode((1 << bit) | 0x63), format!("beq zero,zero,{offset}"));
    }
    for bit in 12..=31 {
        let offset = match bit {
            12..=19 => 1 << bit,
            20 => 2048,
            21..=30 => 1 << (bit - 20),
            _ => -1048576,
        };
        assert_eq!(decode((1 << bit) | 0x6f), format!("jal zero,{offset}"));
    }
}
