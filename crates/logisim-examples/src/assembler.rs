//! Small two-pass assembler. Source, diagnostics and generated images stay outside the host.
use crate::cpu::isa::{ALL, Format};
use std::collections::BTreeMap;
#[cfg(test)]
mod tests;

pub fn assemble(source: &str) -> Result<Vec<u32>, String> {
    Ok(compile(source)?.words)
}

pub struct Assembly {
    pub words: Vec<u32>,
    pub symbols: BTreeMap<String, i64>,
}

pub fn compile(source: &str) -> Result<Assembly, String> {
    if source.len() > 1_048_576 {
        return Err("Assembly source exceeds 1 MiB".into());
    }
    let mut symbols = BTreeMap::new();
    let mut lines = vec![];
    let mut pc = 0usize;
    for (line, raw) in source.lines().enumerate() {
        let error = |s: &str| format!("Line {}: {s}", line + 1);
        let mut text = raw.split([';', '#']).next().unwrap().trim();
        if let Some((label, rest)) = text.split_once(':') {
            define(&mut symbols, label.trim(), pc as i64).map_err(|e| error(&e))?;
            text = rest.trim();
        }
        let tokens: Vec<_> = text
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .collect();
        if tokens.is_empty() {
            continue;
        }
        if tokens[0].eq_ignore_ascii_case(".equ") {
            if tokens.len() != 3 {
                return Err(error("Expected .equ NAME VALUE"));
            }
            let value = number(tokens[2], &symbols).map_err(|e| error(&e))?;
            define(&mut symbols, tokens[1], value).map_err(|e| error(&e))?;
        } else if tokens[0].eq_ignore_ascii_case(".org") {
            if tokens.len() != 2 {
                return Err(error("Expected .org ADDRESS"));
            }
            let target = number(tokens[1], &symbols).map_err(|e| error(&e))?;
            if !(pc as i64..=65535).contains(&target) {
                return Err(error(".org must move forward within 64K words"));
            }
            pc = target as usize;
        } else {
            let count = if tokens[0].eq_ignore_ascii_case(".word") {
                tokens.len() - 1
            } else {
                1
            };
            if count == 0 || pc + count > 65536 {
                return Err(error("Program exceeds 64K words or empty .word"));
            }
            lines.push((line + 1, pc, tokens));
            pc += count;
        }
    }
    let mut words = vec![0; pc];
    for (line, at, tokens) in lines {
        let result: Result<Vec<u32>, String> = (|| {
            if tokens[0].eq_ignore_ascii_case(".word") {
                return tokens[1..]
                    .iter()
                    .map(|s| {
                        let n = number(s, &symbols)?;
                        if !(0..=u32::MAX as i64).contains(&n) {
                            return Err(".word requires an unsigned 32-bit value".into());
                        }
                        Ok(n as u32)
                    })
                    .collect();
            }
            let instruction = *ALL
                .iter()
                .find(|i| i.name.eq_ignore_ascii_case(tokens[0]))
                .ok_or_else(|| format!("Unknown instruction {}", tokens[0]))?;
            let args = &tokens[1..];
            let count = match instruction.format {
                Format::None => 0,
                Format::Ri | Format::Ai => 2,
                Format::Rri | Format::Rrr => 3,
                Format::I | Format::A => 1,
            };
            if args.len() != count {
                return Err(format!("{} expects {count} operands", instruction.name));
            }
            let reg = |s: &str| -> Result<u32, String> {
                let n = s
                    .strip_prefix('r')
                    .or_else(|| s.strip_prefix('R'))
                    .and_then(|s| s.parse::<u32>().ok())
                    .filter(|n| *n < 8);
                n.ok_or_else(|| format!("Invalid register {s}; use r0-r7"))
            };
            let imm = |s: &str, address: bool| -> Result<u32, String> {
                let n = number(s, &symbols)?;
                if !(if address { 0 } else { -32768 }..=65535).contains(&n) {
                    return Err(format!("Immediate out of range: {s}"));
                }
                Ok(n as u16 as u32)
            };
            let mut word = instruction.opcode << 27;
            word |= match instruction.format {
                Format::None => 0,
                Format::Ri => (reg(args[0])? << 24) | imm(args[1], instruction.name == "JAL")?,
                Format::Rri => (reg(args[0])? << 24) | (reg(args[1])? << 21) | imm(args[2], false)?,
                Format::Rrr => {
                    (reg(args[0])? << 24) | (reg(args[1])? << 21) | (reg(args[2])? << 18)
                }
                Format::Ai => (reg(args[0])? << 21) | imm(args[1], true)?,
                Format::I => imm(args[0], true)?,
                Format::A => reg(args[0])? << 21,
            };
            Ok(vec![word])
        })();
        let encoded = result.map_err(|e| format!("Line {line}: {e}"))?;
        words[at..at + encoded.len()].copy_from_slice(&encoded);
    }
    if words.is_empty() {
        return Err("Program is empty".into());
    }
    Ok(Assembly { words, symbols })
}
fn define(symbols: &mut BTreeMap<String, i64>, name: &str, value: i64) -> Result<(), String> {
    if name.is_empty()
        || !name
            .chars()
            .enumerate()
            .all(|(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
    {
        return Err(format!("Invalid symbol {name}"));
    }
    if symbols.insert(name.into(), value).is_some() {
        return Err(format!("Duplicate symbol {name}"));
    }
    Ok(())
}
fn number(text: &str, symbols: &BTreeMap<String, i64>) -> Result<i64, String> {
    if let Some(n) = symbols.get(text) {
        return Ok(*n);
    }
    let (negative, text) = text.strip_prefix('-').map_or((false, text), |t| (true, t));
    let value = if let Some(s) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        i64::from_str_radix(s, 16)
    } else if let Some(s) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
        i64::from_str_radix(s, 2)
    } else {
        text.parse()
    };
    value
        .ok()
        .and_then(|n: i64| if negative { n.checked_neg() } else { Some(n) })
        .ok_or_else(|| format!("Unknown symbol or invalid number {text}"))
}
pub fn hex(words: &[u32]) -> String {
    let mut text = String::from("v2.0 raw\n");
    for row in words.chunks(8) {
        text.push_str(
            &row.iter()
                .map(|w| format!("{w:08x}"))
                .collect::<Vec<_>>()
                .join(" "),
        );
        text.push('\n');
    }
    text
}
