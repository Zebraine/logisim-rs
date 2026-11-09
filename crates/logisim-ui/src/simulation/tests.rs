use super::*;
#[test]
fn hex_input_rejects_overflow_and_preserves_full_u32() {
    assert_eq!(parse_input("0xA", 4), Some(10));
    assert_eq!(parse_input("10", 4), None);
    assert_eq!(parse_input("FFFFFFFF", 32), Some(u32::MAX));
    assert_eq!(parse_input("100000000", 32), None);
    assert_eq!(parse_input("X", 4), None);
    assert_eq!(parse_input("-1", 4), None);
}
