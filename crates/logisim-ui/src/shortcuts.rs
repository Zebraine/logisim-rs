//! Logical, portable bindings for the workspace command set.
use egui::{Key, Modifiers};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeMap};
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    File,
    Edit,
    Project,
    Tools,
    Simulation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Global,
    Canvas,
}
impl Group {
    pub const ALL: [Self; 5] = [
        Self::File,
        Self::Edit,
        Self::Project,
        Self::Tools,
        Self::Simulation,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Edit => "Edit",
            Self::Project => "Project",
            Self::Tools => "Tools",
            Self::Simulation => "Simulation",
        }
    }
}

macro_rules! commands {
    ($($variant:ident, $id:literal, $label:literal, $group:ident, $mods:expr, $key:ident;)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Command { $($variant,)* }
        impl Command {
            pub const ALL: [Self; 26] = [$(Self::$variant,)*];
            pub const COUNT: usize = Self::ALL.len();
            pub const fn id(self) -> &'static str { match self { $(Self::$variant => $id,)* } }
            pub const fn label(self) -> &'static str { match self { $(Self::$variant => $label,)* } }
            pub const fn group(self) -> Group { match self { $(Self::$variant => Group::$group,)* } }
            pub const fn default_chord(self) -> Chord { match self { $(Self::$variant => Chord { mods: $mods, key: Key::$key },)* } }
        }
    }
}
const PRIMARY: Modifiers = Modifiers::COMMAND;
const PRIMARY_SHIFT: Modifiers = Modifiers::COMMAND.plus(Modifiers::SHIFT);
commands! {
    New, "file.new", "New", File, PRIMARY, N;
    Open, "file.open", "Open…", File, PRIMARY, O;
    Save, "file.save", "Save", File, PRIMARY, S;
    SaveAs, "file.save_as", "Save As…", File, PRIMARY_SHIFT, S;
    Exit, "file.exit", "Exit", File, PRIMARY, Q;
    Undo, "edit.undo", "Undo", Edit, PRIMARY, Z;
    Redo, "edit.redo", "Redo", Edit, PRIMARY_SHIFT, Z;
    Cut, "edit.cut", "Cut", Edit, PRIMARY, X;
    Copy, "edit.copy", "Copy", Edit, PRIMARY, C;
    Paste, "edit.paste", "Paste", Edit, PRIMARY, V;
    Duplicate, "edit.duplicate", "Duplicate", Edit, PRIMARY, D;
    SelectAll, "edit.select_all", "Select All", Edit, PRIMARY, A;
    AddCircuit, "project.add_circuit", "Add Circuit…", Project, PRIMARY_SHIFT, N;
    ToggleAppearance, "project.toggle_appearance", "Edit Circuit Appearance", Project, PRIMARY_SHIFT, E;
    ReturnParent, "project.return_parent", "Return to Parent", Project, Modifiers::ALT, ArrowLeft;
    ToolSelect, "tool.select", "Select", Tools, Modifiers::NONE, V;
    ToolPoke, "tool.poke", "Poke", Tools, Modifiers::NONE, P;
    ToolWire, "tool.wire", "Wire", Tools, Modifiers::NONE, W;
    ToolText, "tool.text", "Text", Tools, Modifiers::NONE, T;
    ToolTunnel, "tool.tunnel", "Tunnel", Tools, Modifiers::NONE, U;
    ToolRotate, "tool.rotate", "Rotate", Tools, Modifiers::NONE, R;
    Toggle, "simulation.toggle", "Enable / pause simulation", Simulation, PRIMARY, E;
    Step, "simulation.step", "Step Simulation", Simulation, PRIMARY, I;
    Reset, "simulation.reset", "Reset Simulation", Simulation, PRIMARY, R;
    Tick, "simulation.tick", "Tick once", Simulation, PRIMARY, T;
    AutomaticTicks, "simulation.automatic_ticks", "Automatic ticks", Simulation, PRIMARY, K;
}
impl Command {
    pub fn scope(self) -> Scope {
        match self.group() {
            Group::File | Group::Project => Scope::Global,
            Group::Edit | Group::Tools | Group::Simulation => Scope::Canvas,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    mods: Modifiers,
    key: Key,
}
impl Chord {
    pub(crate) fn is_modifier(key: Key) -> bool {
        matches!(
            key,
            Key::ShiftLeft
                | Key::ShiftRight
                | Key::ControlLeft
                | Key::ControlRight
                | Key::AltLeft
                | Key::AltRight
                | Key::SuperLeft
                | Key::SuperRight
                | Key::Fn
        )
    }

    pub(crate) fn consume_primary(ui: &egui::Ui, key: Key) -> bool {
        let mac = ui.ctx().os().is_mac();
        let wanted = Self { key, mods: PRIMARY };
        ui.input_mut(|i| {
            let count = i.events.len();
            i.events.retain(|event| match event {
                egui::Event::Key {
                    key,
                    modifiers,
                    pressed: true,
                    ..
                } => !wanted.matches(Self::from_event(*key, *modifiers, mac), mac),
                _ => true,
            });
            i.events.len() != count
        })
    }
    /// egui sets both ctrl and command for the same key on Windows and Linux.
    pub fn from_event(key: Key, modifiers: Modifiers, mac: bool) -> Self {
        Self {
            key,
            mods: Modifiers {
                command: modifiers.command || modifiers.mac_cmd || (!mac && modifiers.ctrl),
                ctrl: mac && modifiers.ctrl,
                shift: modifiers.shift,
                alt: modifiers.alt,
                mac_cmd: false,
            },
        }
    }
    pub fn matches(self, other: Self, mac: bool) -> bool {
        self.key == other.key
            && Self::from_event(self.key, self.mods, mac)
                == Self::from_event(other.key, other.mods, mac)
    }
    pub fn label(self, mac: bool) -> String {
        let m = Self::from_event(self.key, self.mods, mac).mods;
        let mut out = String::new();
        if m.command {
            out.push_str(if mac { "Cmd+" } else { "Ctrl+" });
        }
        if m.shift {
            out.push_str("Shift+");
        }
        if m.alt {
            out.push_str("Alt+");
        }
        if m.ctrl {
            out.push_str("Ctrl+");
        }
        out.push_str(self.key.name());
        out
    }
    fn fixed(self, command: Command) -> bool {
        // If two logical chords coincide on either supported platform, a
        // portable configuration must not distinguish them.
        let m = Self::from_event(self.key, self.mods, false).mods;
        use Key::*;
        match self.key {
            key if Self::is_modifier(key) => true,
            Escape | Enter | Backspace | Delete | Tab | Copy | Cut | Paste => true,
            Space | Num0 | Num1 | ArrowLeft | ArrowRight | ArrowUp | ArrowDown if m.is_none() => {
                true
            }
            // Windows translates these physical combinations into native clipboard events.
            Insert if m.command || m.shift => true,
            F | Num0 | Y if m == PRIMARY => true,
            ArrowUp | ArrowDown if m == PRIMARY || m == PRIMARY_SHIFT => true,
            // The appearance editor keeps its original editing shortcuts.
            _ => Command::ALL.into_iter().any(|c| {
                c.group() == Group::Edit && c != command && self.matches(c.default_chord(), false)
            }),
        }
    }
}
impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (enabled, name) in [
            (self.mods.command, "primary"),
            (self.mods.shift, "shift"),
            (self.mods.alt, "alt"),
            (self.mods.ctrl, "ctrl"),
        ] {
            if enabled {
                write!(f, "{name}+")?;
            }
        }
        f.write_str(&self.key.name().to_ascii_lowercase())
    }
}
impl FromStr for Chord {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut rest = value;
        let mut mods = Modifiers::NONE;
        for (name, bit) in [
            ("primary+", &mut mods.command),
            ("shift+", &mut mods.shift),
            ("alt+", &mut mods.alt),
            ("ctrl+", &mut mods.ctrl),
        ] {
            if let Some(s) = rest.strip_prefix(name) {
                *bit = true;
                rest = s;
            }
        }
        if rest.contains('+') || rest.trim() != rest {
            return Err("Invalid shortcut chord.");
        }
        let key = Key::ALL
            .iter()
            .copied()
            .find(|k| k.name().eq_ignore_ascii_case(rest))
            .or_else(|| Key::from_name(rest))
            .ok_or("Invalid shortcut chord.")?;
        if Self::is_modifier(key) {
            return Err("A shortcut requires a non-modifier key.");
        }
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
    pub(crate) fn command(&self, chord: Chord, mac: bool) -> Option<Command> {
        let legacy_redo = Chord {
            key: Key::Y,
            mods: PRIMARY,
        };
        if legacy_redo.matches(chord, mac) {
            return Some(Command::Redo);
        }
        Command::ALL
            .into_iter()
            .find(|c| self.get(*c).matches(chord, mac))
    }
    pub(crate) fn caption(&self, c: Command, ui: &egui::Ui) -> String {
        self.caption_with(c, c.label(), ui)
    }
    pub(crate) fn caption_with(&self, c: Command, label: &str, ui: &egui::Ui) -> String {
        format!(
            "{} · {}",
            crate::i18n::tr(label),
            self.get(c).label(ui.ctx().os().is_mac())
        )
    }
    pub fn get(&self, c: Command) -> Chord {
        self.0[c as usize]
    }
    fn validate(&self, c: Command, chord: Chord) -> Result<(), Conflict> {
        if let Some(other) = Command::ALL
            .into_iter()
            .find(|other| *other != c && self.get(*other).matches(chord, false))
        {
            return Err(Conflict::Command(other));
        }
        if chord.fixed(c) {
            return Err(Conflict::Fixed);
        }
        Ok(())
    }
    pub(crate) fn with_changes(mut self, before: Self, after: Self) -> Result<Self, Conflict> {
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

#[cfg(test)]
mod tests;
