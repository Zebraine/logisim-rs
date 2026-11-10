//! Optional state-only raster v1. Exports `logisim_raster_version() -> u32`,
//! `logisim_raster_config(length) -> u32` (JSON StableConfig, 0 accepts) and
//! `logisim_raster_call(length) -> u64` (length << 32 | Wasm32 offset).
//! Uses logisim_buffer; no borrowed memory survives a guest call. Absent/unknown
//! versions retain vector display; recognized malformed extensions are errors.
//!
//! Rendering is pure in immutable config and explicit state, independent of input
//! signals, call order and VM identity. Stable outputs_changed includes this image.
//! A revision identifies identical dimensions, bounds and pixels within a config;
//! Unchanged requires the explicit previous revision. Host drops it on Reset or
//! config replacement. Plugins must not wrap revision counters. Frames are complete,
//! row-major unpremultiplied RGBA8, nearest filtered, painted after vector graphics.
//! One image per component, clipped to tool bounds; no images are saved to documents.
//!
//! All fields are little endian, alignment 1. Request: LSR1, previous-present u32,
//! previous revision u64, then opaque state. Response header: LSR1, kind u32
//! (0 none, 1 unchanged, 2 image, 3 UTF-8 error), revision u64, width/height u32,
//! min-x/min-y/max-x/max-y i32. Only image has geometry; only image/error has a
//! payload. Reserved fields must be zero. Errors <=2048 bytes, dimensions <=128.
use crate::{MAX_STATE, Tool};
use zerocopy::byteorder::little_endian::{I32, U32, U64};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned};
#[cfg(test)]
mod tests;

pub const VERSION: u32 = 1;
pub const MAX_SIDE: u32 = 128;
pub const MAX_RESPONSE: usize = 40 + (MAX_SIDE * MAX_SIDE * 4) as usize;
const INVALID: &str = "Invalid plugin raster frame.";
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub revision: u64,
    pub width: u32,
    pub height: u32,
    pub bounds: [[i32; 2]; 2],
    pub pixels: Vec<u8>,
}
impl Image {
    pub fn validate(&self) -> Result<()> {
        if self.width == 0
            || self.height == 0
            || self.width > MAX_SIDE
            || self.height > MAX_SIDE
            || self.pixels.len() != (self.width * self.height * 4) as usize
            || self
                .bounds
                .iter()
                .flatten()
                .any(|v| !(-4096..=4096).contains(v))
            || self.bounds[0][0] >= self.bounds[1][0]
            || self.bounds[0][1] >= self.bounds[1][1]
        {
            return Err(INVALID.into());
        }
        Ok(())
    }
    pub fn validate_tool(&self, tool: &Tool) -> Result<()> {
        self.validate()?;
        for axis in 0..2 {
            if self.bounds[0][axis] < tool.bounds[0][axis]
                || self.bounds[1][axis] > tool.bounds[1][axis]
            {
                return Err("Plugin raster lies outside tool bounds.".into());
            }
        }
        Ok(())
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Output {
    None,
    Unchanged(u64),
    Image(Image),
}
#[derive(FromBytes, IntoBytes, KnownLayout, Immutable, Unaligned)]
#[repr(C)]
struct RequestHeader {
    magic: [u8; 4],
    previous: U32,
    revision: U64,
}
#[derive(FromBytes, IntoBytes, KnownLayout, Immutable, Unaligned)]
#[repr(C)]
struct Header {
    magic: [u8; 4],
    kind: U32,
    revision: U64,
    width: U32,
    height: U32,
    xy: [I32; 4],
}

pub fn write_request(out: &mut Vec<u8>, previous: Option<u64>, state: &[u8]) -> Result<()> {
    if state.len() > MAX_STATE {
        return Err(INVALID.into());
    }
    out.clear();
    out.extend_from_slice(
        RequestHeader {
            magic: *b"LSR1",
            previous: u32::from(previous.is_some()).into(),
            revision: previous.unwrap_or(0).into(),
        }
        .as_bytes(),
    );
    out.extend_from_slice(state);
    Ok(())
}
pub fn read_request(bytes: &[u8]) -> Result<(Option<u64>, &[u8])> {
    let (h, state) = RequestHeader::ref_from_prefix(bytes).map_err(|_| INVALID)?;
    if h.magic != *b"LSR1"
        || h.previous.get() > 1
        || (h.previous.get() == 0 && h.revision.get() != 0)
        || state.len() > MAX_STATE
    {
        return Err(INVALID.into());
    }
    Ok(((h.previous.get() == 1).then_some(h.revision.get()), state))
}
fn header(kind: u32, revision: u64) -> Header {
    Header {
        magic: *b"LSR1",
        kind: kind.into(),
        revision: revision.into(),
        width: 0.into(),
        height: 0.into(),
        xy: [0.into(); 4],
    }
}
pub fn write_output(out: &mut Vec<u8>, value: &Output) -> Result<()> {
    let mut h = match value {
        Output::None => header(0, 0),
        Output::Unchanged(r) => header(1, *r),
        Output::Image(i) => {
            i.validate()?;
            header(2, i.revision)
        }
    };
    if let Output::Image(i) = value {
        h.width = i.width.into();
        h.height = i.height.into();
        h.xy = [
            i.bounds[0][0].into(),
            i.bounds[0][1].into(),
            i.bounds[1][0].into(),
            i.bounds[1][1].into(),
        ];
    }
    out.clear();
    out.extend_from_slice(h.as_bytes());
    if let Output::Image(i) = value {
        out.extend_from_slice(&i.pixels);
    }
    Ok(())
}
pub fn write_error(out: &mut Vec<u8>, message: &str) {
    out.clear();
    out.extend_from_slice(header(3, 0).as_bytes());
    for ch in message.chars() {
        if out.len() + ch.len_utf8() > 40 + 2048 {
            break;
        }
        out.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
    }
}
pub fn read_output(bytes: &[u8], previous: Option<u64>) -> Result<Output> {
    if bytes.len() > MAX_RESPONSE {
        return Err(INVALID.into());
    }
    let (h, data) = Header::ref_from_prefix(bytes).map_err(|_| INVALID)?;
    if h.magic != *b"LSR1" {
        return Err(INVALID.into());
    }
    if h.kind.get() != 2
        && (h.width.get() != 0 || h.height.get() != 0 || h.xy.iter().any(|v| v.get() != 0))
    {
        return Err(INVALID.into());
    }
    match h.kind.get() {
        0 if h.revision.get() == 0 && data.is_empty() => Ok(Output::None),
        1 if data.is_empty() && previous == Some(h.revision.get()) => {
            Ok(Output::Unchanged(h.revision.get()))
        }
        2 => {
            let image = Image {
                revision: h.revision.get(),
                width: h.width.get(),
                height: h.height.get(),
                bounds: [
                    [h.xy[0].get(), h.xy[1].get()],
                    [h.xy[2].get(), h.xy[3].get()],
                ],
                pixels: data.to_vec(),
            };
            image.validate()?;
            Ok(Output::Image(image))
        }
        3 if h.revision.get() == 0 && data.len() <= 2048 => {
            Err(std::str::from_utf8(data).map_err(|_| INVALID)?.to_owned())
        }
        _ => Err(INVALID.into()),
    }
}
