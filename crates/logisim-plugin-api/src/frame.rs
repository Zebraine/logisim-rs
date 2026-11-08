//! Optional linear-memory transport v1 for ABI 3 stable execution.
//!
//! Negotiation: an ABI 3 plugin may export `logisim_frame_version() -> u32`.
//! Version 1 additionally exports `logisim_frame_config(length: u32) -> u32`
//! and `logisim_frame_call(length: u32) -> u64`. Missing/unknown versions use JSON;
//! a recognized version with missing or malformed exports is an error.
//! Both entry points consume the existing `logisim_buffer(length)` input range.
//! Config accepts JSON `StableConfig`, returns 0 on success, and binds immutable
//! tool/properties once per execution VM (including recreated VMs after reset).
//! A frame call returns `(length << 32) | wasm32_offset`, valid until the next
//! guest call. Both memory ranges must be within exported linear memory.
//! This extension must have exactly the same state, graphics and error behavior
//! as the JSON stable interface. It does not change atomic batch commit, explicit
//! state ownership or the StableContract purity/dependency requirements. Handlers
//! must not retain borrowed input/state after returning. Reset/actions use JSON.
//! Any incompatible layout change requires a new transport version.
//!
//! All integers are little endian, all records have alignment 1 and no padding.
//! Header (32 bytes): magic `LSF1`, version, operation, flags, signal count,
//! state byte count, graphic count, text byte count (each remaining field u32).
//! Then signals (16 bytes each: width/value/unknown/error as u32), opaque state,
//! graphics (40 bytes each), and UTF-8 text. Graphic records contain kind (0 rect,
//! 1 line, 2 ellipse, 3 text), four i32 coordinates, fill RGBA, stroke RGBA, size,
//! text offset and text length. Offsets are relative to this frame's text section.
//!
//! Requests contain signals/state only, flags 0. Outputs have no state, flags 0;
//! transitions have state only, flags bit0=outputs_changed, bit1=fixed_point.
//! Error frames (operation 3) contain only UTF-8 text, at most 2048 bytes.
//! No Rust pointers, enums, Vec headers, usize or native struct layouts cross the ABI.
use crate::{
    MAX_MESSAGE, MAX_STATE, Shape, Signal, StableOutputs, StateTransition,
    validate::validate_graphics,
};
use zerocopy::byteorder::little_endian::{I32, U32};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned};
#[cfg(test)]
mod tests;

