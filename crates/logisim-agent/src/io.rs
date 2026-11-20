use crate::{Error, Result};
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, Read, Write},
    path::Path,
};

pub fn read(path: &Path) -> Result<Vec<u8>> {
    fn error(e: std::io::Error) -> Error {
        Error::new(
            if e.kind() == std::io::ErrorKind::NotFound {
                "not_found"
            } else {
                "io"
            },
            e,
        )
    }
    if !fs::metadata(path).map_err(error)?.is_file() {
        return Err(Error::invalid("Expected a regular file."));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(error)?
        .take(logisim_file::MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() > logisim_file::MAX_BYTES {
        return Err(Error::new("too_large", "File exceeds 8 MiB."));
    }
    Ok(bytes)
}

pub fn write(out: &mut impl Write, value: &Value) -> std::io::Result<()> {
    serde_json::to_writer(&mut *out, value)?;
    out.write_all(b"\n")?;
    out.flush()
}

/// Drain oversized lines without accumulating them or treating their suffix as another request.
pub fn lines(
    mut input: impl BufRead,
    mut handle: impl FnMut(Result<Value>) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        if (&mut input)
            .take(logisim_file::MAX_BYTES as u64 + 1)
            .read_until(b'\n', &mut bytes)?
            == 0
        {
            return Ok(());
        }
        let value = if bytes.len() > logisim_file::MAX_BYTES {
            if bytes.last() != Some(&b'\n') {
                input.skip_until(b'\n')?;
            }
            Err(Error::new("too_large", "Request exceeds 8 MiB."))
        } else {
            serde_json::from_slice(&bytes).map_err(|e| Error::new("invalid_json", e))
        };
        handle(value)?;
    }
}
