use super::*;
use crate::tests::support::Temp;

#[test]
fn failed_commit_preserves_previous_pair_and_interrupted_commit_is_detected() {
    let temp = Temp::new();
    let dir = temp.path("recovery");
    let mut pair = Pair::default();
    for existing in [false, true] {
        if existing {
            pair.write(&dir, b"old".to_vec(), None, None).unwrap();
        }
        let previous = Pair::read(&dir).unwrap();
        assert!(
            pair.replace(&dir, b"next".to_vec(), Vec::new(), |_, _, _| Err(
                "write failed".into()
            ))
            .is_err()
        );
        assert_eq!(Pair::read(&dir).unwrap(), previous);
    }
    fs::write(dir.join(DATA), b"interrupted").unwrap();
    assert!(Pair::read(&dir).unwrap().metadata().is_err());
    assert!(pair.clear(&dir).is_err());
    assert_eq!(fs::read(dir.join(DATA)).unwrap(), b"interrupted");
}
