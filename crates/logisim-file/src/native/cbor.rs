//! LRS\0, a big-endian u16 version, then a definite-length packed CBOR map.
//! Preflight does not allocate a DTO or trust a declared container length.
use super::{Error, FORMAT, MAGIC, MAX_BYTES, Native, Project, Result, limits};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const VERSION: u16 = 1;
const HEADER_LEN: usize = 6;

pub fn parse_native(bytes: &[u8]) -> Result<Project> {
    document(bytes)?.decode()
}

pub(super) fn document(bytes: &[u8]) -> Result<Native> {
    if bytes.len() > MAX_BYTES {
        return Err(Error("Native project exceeds 8 MiB".into()));
    }
    if !bytes.starts_with(MAGIC) {
        if bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
            return Err(Error("Legacy JSON native project. Use logisim-agent lrs migrate --input OLD.lrs --output NEW.lrs.".into()));
        }
        return Err(Error("Invalid .lrs magic".into()));
    }
    let version = bytes
        .get(4..HEADER_LEN)
        .ok_or_else(|| Error("Truncated .lrs header".into()))?;
    let version = u16::from_be_bytes([version[0], version[1]]);
    if version != VERSION {
        return Err(Error(format!(
            "Unsupported native project version {version}"
        )));
    }
    let payload = &bytes[HEADER_LEN..];
    check(payload)?;
    let mut de = serde_cbor::Deserializer::from_slice(payload).disable_legacy_enums();
    let dto = Native::deserialize(&mut de).map_err(|e| Error(format!("Invalid .lrs CBOR: {e}")))?;
    if dto.format != FORMAT || dto.version != u32::from(VERSION) {
        return Err(Error("Unsupported native payload format or version".into()));
    }
    Ok(dto)
}

pub fn serialize_native(project: &Project) -> Result<Vec<u8>> {
    let mut output = limits::Output(Vec::from(*MAGIC));
    output.0.extend_from_slice(&VERSION.to_be_bytes());
    let mut enc =
        serde_cbor::Serializer::new(serde_cbor::ser::IoWrite::new(&mut output)).packed_format();
    Native::encode(project)?
        .serialize(&mut enc)
        .map_err(|e| Error(format!("Cannot encode .lrs: {e}")))?;
    check(&output.0[HEADER_LEN..])?;
    Ok(output.0)
}

fn check(payload: &[u8]) -> Result<()> {
    if !payload.first().is_some_and(|b| b >> 5 == 5) {
        return Err(Error("Expected a native CBOR map".into()));
    }
    let mut scan = Scan {
        bytes: payload,
        nodes: limits::MAX_NODES,
    };
    scan.value(0)?;
    if !scan.bytes.is_empty() {
        return Err(Error("Trailing data in native project".into()));
    }
    Ok(())
}

struct Scan<'a> {
    bytes: &'a [u8],
    nodes: usize,
}
impl<'a> Scan<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let (head, tail) = self
            .bytes
            .split_at_checked(n)
            .ok_or_else(|| Error("Truncated CBOR item".into()))?;
        self.bytes = tail;
        Ok(head)
    }
    fn head(&mut self) -> Result<(u8, u64)> {
        let first = self.take(1)?[0];
        if first >> 5 == 7 && !matches!(first, 0xf4..=0xf6) {
            return Err(Error("Unsupported CBOR simple value".into()));
        }
        let arg = match first & 31 {
            n @ 0..=23 => u64::from(n),
            n @ 24..=27 => {
                let mut v = [0; 8];
                let width = 1 << (n - 24);
                v[8 - width..].copy_from_slice(self.take(width)?);
                u64::from_be_bytes(v)
            }
            _ => return Err(Error("Invalid or indefinite CBOR length".into())),
        };
        Ok((first >> 5, arg))
    }
    fn string(&mut self, n: u64) -> Result<&'a [u8]> {
        if n > limits::MAX_STRING as u64 {
            return Err(Error("CBOR string length limit exceeded".into()));
        }
        self.take(n as usize)
    }
    fn value(&mut self, depth: usize) -> Result<()> {
        if depth >= 128 || self.nodes == 0 {
            return Err(Error("CBOR depth or node limit exceeded".into()));
        }
        self.nodes -= 1;
        let (major, n) = self.head()?;
        match major {
            0 | 1 => (),
            3 => {
                self.string(n)?;
            }
            4 | 5 if n > limits::MAX_ENTRIES as u64 || n > self.nodes as u64 => {
                return Err(Error("CBOR container length or node limit exceeded".into()));
            }
            4 => {
                for _ in 0..n {
                    self.value(depth + 1)?;
                }
            }
            5 => {
                // Schema slots are small integers. Only plugin property dictionaries
                // need a set of text keys, borrowed directly from the file.
                let mut slots = 0u64;
                let mut names = HashSet::new();
                for _ in 0..n {
                    let (kind, key) = self.head()?;
                    let unique = match kind {
                        0 if key < 64 => {
                            let mask = 1u64 << key;
                            let new = slots & mask == 0;
                            slots |= mask;
                            new
                        }
                        3 => names.insert(self.string(key)?),
                        _ => return Err(Error("Unknown CBOR field number or key type".into())),
                    };
                    if !unique {
                        return Err(Error("Duplicate CBOR map key".into()));
                    }
                    self.value(depth + 1)?;
                }
            }
            7 if matches!(n, 20..=22) => (),
            _ => return Err(Error("Unsupported CBOR type in native document".into())),
        }
        Ok(())
    }
}
