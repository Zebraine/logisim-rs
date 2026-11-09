//! Standalone memory images specified from runtime import/export observations.
use logisim_core::MemoryContents;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HexError {
    TooLarge,
    Dimensions,
    Header,
    Word { line: usize },
    Count { line: usize },
    Capacity { line: usize },
}

/// Read a `v2.0 raw` image. Missing addresses are zero; high data bits are truncated.
/// Parsing is independent of the destination, so failure cannot partially replace it.
pub fn parse_hex(text: &str, address_bits: u8, data_bits: u8) -> Result<MemoryContents, HexError> {
    if text.len() > crate::MAX_BYTES {
        return Err(HexError::TooLarge);
    }
    if !(2..=24).contains(&address_bits) || !(1..=32).contains(&data_bits) {
        return Err(HexError::Dimensions);
    }
    let mut lines = text.lines();
    if lines.next() != Some("v2.0 raw") {
        return Err(HexError::Header);
    }
    let capacity = 1u32 << address_bits;
    let mask = u32::MAX >> (32 - data_bits);
    let mut contents = MemoryContents::default();
    let mut address = 0u32;
    for (index, line) in lines.enumerate() {
        let number = index + 2;
        let line = line.split('#').next().unwrap_or_default();
        for token in line.split_ascii_whitespace() {
            let (count, word) = if let Some((count, word)) = token.split_once('*') {
                if count.is_empty() || !count.bytes().all(|c| c.is_ascii_digit()) {
                    return Err(HexError::Count { line: number });
                }
                let count = count
                    .parse::<u32>()
                    .map_err(|_| HexError::Capacity { line: number })?;
                (count, word)
            } else {
                (1, token)
            };
            let digits = word.strip_prefix(['+', '-']).unwrap_or(word);
            if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(HexError::Word { line: number });
            }
            let value = i64::from_str_radix(word, 16)
                .map_err(|_| HexError::Word { line: number })? as u32
                & mask;
            let end = address
                .checked_add(count)
                .filter(|end| *end <= capacity)
                .ok_or(HexError::Capacity { line: number })?;
            contents.fill(address, end, value);
            address = end;
        }
    }
    Ok(contents)
}

#[cfg(test)]
mod tests;
