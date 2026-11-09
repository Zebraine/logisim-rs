use super::*;

#[test]
fn native_image_runs_comments_and_short_files_zero_fill() {
    let contents = parse_hex("v2.0 raw\r\n12 AB 3*cd 0#comment\n\t2*EF\n", 4, 8).unwrap();
    assert_eq!(
        (0..16).map(|a| contents.get(a)).collect::<Vec<_>>(),
        [
            0x12, 0xab, 0xcd, 0xcd, 0xcd, 0, 0xef, 0xef, 0, 0, 0, 0, 0, 0, 0, 0
        ]
    );
    assert_eq!(parse_hex("v2.0 raw\n10*1 2", 4, 8).unwrap().get(10), 2);
    assert_eq!(parse_hex("v2.0 raw\n0*ff 12", 2, 8).unwrap().get(0), 0x12);
    assert_eq!(
        parse_hex("v2.0 raw\n#empty", 24, 32).unwrap(),
        MemoryContents::default()
    );
}

#[test]
fn values_mask_to_width_and_large_runs_stay_compact() {
    for bits in [1, 8, 32] {
        let mask = u32::MAX >> (32 - bits);
        let contents = parse_hex(
            "v2.0 raw\n1ff ffffffff 100000000 +a -1 7fffffffffffffff",
            4,
            bits,
        )
        .unwrap();
        for (a, value) in [0x1ff, u32::MAX, 0, 10, u32::MAX, u32::MAX]
            .into_iter()
            .enumerate()
        {
            assert_eq!(contents.get(a as u32), value & mask);
        }
    }
    let contents = parse_hex("v2.0 raw\n16777216*ffffffff", 24, 32).unwrap();
    assert_eq!(contents.get(0xff_ffff), u32::MAX);
    assert_eq!(contents.get(0x100_0000), 0);
    assert_eq!(contents.boundaries().count(), 2);
}

#[test]
fn invalid_or_unsupported_images_are_rejected_with_line_numbers() {
    for header in [
        "",
        "12 34",
        "V2.0 RAW\n12",
        "\nv2.0 raw\n12",
        "v2.0 raw # note\n12",
        "v3.0 hex words plain\n12",
    ] {
        assert_eq!(parse_hex(header, 4, 8), Err(HexError::Header));
    }
    for word in ["zz", "0x12", "8000000000000000", "1*", "+", "1*2*3"] {
        assert_eq!(
            parse_hex(&format!("v2.0 raw\n12\n{word}"), 4, 8),
            Err(HexError::Word { line: 3 })
        );
    }
    for count in ["-1", "+1", "a", ""] {
        assert_eq!(
            parse_hex(&format!("v2.0 raw\n{count}*1"), 4, 8),
            Err(HexError::Count { line: 2 })
        );
    }
    for data in ["17*ff", "16*0 1", "4294967296*1"] {
        assert_eq!(
            parse_hex(&format!("v2.0 raw\n{data}"), 4, 8),
            Err(HexError::Capacity { line: 2 })
        );
    }
    assert_eq!(parse_hex("v2.0 raw", 0, 8), Err(HexError::Dimensions));
    assert_eq!(parse_hex("v2.0 raw", 4, 33), Err(HexError::Dimensions));
    assert_eq!(
        parse_hex(&" ".repeat(crate::MAX_BYTES + 1), 4, 8),
        Err(HexError::TooLarge)
    );
}
