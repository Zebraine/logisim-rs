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
    assert!(first.view().owned && other.error.is_some() && !other.view().owned);
    other.finish_document();
    let pair = Pair::read(&dir).unwrap();
    assert!(pair.metadata().unwrap().source.is_none());
    assert_eq!(pair, first.observed);
}

#[test]
fn malformed_xml_with_consistent_metadata_is_damaged_and_survives_restarts() {
    let tmp = Temp::new();
    let dir = tmp.path("recovery");
    let mut pair = Pair::default();
    pair.write(&dir, b"<project>".to_vec(), None, None).unwrap();
    let mut auto = Autosave::at(Some(dir.clone()));
    assert!(auto.pending());
    assert!(matches!(
        auto.view().state,
        logisim_ui::SnapshotState::Damaged
    ));
    let mut disk = Storage::default();
    assert!(auto.recover(&mut disk).is_err());
    auto.finish_document();
    assert_eq!(Pair::read(&dir).unwrap(), pair);
}

fn write_checkpoint(auto: &mut Autosave, project: &Project) -> Instant {
    let start = Instant::now();
    for at in [start, start + Duration::from_secs(300)] {
        auto.update(
            at,
            FileSettings::default(),
            true,
            &Storage::default(),
            project,
        );
    }
    assert!(auto.view().owned);
    start
}

#[test]
fn observed_foreign_copy_survives_settings_clear_and_finish_but_not_startup_discard() {
    let temp = Temp::new();
    let dir = temp.path("recovery");
    let mut owner = Autosave::at(Some(dir.clone()));
    let mut project = Project::default();
    let now = write_checkpoint(&mut owner, &project);
    let mut other = Autosave::at(Some(dir.clone()));
    let saved = Pair::read(&dir).unwrap();
    assert!(!other.view().owned && other.clear_owned().is_err());
    assert!(other.clear_from_settings().is_err());
    let mut disk = Storage::default();
    let source = temp.path("other.circ");
    disk.save(&project, &source, crate::storage::SaveMode::Current)
        .unwrap();
    let checksum = disk.source_checksum();
    other.changed = false;
    assert_eq!(
        other.recover(&mut disk).unwrap_err(),
        "Recovery copy is owned by another window."
    );
    assert!(other.changed && !other.view().owned && other.written.is_none());
    assert_eq!(
        other.error.as_deref(),
        Some("Recovery copy is owned by another window.")
    );
    assert_eq!(disk.path.as_ref(), Some(&source));
    assert_eq!(disk.source_checksum(), checksum);
    assert!(!disk.dirty(&project));
    assert_eq!(other.observed, saved);
    other.finish_document();
    assert_eq!(Pair::read(&dir).unwrap(), saved);
    other.discard().unwrap();
    assert!(Pair::read(&dir).unwrap().empty());
    project.main_circuit_mut().name = "edited".into();
    owner.update(
        now + Duration::from_secs(600),
        FileSettings::default(),
        true,
        &Storage::default(),
        &project,
    );
    assert!(owner.view().owned);
    assert_eq!(Pair::read(&dir).unwrap().project(&dir).unwrap(), project);
    other.refresh();
    drop(owner);
    assert_eq!(other.recover(&mut disk).unwrap(), project);
    assert!(other.view().owned && other.error.is_none());
    assert_eq!(other.written.as_ref(), Some(&project));
}

#[test]
fn failed_clear_keeps_ownership_until_verified_replacement() {
    let temp = Temp::new();
    let dir = temp.path("recovery");
    let mut owner = Autosave::at(Some(dir.clone()));
    let project = Project::default();
    write_checkpoint(&mut owner, &project);
    let lock = slot::try_lock(&dir, slot::LOCK).unwrap().unwrap();
    let before = Pair::read(&dir).unwrap();
    assert!(owner.clear_owned().is_err());
    assert!(owner.view().owned);
    assert_eq!(Pair::read(&dir).unwrap(), before);
    drop(lock);
    owner.clear_owned().unwrap();
    assert!(!owner.view().owned && !owner.pending());

    write_checkpoint(&mut owner, &project);
    let meta = dir.join(slot::META);
    let mut newer = fs::read(&meta).unwrap();
    newer.push(b' ');
    fs::write(&meta, &newer).unwrap();
    assert!(owner.clear_owned().is_err());
    assert!(!owner.view().owned);
    assert_eq!(fs::read(&meta).unwrap(), newer);
    assert!(owner.clear_from_settings().is_err());
}

