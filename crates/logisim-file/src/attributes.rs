//! Shared scalar attribute decoding. Callers own defaults, ranges and allowed keys.
use crate::{Error, Result};
use std::collections::BTreeMap;

pub(crate) type Attributes = BTreeMap<String, String>;

pub(crate) fn value<'a>(a: &'a Attributes, key: &str, default: &'a str) -> &'a str {
    a.get(key).map(String::as_str).unwrap_or(default)
}
pub(crate) fn number(
    a: &Attributes,
    key: &str,
    default: u8,
    range: std::ops::RangeInclusive<u8>,
) -> Result<u8> {
    let n = if let Some(s) = a.get(key) {
        s.parse::<u8>()
            .map_err(|_| Error(format!("Invalid {key}: {s}")))?
    } else {
        default
    };
    if !range.contains(&n) {
        return Err(Error(format!("Unsupported {key}: {n}")));
    }
    Ok(n)
}
pub(crate) fn boolean(a: &Attributes, key: &str, default: bool) -> Result<bool> {
    match a.get(key).map(String::as_str) {
        None => Ok(default),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(v) => Err(Error(format!("Invalid {key}: {v}"))),
    }
}
