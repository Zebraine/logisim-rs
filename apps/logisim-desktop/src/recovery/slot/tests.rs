use super::*;
use crate::tests::support::Temp;

#[test]
fn failed_commit_preserves_previous_pair_and_interrupted_commit_is_detected() {
    let temp = Temp::new();
    let dir = temp.path("recovery");
    let mut pair = Pair::default();
    for existing in [false, true] {
        if existing {
            pair.write(
                &dir,
                b"old".to_vec(),
                None,
                None,
                logisim_file::ProjectFormat::Circ,
            )
            .unwrap();
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
    assert!(pair.clear(&dir, None).is_err());
    assert_eq!(fs::read(dir.join(DATA)).unwrap(), b"interrupted");
}

#[test]
fn failure_on_second_removal_restores_the_first_file() {
    let temp = Temp::new();
    let dir = temp.path("recovery");
    fs::create_dir_all(dir.join(DATA)).unwrap();
    fs::write(dir.join(META), b"diagnostic record").unwrap();
    let mut pair = Pair::read(&dir).unwrap();
    assert!(pair.clear(&dir, None).is_err());
    assert_eq!(fs::read(dir.join(META)).unwrap(), b"diagnostic record");
    assert_eq!(Pair::read(&dir).unwrap(), pair);
}

#[test]
fn failed_clear_of_two_invalid_entries_keeps_the_unbuffered_record() {
    let tmp = Temp::new();
    let dir = tmp.path("recovery");
    fs::create_dir_all(dir.join(DATA)).unwrap();
    let record = vec![b'x'; 65537];
    fs::write(dir.join(META), &record).unwrap();
    let mut pair = Pair::read(&dir).unwrap();
    assert!(pair.clear(&dir, None).is_err());
    assert_eq!(fs::read(dir.join(META)).unwrap(), record);
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
}