#[test]
fn explicitly_clearing_damage_checks_the_observed_pair() {
    let temp = Temp::new();
    let dir = temp.path("recovery");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join(slot::DATA), b"damaged").unwrap();
    let mut auto = Autosave::at(Some(dir.clone()));
    assert!(!auto.view().owned);
    assert!(auto.clear_owned().is_err());
    fs::write(dir.join(slot::DATA), b"different").unwrap();
    assert!(auto.clear_from_settings().is_err());
    assert_eq!(fs::read(dir.join(slot::DATA)).unwrap(), b"different");
    auto.clear_from_settings().unwrap();
    assert!(Pair::read(&dir).unwrap().empty());
}

#[test]
fn recovery_files_and_both_locks_are_reserved_project_destinations() {
    let temp = Temp::new();
    let dir = temp.path("recovery");
    let auto = Autosave::at(Some(dir.clone()));
    for path in [
        dir.join(slot::DATA),
        dir.join(slot::META),
        temp.path(slot::LOCK),
        temp.path(slot::OWNER),
    ] {
        assert!(auto.reserved(&path));
    }
    assert!(!auto.reserved(&temp.path("project.circ")));
}

#[test]
fn unchanged_checkpoints_keep_both_mtimes_and_failed_edits_retry() {
    let tmp = Temp::new();
    let dir = tmp.path("recovery");
    let mut owner = Autosave::at(Some(dir.clone()));
    let mut project = Project::default();
    let now = write_checkpoint(&mut owner, &project);
    let at = |s| now + Duration::from_secs(s);
    let tick = |auto: &mut Autosave, s, project: &Project| {
        auto.update(
            at(s),
            FileSettings::default(),
            true,
            &Storage::default(),
            project,
        )
    };
    let mtimes =
        || [slot::DATA, slot::META].map(|f| fs::metadata(dir.join(f)).unwrap().modified().unwrap());
    let old = mtimes();
    for s in [600, 900] {
        tick(&mut owner, s, &project);
        assert_eq!(owner.last_success, Some(at(300)));
        assert_eq!(owner.next_due, Some(at(s + 300)));
        assert_eq!(mtimes(), old);
    }
    let before = project.clone();
    project.main_circuit_mut().name = "next".into();
    let lock = slot::try_lock(&dir, slot::LOCK).unwrap().unwrap();
    tick(&mut owner, 1200, &project);
    assert_eq!(owner.written.as_ref(), Some(&before));
    drop(lock);
    tick(&mut owner, 1499, &project);
    assert_eq!(mtimes(), old);
    tick(&mut owner, 1500, &project);
    assert_eq!(owner.written.as_ref(), Some(&project));
    assert_eq!(Pair::read(&dir).unwrap().project(&dir).unwrap(), project);
    drop(owner);
    let mut restored = Autosave::at(Some(dir.clone()));
    let recovered = restored.recover(&mut Storage::default()).unwrap();
    let old = mtimes();
    tick(&mut restored, 1500, &recovered);
    tick(&mut restored, 1800, &recovered);
    assert_eq!(mtimes(), old);
    assert!(restored.last_success.is_none());
    restored.clear_owned().unwrap();
    assert!(restored.written.is_none());
    write_checkpoint(&mut restored, &recovered);
    restored.finish_document();
    assert!(restored.written.is_none() && !restored.pending());
}

#[test]
fn invalid_minutes_report_once_and_suspend_writes_until_corrected() {
    let temp = Temp::new();
    let dir = temp.path("recovery");
    let mut auto = Autosave::at(Some(dir.clone()));
    let now = Instant::now();
    for minutes in [0, 31, 255] {
        let options = FileSettings {
            autosave: true,
            autosave_minutes: minutes,
        };
        for (frame, seconds) in [0, 1, 12000].into_iter().enumerate() {
            auto.changed = false;
            auto.update(
                now + Duration::from_secs(seconds),
                options,
                true,
                &Storage::default(),
                &Project::default(),
            );
            assert_eq!(auto.error.as_deref(), Some(INVALID_INTERVAL));
            assert_eq!(auto.changed, frame == 0);
            assert!(auto.next_due.is_none() && !dir.exists());
        }
    }
    write_checkpoint(&mut auto, &Project::default());
    assert!(auto.error.is_none());
}

