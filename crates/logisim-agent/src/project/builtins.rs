//! The patch catalog maps public keys directly to persistent core attributes.
//! After typed decoding, the file codec validates the complete prospective component.
use super::edit::{choice, decode};
use crate::Result;
use logisim_core::{
    Arithmetic, Component, Extension, Facing, HorizontalAlign, Kind, MatrixInput, Plexer, Pull,
    Shift, VerticalAlign,
};
use serde_json::{Value, json};

pub(super) struct Field {
    pub name: &'static str,
    pub read: fn(&Component) -> Value,
    pub write: fn(&mut Component, &Value) -> Result<()>,
}

// Using the same member for reading and writing keeps the snapshot and edit vocabulary paired.
macro_rules! field {
    ($id:ident, $key:literal, $($member:ident).+) => {
        const $id: Field = Field {
            name: $key,
            read: |c| json!(c.$($member).+),
            write: |c, value| {
                c.$($member).+ = decode($key, value)?;
                Ok(())
            },
        };
    };
}

macro_rules! enum_field {
    ($id:ident, $key:literal, $($member:ident).+, [$($variant:path => $name:literal),+]) => {
        const $id: Field = Field {
            name: $key,
            read: |c| match c.$($member).+ { $($variant => $name.into()),+ },
            write: |c, value| {
                c.$($member).+ = choice($key, value, [$(($variant, $name)),+])?;
                Ok(())
            },
        };
    };
}

field!(BITS, "bits", bits);
field!(VALUE, "value", constant);
field!(HIGH, "high_ticks", sequential.high_ticks);
field!(LOW, "low_ticks", sequential.low_ticks);
field!(TEXT, "text", label);
field!(IN_WIDTH, "in_width", input_bits);
field!(OUT_WIDTH, "out_width", bits);
field!(SIGNED, "signed", arithmetic.signed);
field!(SELECT_BITS, "select_bits", plexing.select_bits);
field!(ENABLE, "enable", plexing.enable);
field!(TOP_RIGHT, "top_right", plexing.top_right);
field!(DISABLED_ZERO, "disabled_zero", plexing.disabled_zero);
field!(THREE_STATE, "three_state", plexing.three_state);
field!(SWITCH_COUNT, "switch_count", io.switch_count);
field!(ON, "on_color", io.on_color);
field!(OFF, "off_color", io.off_color);
field!(BACKGROUND, "background", io.background);
field!(ACTIVE_HIGH, "active_high", io.active_high);
field!(MULTIBIT, "multibit", io.rgb_multibit);
field!(ROWS, "matrix_rows", io.matrix_rows);
field!(COLUMNS, "matrix_columns", io.matrix_columns);
field!(CIRCULAR, "matrix_circular", io.matrix_circular);
field!(PERSISTENCE, "matrix_persistence", io.matrix_persistence);

enum_field!(PULL, "pull", bus.pull, [Pull::Zero => "0", Pull::One => "1", Pull::Error => "X"]);
enum_field!(EXTENSION, "type", extension, [Extension::Zero => "zero", Extension::One => "one",
    Extension::Sign => "sign", Extension::Input => "input"]);
enum_field!(SHIFT, "shift", arithmetic.shift, [Shift::LogicalLeft => "logical_left",
    Shift::LogicalRight => "logical_right", Shift::ArithmeticRight => "arithmetic_right",
    Shift::RotateLeft => "rotate_left", Shift::RotateRight => "rotate_right"]);
enum_field!(MATRIX_INPUT, "matrix_input", io.matrix_input, [MatrixInput::Columns => "column",
    MatrixInput::Rows => "row", MatrixInput::Select => "select"]);
enum_field!(HALIGN, "halign", text_horizontal, [HorizontalAlign::Left => "left",
    HorizontalAlign::Center => "center", HorizontalAlign::Right => "right"]);
enum_field!(VALIGN, "valign", text_vertical, [VerticalAlign::Top => "top",
    VerticalAlign::Center => "center", VerticalAlign::Baseline => "base", VerticalAlign::Bottom => "bottom"]);

const LABEL_FACING: Field = Field {
    name: "label_facing",
    read: |c| {
        if c.label_center {
            "center"
        } else {
            c.label_facing.name()
        }
        .into()
    },
    write: |c, value| {
        c.label_center = c.kind == Kind::Button
            && value
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case("center"));
        c.label_facing = if c.label_center {
            Facing::North
        } else {
            choice("label_facing", value, Facing::ALL.map(|f| (f, f.name())))?
        };
        Ok(())
    },
};

// (Core kind, optional short name, component-specific fields). Core names are always accepted.
type Entry = (Kind, Option<&'static str>, &'static [Field]);
const CATALOG: &[Entry] = &[
    (Kind::Constant, None, &[BITS, VALUE]),
    (Kind::Ground, None, &[BITS]),
    (Kind::Power, None, &[BITS]),
    (Kind::PullResistor, None, &[PULL]),
    (Kind::Button, None, &[ON, LABEL_FACING]),
    (Kind::Switch, None, &[BITS, ON, LABEL_FACING]),
    (
        Kind::DipSwitch,
        Some("DipSwitch"),
        &[SWITCH_COUNT, ON, LABEL_FACING],
    ),
    (Kind::Clock, None, &[HIGH, LOW, LABEL_FACING]),
    (Kind::Text, None, &[TEXT, HALIGN, VALIGN]),
    (Kind::BitExtender, None, &[IN_WIDTH, OUT_WIDTH, EXTENSION]),
    (Kind::Arithmetic(Arithmetic::Shifter), None, &[BITS, SHIFT]),
    (Kind::Arithmetic(Arithmetic::Adder), None, &[BITS]),
    (
        Kind::Arithmetic(Arithmetic::Comparator),
        None,
        &[BITS, SIGNED],
    ),
    (
        Kind::Plexer(Plexer::Multiplexer),
        None,
        &[BITS, SELECT_BITS, ENABLE, TOP_RIGHT, DISABLED_ZERO],
    ),
    (
        Kind::Plexer(Plexer::Demultiplexer),
        None,
        &[
            BITS,
            SELECT_BITS,
            ENABLE,
            TOP_RIGHT,
            DISABLED_ZERO,
            THREE_STATE,
        ],
    ),
    (
        Kind::Plexer(Plexer::Decoder),
        None,
        &[SELECT_BITS, ENABLE, TOP_RIGHT, DISABLED_ZERO, THREE_STATE],
    ),
    (Kind::Led, None, &[ON, OFF, ACTIVE_HIGH, LABEL_FACING]),
    (
        Kind::SevenSegment,
        Some("SevenSegment"),
        &[ON, OFF, BACKGROUND, ACTIVE_HIGH],
    ),
    (Kind::HexDisplay, Some("HexDisplay"), &[ON, OFF, BACKGROUND]),
    (Kind::RgbLed, Some("RGBLED"), &[MULTIBIT, LABEL_FACING]),
    (
        Kind::DotMatrix,
        Some("DotMatrix"),
        &[ON, OFF, ROWS, COLUMNS, MATRIX_INPUT, CIRCULAR, PERSISTENCE],
    ),
];

pub(super) fn kind(name: &str) -> Option<Kind> {
    CATALOG
        .iter()
        .find(|(k, alias, _)| {
            k.name().eq_ignore_ascii_case(name)
                || alias.is_some_and(|s| s.eq_ignore_ascii_case(name))
        })
        .map(|e| e.0)
}

pub(super) fn fields(kind: Kind) -> &'static [Field] {
    CATALOG.iter().find(|e| e.0 == kind).map_or(&[], |e| e.2)
}
