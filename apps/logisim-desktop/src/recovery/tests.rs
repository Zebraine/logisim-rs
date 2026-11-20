use super::*;
use crate::tests::support::Temp;
use logisim_core::{Component, Kind, Point};
use std::fs;

#[test]
fn deadlines_reset_on_options_and_failure_without_writing_source_or_protected_imports() {
    let temp = Temp::new();
    let dir = temp.path("recovery");
    let path = temp.path("source.circ");
    let original = logisim_file::serialize(&Project::default()).unwrap();
    fs::write(&path, &original).unwrap();
    let mut disk = Storage::new();
    let (mut project, _) = disk.load(&path).unwrap();
    let mut auto = Autosave::at(Some(dir.clone()));
    let start = Instant::now();
    let time = |s| start + Duration::from_secs(s);
    let mut options = FileSettings {
        autosave: true,
        autosave_minutes: 1,
    };
    auto.update(time(0), options, false, &disk, &project);
    project
        .main_circuit_mut()
        .circuit
        .add_component(Component::new(Kind::Pin, Point::ZERO));
    auto.update(time(100), options, true, &disk, &project);
    auto.update(time(159), options, true, &disk, &project);
    assert!(!dir.exists());
    options.autosave_minutes = 2;
    auto.update(time(160), options, true, &disk, &project);
    auto.update(time(279), options, true, &disk, &project);
    assert!(!dir.exists());
    fs::write(&dir, b"blocked").unwrap();
    auto.update(time(280), options, true, &disk, &project);
    assert!(auto.error.is_some());
    fs::remove_file(&dir).unwrap();
    auto.update(time(399), options, true, &disk, &project);
    assert!(!dir.exists());
    auto.update(time(400), options, true, &disk, &project);
    assert!(auto.error.is_none() && auto.last_success == Some(time(400)));
    assert_eq!(
        auto.observed.metadata().unwrap().source.as_deref(),
        Some(path.as_path())
    );
    let xml = fs::read_to_string(dir.join(slot::DATA)).unwrap();
    assert_eq!(logisim_file::parse(&xml).unwrap().document, project);
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    let old = Pair::read(&dir).unwrap();
    project.circuits[0].name.clear();
    auto.update(time(519), options, true, &disk, &project);
    assert!(auto.error.is_none());
    auto.update(time(520), options, true, &disk, &project);
    assert!(auto.error.is_some());
    assert_eq!(Pair::read(&dir).unwrap(), old);
    options.autosave = false;
    auto.update(time(900), options, true, &disk, &project);
    assert_eq!(Pair::read(&dir).unwrap(), old);
    auto.finish_document();
    assert!(Pair::read(&dir).unwrap().empty());
    disk.protected = Some(path);
    options.autosave = true;
    for at in [1000, 2000] {
        auto.update(time(at), options, true, &disk, &Project::default());
    }
    assert!(Pair::read(&dir).unwrap().empty());
}

#[test]
fn untitled_snapshots_use_null_source_and_windows_cannot_replace_another_slot() {
    let temp = Temp::new();
    let dir = temp.path("recovery");
    let mut first = Autosave::at(Some(dir.clone()));
    let mut other = Autosave::at(Some(dir.clone()));
    let project = Project::default();
    let storage = Storage::default();
    let options = FileSettings::default();
    let now = Instant::now();
    for state in [&mut first, &mut other] {
        state.update(now, options, true, &storage, &project);
        state.update(
            now + Duration::from_secs(300),
            options,
            true,
            &storage,
            &project,
        );
    }
    assert!(first.owned && other.error.is_some() && !other.owned);
    other.finish_document();
    let pair = Pair::read(&dir).unwrap();
    assert!(pair.metadata().unwrap().source.is_none());
    assert_eq!(pair, first.observed);
}