#[test]
fn damaged_clear_rejects_the_live_writer_then_releases_its_temporary_lease() {
    let tmp = Temp::new();
    let dir = tmp.path("recovery");
    let mut owner = Autosave::at(Some(dir.clone()));
    let mut project = Project::default();
    let at = write_checkpoint(&mut owner, &project);
    let meta = dir.join(slot::META);
    let original = fs::read(&meta).unwrap();
    fs::write(&meta, b"damaged").unwrap();
    let mut observer = Autosave::at(Some(dir.clone()));
    assert_eq!(observer.view().state, logisim_ui::SnapshotState::Damaged);
    let damaged = Pair::read(&dir).unwrap();
    assert_eq!(
        observer.clear_from_settings().unwrap_err(),
        "Recovery copy is owned by another window."
    );
    assert_eq!(Pair::read(&dir).unwrap(), damaged);
    fs::write(&meta, original).unwrap();
    project.main_circuit_mut().name = "owner edit".into();
    owner.update(
        at + Duration::from_secs(600),
        FileSettings::default(),
        true,
        &Storage::default(),
        &project,
    );
    assert_eq!(Pair::read(&dir).unwrap().project(&dir).unwrap(), project);
    drop(owner);
    assert!(observer.clear_from_settings().is_err()); // Repaired since confirmation.
    assert_eq!(Pair::read(&dir).unwrap().project(&dir).unwrap(), project);
    fs::write(&meta, b"damaged again").unwrap();
    observer.refresh();
    observer.clear_from_settings().unwrap();
    assert!(Pair::read(&dir).unwrap().empty());
    assert!(slot::owner(&dir).is_ok());
}

#[test]
fn unavailable_directory_with_cached_metadata_can_be_cleared_after_access_returns() {
    let tmp = Temp::new();
    let dir = tmp.path("recovery");
    let hidden = tmp.path("temporarily-unmounted");
    let mut owner = Autosave::at(Some(dir.clone()));
    write_checkpoint(&mut owner, &Project::default());
    let mut observer = Autosave::at(Some(dir.clone()));
    fs::rename(&dir, &hidden).unwrap();
    fs::write(&dir, b"unavailable").unwrap();
    observer.refresh();
    assert!(observer.metadata.is_some());
    assert_eq!(
        observer.view().state,
        logisim_ui::SnapshotState::Unavailable {
            snapshot_present: true
        }
    );
    fs::remove_file(&dir).unwrap();
    fs::rename(&hidden, &dir).unwrap();
    let before = Pair::read(&dir).unwrap();
    assert_eq!(
        observer.clear_from_settings().unwrap_err(),
        "Recovery copy is owned by another window."
    );
    assert_eq!(Pair::read(&dir).unwrap(), before);
    drop(owner);
    observer.clear_from_settings().unwrap();
    assert!(Pair::read(&dir).unwrap().empty());
    assert!(!observer.view().owned && observer.error.is_none());
}

#[cfg(unix)]
#[test]
fn unreadable_data_can_be_cleared_without_rereading_its_contents() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = Temp::new();
    let dir = tmp.path("recovery");
    let mut owner = Autosave::at(Some(dir.clone()));
    write_checkpoint(&mut owner, &Project::default());
    let mut observer = Autosave::at(Some(dir.clone()));
    let data = dir.join(slot::DATA);
    fs::set_permissions(&data, fs::Permissions::from_mode(0)).unwrap();
    if fs::read(&data).is_ok() {
        // A privileged test process bypasses Unix read permissions.
        return;
    }
    observer.refresh();
    assert!(observer.metadata.is_some());
    assert_eq!(
        observer.view().state,
        logisim_ui::SnapshotState::Unavailable {
            snapshot_present: true
        }
    );
    assert_eq!(
        observer.clear_from_settings().unwrap_err(),
        "Recovery copy is owned by another window."
    );
    let saved = Pair::read(&dir).unwrap();
    drop(owner);
    assert_eq!(Pair::read(&dir).unwrap(), saved);
    observer.clear_from_settings().unwrap();
    assert!(Pair::read(&dir).unwrap().empty());
}
