//! The patch catalog maps public keys directly to persistent core attributes.
//! After typed decoding, the file codec validates the complete prospective component.
use super::edit::{choice, decode};
use crate::Result;
use logisim_core::{
    Arithmetic, BitSearch, Component, ControlPosition, CounterOverflow, Extension, Facing,
    FlipFlop, GateSize, HorizontalAlign, Kind, Latch, LatchEnable, MatrixInput, Plexer, Pull,
    Shift, TransistorType, Trigger, VerticalAlign,
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
field!(BIT_INPUTS, "bit_inputs", arithmetic.bit_inputs);
field!(NO_INPUT_ZERO, "no_input_zero", plexing.no_input_zero);
field!(GROUP_BITS, "group_bits", plexing.group_bits);
field!(DISPLAY_MULTIBIT, "multibit", plexing.multibit);
field!(CONTROL_LEFT, "control_left", bus.control_left);
field!(NEGATE_CONTROL, "negate_control", bus.negate_control);
field!(
    GATE_BOTTOM_RIGHT,
    "gate_bottom_right",
    transistor.gate_bottom_right
);
enum_field!(SEARCH, "search", arithmetic.search, [BitSearch::LowestOne => "lowest_one",
    BitSearch::HighestOne => "highest_one", BitSearch::LowestZero => "lowest_zero",
    BitSearch::HighestZero => "highest_zero"]);
enum_field!(CONTROLLED_SIZE, "size", bus.size, [GateSize::Narrow => "narrow",
    GateSize::Medium => "medium", GateSize::Wide => "wide"]);
enum_field!(TRANSISTOR_TYPE, "transistor_type", transistor.transistor_type,
    [TransistorType::P => "p", TransistorType::N => "n"]);
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

field!(
    NEGATE_CONTROLS,
    "negate_controls",
    sequential.negate_controls
);
field!(NEW_LAYOUT, "new_layout", sequential.new_layout);
field!(SELECT_HIGH, "select_high", sequential.select_high);
field!(COUNTER_MAX, "counter_max", sequential.counter_max);
field!(COUNTER_OLD, "counter_old", sequential.counter_old);
field!(RANDOM_SEED, "random_seed", sequential.random_seed);

enum_field!(TRIGGER, "trigger", sequential.trigger, [Trigger::Rising => "rising",
    Trigger::Falling => "falling", Trigger::High => "high", Trigger::Low => "low"]);
enum_field!(LATCH_TRIGGER, "trigger", latch_enable, [LatchEnable::High => "high",
    LatchEnable::Low => "low", LatchEnable::None => "latch"]);
enum_field!(CONTROLS, "controls", sequential.controls, [ControlPosition::AboveBelow => "above_below",
    ControlPosition::BelowAbove => "below_above", ControlPosition::Legacy => "legacy"]);
enum_field!(OVERFLOW, "counter_overflow", sequential.counter_overflow,
    [CounterOverflow::Wrap => "wrap", CounterOverflow::Stay => "stay",
     CounterOverflow::Continue => "continue", CounterOverflow::Load => "load"]);

const FLIP_FLOP_FIELDS: &[Field] = &[TRIGGER, CONTROLS, NEGATE_CONTROLS, NEW_LAYOUT];
const LATCH_FIELDS: &[Field] = &[LATCH_TRIGGER, CONTROLS, NEGATE_CONTROLS, NEW_LAYOUT];

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
    (Kind::Arithmetic(Arithmetic::Subtractor), None, &[BITS]),
    (Kind::Arithmetic(Arithmetic::Multiplier), None, &[BITS]),
    (Kind::Arithmetic(Arithmetic::Divider), None, &[BITS]),
    (Kind::Arithmetic(Arithmetic::Negator), None, &[BITS]),
    (
        Kind::Arithmetic(Arithmetic::BitAdder),
        Some("BitAdder"),
        &[BITS, BIT_INPUTS],
    ),
    (
        Kind::Arithmetic(Arithmetic::BitFinder),
        Some("BitFinder"),
        &[BITS, SEARCH],
    ),
    (
        Kind::Plexer(Plexer::PriorityEncoder),
        None,
        &[SELECT_BITS, DISABLED_ZERO, NO_INPUT_ZERO],
    ),
    (
        Kind::Plexer(Plexer::BitSelector),
        Some("BitSelector"),
        &[BITS, GROUP_BITS, TOP_RIGHT],
    ),
    (
        Kind::Plexer(Plexer::DisplayDecoder),
        Some("DisplayDecoder"),
        &[DISPLAY_MULTIBIT],
    ),
    (
        Kind::ControlledBuffer,
        None,
        &[BITS, CONTROL_LEFT, NEGATE_CONTROL],
    ),
    (
        Kind::ControlledInverter,
        None,
        &[BITS, CONTROL_LEFT, NEGATE_CONTROL, CONTROLLED_SIZE],
    ),
    (
        Kind::Transistor,
        None,
        &[BITS, GATE_BOTTOM_RIGHT, TRANSISTOR_TYPE],
    ),
    (Kind::TransmissionGate, None, &[BITS, GATE_BOTTOM_RIGHT]),
    (Kind::FlipFlop(FlipFlop::D), None, FLIP_FLOP_FIELDS),
    (Kind::FlipFlop(FlipFlop::T), None, FLIP_FLOP_FIELDS),
    (
        Kind::FlipFlop(FlipFlop::Jk),
        Some("JK Flip-Flop"),
        FLIP_FLOP_FIELDS,
    ),
    (
        Kind::FlipFlop(FlipFlop::Sr),
        Some("SR Flip-Flop"),
        FLIP_FLOP_FIELDS,
    ),
    (Kind::Latch(Latch::D), None, LATCH_FIELDS),
    (Kind::Latch(Latch::Sr), Some("SR Latch"), LATCH_FIELDS),
    (Kind::Register, None, &[BITS, TRIGGER, SELECT_HIGH]),
    (
        Kind::Counter,
        None,
        &[BITS, TRIGGER, COUNTER_MAX, OVERFLOW, COUNTER_OLD],
    ),
    (Kind::ShiftRegister, Some("ShiftRegister"), &[BITS, TRIGGER]),
    (Kind::Random, Some("Random"), &[BITS, TRIGGER, RANDOM_SEED]),
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
