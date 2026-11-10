use super::*;
#[test]
fn encodings_and_forward_labels() {
    let words=assemble(".equ PORT 0xff30\nLI r1, 0xbeef\nADD r3,r1,r2\nST r3,r0,PORT\nBZ r3,end\nJAL r7,end\nend: JR r7\nHALT").unwrap();
    assert_eq!(
        words,
        [
            0x0900beef, 0x13280000, 0x8b00ff30, 0x90600005, 0xaf000005, 0xb0e00000, 0xf8000000
        ]
    );
}
#[test]
fn bounded_images_and_literals() {
    assert_eq!(
        assemble(".word 0xffffffff\n.org 3\nADDI r1,r1,-1").unwrap(),
        [u32::MAX, 0, 0, 0x6120ffff]
    );
    for source in [
        "LI r8,1",
        "LI r1,65536",
        "LI r1,-32769",
        "JMP -1",
        "JAL r7,-1",
        "a: NOP\na: HALT",
        ".org 65535\nNOP\nHALT",
        ".word",
        "BAD",
        "LD r1,r2",
        ".org 10\n.org 9",
        "LI r1,missing",
        "LI r1,--9223372036854775808",
        ".equ x 1\nx: HALT",
        "",
    ] {
        assert!(assemble(source).is_err(), "{source}");
    }
    let words = assemble("li R1,0b101 ; comment\nHALT # done").unwrap();
    let image = logisim_file::parse_hex(&hex(&words), 16, 32).unwrap();
    for (i, v) in words.iter().enumerate() {
        assert_eq!(image.get(i as u32), *v);
    }
}
