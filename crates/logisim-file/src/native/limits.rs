//! Check the envelope and budgets before allocating document DTOs. Duplicate
//! keys are errors even inside plugin-owned property maps.
use super::{Error, FORMAT, Result};
use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use std::{collections::HashSet, fmt};

const MAX_NODES: usize = 1_000_000;
const MAX_ENTRIES: usize = 65_536;
const MAX_STRING: usize = logisim_core::plugin::api::MAX_MESSAGE;

pub(super) fn check(bytes: &[u8]) -> Result<()> {
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let mut remaining = MAX_NODES;
    de.deserialize_map(Node {
        remaining: &mut remaining,
        envelope: true,
    })
    .and_then(|()| de.end())
    .map_err(|e| Error(format!("Invalid .lrs: {e}")))
}

struct Node<'a> {
    remaining: &'a mut usize,
    envelope: bool,
}
impl<'de> DeserializeSeed<'de> for &mut Node<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, de: D) -> std::result::Result<(), D::Error> {
        *self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| de::Error::custom("JSON node limit exceeded"))?;
        de.deserialize_any(Node {
            remaining: self.remaining,
            envelope: false,
        })
    }
}
impl<'de> Visitor<'de> for Node<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a bounded JSON document")
    }
    fn visit_seq<A: SeqAccess<'de>>(mut self, mut a: A) -> std::result::Result<(), A::Error> {
        let mut count = 0;
        while a.next_element_seed(&mut self)?.is_some() {
            count += 1;
            if count > MAX_ENTRIES {
                return Err(de::Error::custom("array length limit exceeded"));
            }
        }
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(mut self, mut a: A) -> std::result::Result<(), A::Error> {
        let mut keys = HashSet::new();
        while let Some(key) = a.next_key::<String>()? {
            if key.len() > MAX_STRING || keys.len() >= MAX_ENTRIES {
                return Err(de::Error::custom("object key or length limit exceeded"));
            }
            match (self.envelope, key.as_str()) {
                (true, "format") => {
                    let name = a.next_value::<String>()?;
                    if name != FORMAT {
                        return Err(de::Error::custom("unknown native project format"));
                    }
                }
                (true, "version") => {
                    let version = a.next_value::<u32>()?;
                    if version != 1 {
                        return Err(de::Error::custom(format!(
                            "unsupported native project version {version}"
                        )));
                    }
                }
                _ => {
                    a.next_value_seed(&mut self)?;
                }
            }
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
        }
        Ok(())
    }
    fn visit_str<E: de::Error>(self, s: &str) -> std::result::Result<(), E> {
        if s.len() > MAX_STRING {
            return Err(E::custom("string length limit exceeded"));
        }
        Ok(())
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: de::Error>(self) -> std::result::Result<(), E> {
        Ok(())
    }
}

/// Apply the same byte limit while producing pretty JSON.
pub(super) struct Output(pub(super) Vec<u8>);
impl std::io::Write for Output {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.len() > super::MAX_BYTES - self.0.len() {
            return Err(std::io::Error::other("Native project exceeds 8 MiB"));
        }
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
