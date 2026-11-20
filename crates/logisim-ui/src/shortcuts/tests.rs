use super::*;

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
