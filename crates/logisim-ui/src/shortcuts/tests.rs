use super::*;

#[test]
fn default_chords_match_the_pre_registry_keyboard_contract() {
    // Key by the persisted command ID so presentation order cannot change this contract.
    let mut baseline = std::collections::BTreeMap::from([
        ("file.new", "primary+n"),
        ("file.open", "primary+o"),
        ("file.save", "primary+s"),
        ("file.save_as", "primary+shift+s"),
        ("file.exit", "primary+q"),
        ("edit.undo", "primary+z"),
        ("edit.redo", "primary+shift+z"),
        ("edit.cut", "primary+x"),
        ("edit.copy", "primary+c"),
        ("edit.paste", "primary+v"),
        ("edit.duplicate", "primary+d"),
        ("edit.select_all", "primary+a"),
        ("project.add_circuit", "primary+shift+n"),
        ("project.toggle_appearance", "primary+shift+e"),
        ("project.return_parent", "alt+left"),
        ("tool.select", "v"),
        ("tool.poke", "p"),
        ("tool.wire", "w"),
        ("tool.text", "t"),
        ("tool.tunnel", "u"),
        ("tool.rotate", "r"),
        ("simulation.toggle", "primary+e"),
        ("simulation.step", "primary+i"),
        ("simulation.reset", "primary+r"),
        ("simulation.tick", "primary+t"),
        ("simulation.automatic_ticks", "primary+k"),
        ("appearance.undo", "primary+z"),
        ("appearance.redo", "primary+shift+z"),
        ("appearance.cut", "primary+x"),
        ("appearance.copy", "primary+c"),
        ("appearance.paste", "primary+v"),
        ("appearance.duplicate", "primary+d"),
        ("appearance.select_all", "primary+a"),
        ("appearance.raise", "primary+up"),
        ("appearance.lower", "primary+down"),
        ("appearance.front", "primary+shift+up"),
        ("appearance.back", "primary+shift+down"),
    ]);
    let bindings = Shortcuts::default();
    for command in Command::ALL {
        let id = command.id();
        let actual = bindings.get(command).to_string();
        let expected = baseline
            .remove(id)
            .unwrap_or_else(|| panic!("No baseline for {id} = {actual}"));
        assert_eq!(actual, expected, "Expected {id} = {expected}, got {actual}");
    }
    assert!(
        baseline.is_empty(),
        "Commands missing from the registry: {baseline:?}"
    );
}

#[test]
fn layer_keys_are_available_outside_their_scope_and_when_rebound() {
    for (command, arrow) in [
        (Command::AppearanceFront, "up"),
        (Command::AppearanceBack, "down"),
    ] {
        let mut bindings = Shortcuts::default();
        let shift = format!("shift+{arrow}").parse().unwrap();
        let primary_shift = format!("primary+shift+{arrow}").parse().unwrap();
        for mac in [false, true] {
            assert_eq!(
                bindings.command(primary_shift, Scope::Appearance, mac),
                Some(command)
            );
            assert_eq!(bindings.command(primary_shift, Scope::Canvas, mac), None);
            assert_eq!(bindings.command(shift, Scope::Appearance, mac), None);
            assert_eq!(bindings.command(shift, Scope::Canvas, mac), None);
        }
        bindings.bind(Command::Save, shift).unwrap();
        assert_eq!(
            bindings.bind(Command::Save, primary_shift),
            Err(Conflict::Command(command))
        );
        bindings.bind(command, "f5".parse().unwrap()).unwrap();
        bindings.bind(Command::Save, primary_shift).unwrap();
    }
}

#[test]
fn shared_chords_resolve_in_the_active_editor_and_global_conflicts_are_symmetric() {
    use Command::*;
    let mut bindings = Shortcuts::default();
    let key = |key: &str| key.parse().unwrap();
    for mac in [false, true] {
        assert_eq!(
            bindings.command(key("primary+c"), Scope::Canvas, mac),
            Some(Copy)
        );
        assert_eq!(
            bindings.command(key("primary+c"), Scope::Appearance, mac),
            Some(AppearanceCopy)
        );
        assert_eq!(
            bindings.command(key("primary+y"), Scope::Canvas, mac),
            Some(Redo)
        );
        assert_eq!(
            bindings.command(key("primary+y"), Scope::Appearance, mac),
            None
        );
    }
    bindings.bind(AppearanceCopy, key("f5")).unwrap();
    bindings.bind(Copy, key("f5")).unwrap();
    assert!(matches!(
        bindings.bind(Save, key("f5")),
        Err(Conflict::Command(_))
    ));
    assert_eq!(
        bindings.bind(AppearanceCopy, key("primary+s")),
        Err(Conflict::Command(Save))
    );
    assert_eq!(
        bindings.bind(AppearancePaste, key("f5")),
        Err(Conflict::Command(AppearanceCopy))
    );
    bindings
        .bind(AppearanceCopy, key("primary+up"))
        .unwrap_err();
    bindings.bind(ToolWire, key("primary+up")).unwrap();
    bindings.bind(New, key("primary+c")).unwrap();
    assert_eq!(
        bindings.bind(Copy, key("primary+c")),
        Err(Conflict::Command(New))
    );
    let mut local = Shortcuts::default();
    local.bind(AppearanceCopy, key("f9")).unwrap();
    let mut disk = Shortcuts::default();
    disk.bind(Copy, key("f9")).unwrap();
    let merged = disk.with_changes(Shortcuts::default(), local).unwrap();
    assert_eq!(merged.get(AppearanceCopy), merged.get(Copy));
    disk.bind(Save, key("f8")).unwrap();
    local.bind(AppearanceCopy, key("f8")).unwrap();
    assert_eq!(
        disk.with_changes(Shortcuts::default(), local),
        Err(Conflict::Command(AppearanceCopy))
    );
}

#[test]
fn tool_keys_can_be_reassigned_but_fixed_editing_chords_stay_reserved() {
    for command in Command::ALL
        .into_iter()
        .filter(|c| c.group() == Group::Tools)
    {
        let mut bindings = Shortcuts::default();
        let original = bindings.get(command);
        assert_eq!(command.scope(), Scope::Canvas);
        assert_eq!(
            bindings.bind(Command::Save, original),
            Err(Conflict::Command(command))
        );
        bindings.bind(command, "f10".parse().unwrap()).unwrap();
        bindings.bind(Command::Save, original).unwrap();
    }

    let mut bindings = Shortcuts::default();
    bindings.bind(Command::Copy, "f5".parse().unwrap()).unwrap();
    for chord in [
        "delete",
        "backspace",
        "space",
        "up",
        "down",
        "left",
        "right",
        "primary+f",
        "primary+0",
        "primary+y",
        "shift+insert",
        "primary+insert",
    ] {
        assert_eq!(
            bindings.bind(Command::ToolPoke, chord.parse().unwrap()),
            Err(Conflict::Fixed),
            "{chord}"
        );
    }
}

#[test]
fn persisted_key_names_round_trip_every_desktop_key() {
    for &key in Key::ALL {
        let chord = Chord::from_event(key, Modifiers::NONE, false);
        let parsed = chord.to_string().parse::<Chord>();
        if Chord::is_modifier(key) {
            assert!(parsed.is_err());
            assert_eq!(
                Shortcuts::default().bind(Command::Save, chord),
                Err(Conflict::Fixed)
            );
        } else {
            assert_eq!(parsed, Ok(chord), "{key:?}");
        }
    }
}
