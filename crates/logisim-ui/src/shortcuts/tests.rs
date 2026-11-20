use super::*;

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
        "primary+c",
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
        "primary+up",
        "primary+shift+down",
    ] {
        assert_eq!(
            bindings.bind(Command::ToolPoke, chord.parse().unwrap()),
            Err(Conflict::Fixed),
            "{chord}"
        );
    }
}