pub const VERSION: u32 = 1;
const INVALID: &str = "Invalid plugin linear-memory frame.";
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Outputs,
    Transition,
}
impl Operation {
    fn code(self) -> u32 {
        match self {
            Self::Outputs => 1,
            Self::Transition => 2,
        }
    }
}
#[derive(FromBytes, IntoBytes, KnownLayout, Immutable, Unaligned)]
#[repr(C)]
struct Header {
    magic: [u8; 4],
    version: U32,
    operation: U32,
    flags: U32,
    signals: U32,
    state: U32,
    graphics: U32,
    text: U32,
}
#[derive(FromBytes, IntoBytes, KnownLayout, Immutable, Unaligned)]
#[repr(C)]
struct Port {
    width: U32,
    value: U32,
    unknown: U32,
    error: U32,
}
#[derive(FromBytes, IntoBytes, KnownLayout, Immutable, Unaligned)]
#[repr(C)]
struct Graphic {
    kind: U32,
    xy: [I32; 4],
    fill: [u8; 4],
    stroke: [u8; 4],
    size: U32,
    offset: U32,
    length: U32,
}
struct Parts<'a> {
    header: &'a Header,
    signals: &'a [Port],
    state: &'a [u8],
    graphics: &'a [Graphic],
    text: &'a [u8],
}
impl<'a> Parts<'a> {
    fn read(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() > MAX_MESSAGE {
            return Err(INVALID.into());
        }
        let (h, rest) = Header::ref_from_prefix(bytes).map_err(|_| INVALID)?;
        if h.magic != *b"LSF1"
            || h.version.get() != VERSION
            || h.signals.get() > 64
            || h.state.get() as usize > MAX_STATE
            || h.graphics.get() > 256
            || h.text.get() as usize > MAX_MESSAGE
        {
            return Err(INVALID.into());
        }
        let (signals, rest) = <[Port]>::ref_from_prefix_with_elems(rest, h.signals.get() as usize)
            .map_err(|_| INVALID)?;
        let (state, rest) = rest
            .split_at_checked(h.state.get() as usize)
            .ok_or(INVALID)?;
        let (graphics, text) =
            <[Graphic]>::ref_from_prefix_with_elems(rest, h.graphics.get() as usize)
                .map_err(|_| INVALID)?;
        if text.len() != h.text.get() as usize {
            return Err(INVALID.into());
        }
        Ok(Self {
            header: h,
            signals,
            state,
            graphics,
            text,
        })
    }
    fn signals(&self) -> Result<impl Iterator<Item = Signal> + '_> {
        for p in self.signals {
            if p.width.get() > 32 {
                return Err(INVALID.into());
            }
            p.signal().validate()?;
        }
        Ok(self.signals.iter().map(Port::signal))
    }
}
impl Port {
    fn signal(&self) -> Signal {
        Signal {
            width: self.width.get() as u8,
            value: self.value.get(),
            unknown: self.unknown.get(),
            error: self.error.get(),
        }
    }
}
/// Stack-sized signals and a borrowed opaque state; no decoding allocation.
pub struct Input<'a> {
    pub operation: Operation,
    signals: [Signal; 64],
    count: usize,
    pub state: &'a [u8],
}
impl<'a> Input<'a> {
    pub fn read(bytes: &'a [u8]) -> Result<Self> {
        let p = Parts::read(bytes)?;
        if p.header.flags.get() != 0 || !p.graphics.is_empty() || !p.text.is_empty() {
            return Err(INVALID.into());
        }
        let operation = match p.header.operation.get() {
            1 => Operation::Outputs,
            2 => Operation::Transition,
            _ => return Err(INVALID.into()),
        };
        let mut signals = [Signal {
            width: 1,
            value: 0,
            unknown: 0,
            error: 0,
        }; 64];
        for (slot, value) in signals.iter_mut().zip(p.signals()?) {
            *slot = value;
        }
        Ok(Self {
            operation,
            signals,
            count: p.signals.len(),
            state: p.state,
        })
    }
    pub fn signals(&self) -> &[Signal] {
        &self.signals[..self.count]
    }
}
pub fn request_len(signals: &[Signal], state: &[u8]) -> Result<usize> {
    if signals.len() > 64 || state.len() > MAX_STATE {
        return Err(INVALID.into());
    }
    for signal in signals {
        signal.validate()?;
    }
    Ok(size_of::<Header>() + size_of::<Port>() * signals.len() + state.len())
}
/// Write directly into the guest's checked input range; no intermediate byte vector.
pub fn write_request(
    bytes: &mut [u8],
    operation: Operation,
    signals: &[Signal],
    state: &[u8],
) -> Result<()> {
    if bytes.len() != request_len(signals, state)? {
        return Err(INVALID.into());
    }
    write_header(bytes, operation.code(), 0, signals.len(), state.len(), 0, 0);
    let mut position = size_of::<Header>();
    write_signals(bytes, &mut position, signals);
    bytes[position..].copy_from_slice(state);
    Ok(())
}
#[derive(Debug, PartialEq, Eq)]
pub enum Output {
    Outputs(StableOutputs),
    Transition(StateTransition),
}
/// Reuse the SDK output buffer; state remains opaque and graphics have fixed records.
pub fn write_output(bytes: &mut Vec<u8>, output: &Output) -> Result<()> {
    let (operation, flags, signals, state, graphics): (_, _, &[_], &[_], &[_]) = match output {
        Output::Outputs(o) => (1, 0, o.outputs.as_slice(), &[][..], o.graphics.as_slice()),
        Output::Transition(t) => (
            2,
            u32::from(t.outputs_changed) | (u32::from(t.fixed_point) << 1),
            &[][..],
            t.state.as_slice(),
            &[][..],
        ),
    };
    request_len(signals, state)?;
    validate_graphics(graphics)?;
    let text_len: usize = graphics
        .iter()
        .map(|s| match s {
            Shape::Text { text, .. } => text.len(),
            _ => 0,
        })
        .sum();
    let len = size_of::<Header>()
        + size_of::<Port>() * signals.len()
        + state.len()
        + size_of::<Graphic>() * graphics.len()
        + text_len;
    if len > MAX_MESSAGE {
        return Err(INVALID.into());
    }
    bytes.resize(len, 0);
    write_header(
        bytes,
        operation,
        flags,
        signals.len(),
        state.len(),
        graphics.len(),
        text_len,
    );
    let mut position = size_of::<Header>();
    write_signals(bytes, &mut position, signals);
    bytes[position..position + state.len()].copy_from_slice(state);
    position += state.len();
    let text_start = position + graphics.len() * size_of::<Graphic>();
    let mut text_offset = 0;
    for shape in graphics {
        let (kind, xy, fill, stroke, size, text) = match shape {
            Shape::Rect {
                min,
                max,
                fill,
                stroke,
            } => (0, [min[0], min[1], max[0], max[1]], *fill, *stroke, 0, ""),
            Shape::Line { a, b, color } => (1, [a[0], a[1], b[0], b[1]], [0; 4], *color, 0, ""),
            Shape::Ellipse {
                min,
                max,
                fill,
                stroke,
            } => (2, [min[0], min[1], max[0], max[1]], *fill, *stroke, 0, ""),
            Shape::Text {
                at,
                text,
                size,
                color,
            } => (
                3,
                [at[0], at[1], 0, 0],
                *color,
                [0; 4],
                u32::from(*size),
                text.as_str(),
            ),
        };
        let record = Graphic {
            kind: kind.into(),
            xy: xy.map(Into::into),
            fill,
            stroke,
            size: size.into(),
            offset: (text_offset as u32).into(),
            length: (text.len() as u32).into(),
        };
        bytes[position..position + size_of::<Graphic>()].copy_from_slice(record.as_bytes());
        position += size_of::<Graphic>();
        bytes[text_start + text_offset..text_start + text_offset + text.len()]
            .copy_from_slice(text.as_bytes());
        text_offset += text.len();
    }
    Ok(())
}
pub fn write_error(bytes: &mut Vec<u8>, error: &str) {
    let mut len = error.len().min(2048);
    while !error.is_char_boundary(len) {
        len -= 1;
    }
    bytes.resize(size_of::<Header>() + len, 0);
    write_header(bytes, 3, 0, 0, 0, 0, len);
    bytes[size_of::<Header>()..].copy_from_slice(&error.as_bytes()[..len]);
}
pub fn read_output(bytes: &[u8], expected: Operation) -> Result<Output> {
    let p = Parts::read(bytes)?;
    if p.header.operation.get() == 3 {
        if p.header.flags.get() != 0
            || !p.signals.is_empty()
            || !p.state.is_empty()
            || !p.graphics.is_empty()
            || p.text.len() > 2048
        {
            return Err(INVALID.into());
        }
        return Err(std::str::from_utf8(p.text).map_err(|_| INVALID)?.to_owned());
    }
    if p.header.operation.get() != expected.code() {
        return Err(INVALID.into());
    }
    match expected {
        Operation::Transition => {
            if p.header.flags.get() & !3 != 0
                || !p.signals.is_empty()
                || !p.graphics.is_empty()
                || !p.text.is_empty()
            {
                return Err(INVALID.into());
            }
            Ok(Output::Transition(StateTransition {
                state: p.state.to_vec(),
                outputs_changed: p.header.flags.get() & 1 != 0,
                fixed_point: p.header.flags.get() & 2 != 0,
            }))
        }
        Operation::Outputs => {
            if p.header.flags.get() != 0 || !p.state.is_empty() {
                return Err(INVALID.into());
            }
            let outputs = p.signals()?.collect();
            let mut graphics = Vec::with_capacity(p.graphics.len());
            let mut next_text = 0;
            for g in p.graphics {
                let [x, y, a, b] = g.xy.map(|v| v.get());
                let min = [x, y];
                let max = [a, b];
                if g.offset.get() as usize != next_text {
                    return Err(INVALID.into());
                }
                let length = g.length.get() as usize;
                let text = p
                    .text
                    .get(next_text..next_text.checked_add(length).ok_or(INVALID)?)
                    .ok_or(INVALID)?;
                if g.kind.get() != 3 && (length != 0 || g.size.get() != 0) {
                    return Err(INVALID.into());
                }
                graphics.push(match g.kind.get() {
                    0 => Shape::Rect {
                        min,
                        max,
                        fill: g.fill,
                        stroke: g.stroke,
                    },
                    1 if g.fill == [0; 4] => Shape::Line {
                        a: min,
                        b: max,
                        color: g.stroke,
                    },
                    2 => Shape::Ellipse {
                        min,
                        max,
                        fill: g.fill,
                        stroke: g.stroke,
                    },
                    3 if length <= 1024
                        && max == [0, 0]
                        && g.stroke == [0; 4]
                        && g.size.get() <= 64 =>
                    {
                        Shape::Text {
                            at: min,
                            text: std::str::from_utf8(text).map_err(|_| INVALID)?.to_owned(),
                            size: g.size.get() as u8,
                            color: g.fill,
                        }
                    }
                    _ => return Err(INVALID.into()),
                });
                next_text += length;
            }
            if next_text != p.text.len() {
                return Err(INVALID.into());
            }
            validate_graphics(&graphics)?;
            Ok(Output::Outputs(StableOutputs { outputs, graphics }))
        }
    }
}
fn write_header(
    bytes: &mut [u8],
    operation: u32,
    flags: u32,
    signals: usize,
    state: usize,
    graphics: usize,
    text: usize,
) {
    let h = Header {
        magic: *b"LSF1",
        version: VERSION.into(),
        operation: operation.into(),
        flags: flags.into(),
        signals: (signals as u32).into(),
        state: (state as u32).into(),
        graphics: (graphics as u32).into(),
        text: (text as u32).into(),
    };
    bytes[..size_of::<Header>()].copy_from_slice(h.as_bytes());
}
fn write_signals(bytes: &mut [u8], position: &mut usize, signals: &[Signal]) {
    for s in signals {
        let p = Port {
            width: u32::from(s.width).into(),
            value: s.value.into(),
            unknown: s.unknown.into(),
            error: s.error.into(),
        };
        bytes[*position..*position + size_of::<Port>()].copy_from_slice(p.as_bytes());
        *position += size_of::<Port>();
    }
}
