use super::*;
#[test]
fn wire_roundtrip_and_rejected_ranges() {
    let mut bytes = vec![];
    write_request(&mut bytes, Some(u64::MAX), &[1, 2, 3]).unwrap();
    assert_eq!(
        read_request(&bytes).unwrap(),
        (Some(u64::MAX), &[1, 2, 3][..])
    );
    for len in 0..16 {
        assert!(read_request(&bytes[..len]).is_err());
    }
    bytes[4] = 2;
    assert!(read_request(&bytes).is_err());
    assert!(write_request(&mut bytes, None, &vec![0; MAX_STATE + 1]).is_err());
    let image = Image {
        revision: 9,
        width: 2,
        height: 1,
        bounds: [[0, 0], [20, 10]],
        pixels: vec![255, 0, 0, 255, 0, 255, 0, 128],
    };
    write_output(&mut bytes, &Output::Image(image.clone())).unwrap();
    assert_eq!(bytes.len(), 48);
    assert_eq!(read_output(&bytes, None).unwrap(), Output::Image(image));
    for len in 0..bytes.len() {
        assert!(read_output(&bytes[..len], None).is_err());
    }
    let mut bad = bytes.clone();
    bad[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(read_output(&bad, None).is_err());
    bad = bytes.clone();
    bad[24..28].copy_from_slice(&4097i32.to_le_bytes());
    assert!(read_output(&bad, None).is_err());
    bytes.push(0);
    assert!(read_output(&bytes, None).is_err());
    write_output(&mut bytes, &Output::Unchanged(9)).unwrap();
    assert_eq!(read_output(&bytes, Some(9)).unwrap(), Output::Unchanged(9));
    assert!(read_output(&bytes, None).is_err());
    assert!(read_output(&bytes, Some(8)).is_err());
    bytes[16] = 1;
    assert!(read_output(&bytes, Some(9)).is_err());
    write_output(&mut bytes, &Output::None).unwrap();
    assert_eq!(read_output(&bytes, None).unwrap(), Output::None);
    write_error(&mut bytes, &"界".repeat(1000));
    assert!(bytes.len() <= 2088);
    assert!(read_output(&bytes, None).unwrap_err().starts_with('界'));
}
