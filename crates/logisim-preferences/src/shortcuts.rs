//! Logical, portable bindings for the workspace command set.
mod keys;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeMap};
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    File,
    Edit,
    Project,
    Tools,
    Simulation,
    Appearance,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Global,
    Canvas,
    Appearance,
}
impl Scope {
    pub fn overlaps(self, other: Self) -> bool {
        self == other || self == Self::Global || other == Self::Global
    }
}
impl Group {
    pub const ALL: [Self; 6] = [
        Self::File,
        Self::Edit,
        Self::Project,
        Self::Tools,
        Self::Simulation,
        Self::Appearance,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Edit => "Edit",
            Self::Project => "Project",
            Self::Tools => "Tools",
            Self::Simulation => "Simulation",
            Self::Appearance => "Appearance",
        }
    }
}

macro_rules! commands {
    ($($variant:ident, $id:literal, $label:literal, $group:ident, $mods:expr, $key:literal;)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Command { $($variant,)* }
        impl Command {
            pub const ALL: [Self; 37] = [$(Self::$variant,)*];
            pub const COUNT: usize = Self::ALL.len();
            pub const fn id(self) -> &'static str { match self { $(Self::$variant => $id,)* } }
            pub const fn label(self) -> &'static str { match self { $(Self::$variant => $label,)* } }
            pub const fn group(self) -> Group { match self { $(Self::$variant => Group::$group,)* } }
            pub const fn default_chord(self) -> Chord { match self { $(Self::$variant => Chord { key: $key, mods: $mods },)* } }
        }
    }
}
const PRIMARY: u8 = 1;
const SHIFT: u8 = 2;
const ALT: u8 = 4;
const CTRL: u8 = 8;
const PRIMARY_SHIFT: u8 = PRIMARY | SHIFT;
commands! {
    New, "file.new", "New", File, PRIMARY, "N";
    Open, "file.open", "Open…", File, PRIMARY, "O";
    Save, "file.save", "Save", File, PRIMARY, "S";
    SaveAs, "file.save_as", "Save As…", File, PRIMARY_SHIFT, "S";
    Exit, "file.exit", "Exit", File, PRIMARY, "Q";
    Undo, "edit.undo", "Undo", Edit, PRIMARY, "Z";
    Redo, "edit.redo", "Redo", Edit, PRIMARY_SHIFT, "Z";
    Cut, "edit.cut", "Cut", Edit, PRIMARY, "X";
    Copy, "edit.copy", "Copy", Edit, PRIMARY, "C";
    Paste, "edit.paste", "Paste", Edit, PRIMARY, "V";
    Duplicate, "edit.duplicate", "Duplicate", Edit, PRIMARY, "D";
    SelectAll, "edit.select_all", "Select All", Edit, PRIMARY, "A";
    AddCircuit, "project.add_circuit", "Add Circuit…", Project, PRIMARY_SHIFT, "N";
    ToggleAppearance, "project.toggle_appearance", "Edit Circuit Appearance", Project, PRIMARY_SHIFT, "E";
    ReturnParent, "project.return_parent", "Return to Parent", Project, ALT, "Left";
    ToolSelect, "tool.select", "Select", Tools, 0, "V";
    ToolPoke, "tool.poke", "Poke", Tools, 0, "P";
    ToolWire, "tool.wire", "Wire", Tools, 0, "W";
    ToolText, "tool.text", "Text", Tools, 0, "T";
    ToolTunnel, "tool.tunnel", "Tunnel", Tools, 0, "U";
    ToolRotate, "tool.rotate", "Rotate", Tools, 0, "R";
    Toggle, "simulation.toggle", "Enable / pause simulation", Simulation, PRIMARY, "E";
    Step, "simulation.step", "Step Simulation", Simulation, PRIMARY, "I";
    Reset, "simulation.reset", "Reset Simulation", Simulation, PRIMARY, "R";
    Tick, "simulation.tick", "Tick once", Simulation, PRIMARY, "T";
    AutomaticTicks, "simulation.automatic_ticks", "Automatic ticks", Simulation, PRIMARY, "K";
    AppearanceUndo, "appearance.undo", "Undo", Appearance, PRIMARY, "Z";
    AppearanceRedo, "appearance.redo", "Redo", Appearance, PRIMARY_SHIFT, "Z";
    AppearanceCut, "appearance.cut", "Cut", Appearance, PRIMARY, "X";
    AppearanceCopy, "appearance.copy", "Copy", Appearance, PRIMARY, "C";
    AppearancePaste, "appearance.paste", "Paste", Appearance, PRIMARY, "V";
    AppearanceDuplicate, "appearance.duplicate", "Duplicate", Appearance, PRIMARY, "D";
    AppearanceSelectAll, "appearance.select_all", "Select All", Appearance, PRIMARY, "A";
    AppearanceRaise, "appearance.raise", "Raise", Appearance, PRIMARY, "Up";
    AppearanceLower, "appearance.lower", "Lower", Appearance, PRIMARY, "Down";
    AppearanceFront, "appearance.front", "Bring to Front", Appearance, PRIMARY_SHIFT, "Up";
    AppearanceBack, "appearance.back", "Send to Back", Appearance, PRIMARY_SHIFT, "Down";
}
impl Command {
    pub fn scope(self) -> Scope {
        match self.group() {
            Group::File | Group::Project => Scope::Global,
            Group::Appearance => Scope::Appearance,
            Group::Edit | Group::Tools | Group::Simulation => Scope::Canvas,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    mods: u8,
    key: &'static str,
}
impl Chord {
    pub fn from_key(key: &'static str, primary: bool, shift: bool, alt: bool, ctrl: bool) -> Self {
        Self {
            key,
            mods: u8::from(primary)
                | (u8::from(shift) << 1)
                | (u8::from(alt) << 2)
                | (u8::from(ctrl) << 3),
        }
    }
    fn modifiers(self, mac: bool) -> u8 {
        if !mac && self.mods & CTRL != 0 {
            (self.mods & !CTRL) | PRIMARY
        } else {
            self.mods
        }
    }
    pub fn matches(self, other: Self, mac: bool) -> bool {
        self.key == other.key && self.modifiers(mac) == other.modifiers(mac)
    }
    pub fn label(self, mac: bool) -> String {
        let mods = self.modifiers(mac);
        let mut out = String::new();
        for (mask, label) in [
            (PRIMARY, if mac { "Cmd+" } else { "Ctrl+" }),
            (SHIFT, "Shift+"),
            (ALT, "Alt+"),
            (CTRL, "Ctrl+"),
        ] {
            if mods & mask != 0 {
                out.push_str(label);
            }
        }
        out.push_str(self.key);
        out
    }
    fn fixed(self) -> bool {
        let m = self.modifiers(false);
        match self.key {
            "Escape" | "Enter" | "Backspace" | "Delete" | "Tab" | "Copy" | "Cut" | "Paste" => true,
            "Space" | "0" | "1" | "Left" | "Right" | "Up" | "Down" if m == 0 => true,
            "Insert" => m & (PRIMARY | SHIFT) != 0,
            "F" | "0" | "Y" if m == PRIMARY => true,
            // Capture can produce modifier-only keys; only keys in the portable vocabulary can bind.
            _ => keys::canonical(self.key).is_none(),
        }
    }
}
impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (enabled, name) in [
            (self.mods & PRIMARY != 0, "primary"),
            (self.mods & SHIFT != 0, "shift"),
            (self.mods & ALT != 0, "alt"),
            (self.mods & CTRL != 0, "ctrl"),
        ] {
            if enabled {
                write!(f, "{name}+")?;
            }
        }
        f.write_str(&self.key.to_ascii_lowercase())
    }
}
impl FromStr for Chord {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut rest = value;
        let mut mods = 0;
        for (name, bit) in [
            ("primary+", PRIMARY),
            ("shift+", SHIFT),
            ("alt+", ALT),
            ("ctrl+", CTRL),
        ] {
            if let Some(s) = rest.strip_prefix(name) {
                mods |= bit;
                rest = s;
            }
        }
        if rest.contains('+') || rest.trim() != rest {
            return Err("Invalid shortcut chord.");
        }
        let key = keys::canonical(rest).ok_or("Invalid shortcut chord.")?;
        Ok(Self { mods, key })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conflict {
    Command(Command),
    Fixed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcuts([Chord; Command::COUNT]);
impl Default for Shortcuts {
    fn default() -> Self {
        Self(Command::ALL.map(Command::default_chord))
    }
}
impl Shortcuts {
    pub fn command(&self, chord: Chord, scope: Scope, mac: bool) -> Option<Command> {
        let legacy_redo = Chord {
            key: "Y",
            mods: PRIMARY,
        };
        if scope == Scope::Canvas && legacy_redo.matches(chord, mac) {
            return Some(Command::Redo);
        }
        Command::ALL
            .into_iter()
            .find(|c| c.scope().overlaps(scope) && self.get(*c).matches(chord, mac))
    }
    pub fn get(&self, c: Command) -> Chord {
        self.0[c as usize]
    }
    fn validate(&self, c: Command, chord: Chord) -> Result<(), Conflict> {
        if let Some(other) = Command::ALL.into_iter().find(|other| {
            *other != c
                && c.scope().overlaps(other.scope())
                && self.get(*other).matches(chord, false)
        }) {
            return Err(Conflict::Command(other));
        }
        if chord.fixed() {
            return Err(Conflict::Fixed);
        }
        Ok(())
    }
    pub fn with_changes(mut self, before: Self, after: Self) -> Result<Self, Conflict> {
        for command in Command::ALL {
            if before.get(command) != after.get(command) {
                self.0[command as usize] = after.get(command);
            }
        }
        // Validate the finished map so a valid swap can be replayed atomically.
        for command in Command::ALL {
            self.validate(command, self.get(command))?;
        }
        Ok(self)
    }
    pub fn bind(&mut self, c: Command, chord: Chord) -> Result<(), Conflict> {
        self.validate(c, chord)?;
        self.0[c as usize] = chord;
        Ok(())
    }
}
impl Serialize for Shortcuts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(Command::COUNT))?;
        for c in Command::ALL {
            map.serialize_entry(c.id(), &self.get(c).to_string())?;
        }
        map.end()
    }
}
impl<'de> Deserialize<'de> for Shortcuts {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Bindings;
        impl<'de> de::Visitor<'de> for Bindings {
            type Value = Shortcuts;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map of workspace commands to shortcut chords")
            }
            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut result = Shortcuts::default();
                let mut seen = [false; Command::COUNT];
                while let Some((id, value)) = map.next_entry::<String, String>()? {
                    let c = Command::ALL
                        .into_iter()
                        .find(|c| c.id() == id)
                        .ok_or_else(|| de::Error::custom("Unknown shortcut command."))?;
                    if std::mem::replace(&mut seen[c as usize], true) {
                        return Err(de::Error::custom("Duplicate shortcut command."));
                    }
                    result.0[c as usize] = value.parse().map_err(de::Error::custom)?;
                }
                for c in Command::ALL {
                    result
                        .validate(c, result.get(c))
                        .map_err(|_| de::Error::custom("Shortcut conflict."))?;
                }
                Ok(result)
            }
        }
        deserializer.deserialize_map(Bindings)
    }
}
