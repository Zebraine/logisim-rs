//! Bounded .circ XML primitives shared by project, component and appearance codecs.
use crate::{Error, Result, attributes::Attributes, valid_point};
use logisim_core::Point;
use roxmltree::Node;
use std::{collections::BTreeMap, fmt::Write};

pub(crate) fn attr<'a>(n: Node<'a, '_>, name: &str) -> Result<&'a str> {
    n.attribute(name)
        .ok_or_else(|| Error(format!("<{}> is missing {name}", n.tag_name().name())))
}
pub(crate) fn attributes(n: Node<'_, '_>) -> Result<Attributes> {
    let mut result = BTreeMap::new();
    for a in n.children().filter(Node::is_element) {
        if !a.has_tag_name("a") || a.children().any(|n| n.is_element()) {
            return Err(Error(format!(
                "Unsupported <{}> content",
                a.tag_name().name()
            )));
        }
        let key = attr(a, "name")?;
        let value = a.attribute("val").map(str::to_owned).unwrap_or_else(|| {
            a.children()
                .filter(Node::is_text)
                .filter_map(|n| n.text())
                .collect()
        });
        if result.insert(key.to_owned(), value).is_some() {
            return Err(Error(format!("Duplicate attribute: {key}")));
        }
    }
    Ok(result)
}
pub(crate) fn point(s: &str) -> Result<Point> {
    let parsed = s
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .and_then(|s| s.split_once(','))
        .and_then(|(x, y)| Some(Point::new(x.trim().parse().ok()?, y.trim().parse().ok()?)));
    parsed
        .filter(|p| valid_point(*p))
        .ok_or_else(|| Error(format!("Invalid or out-of-range coordinate: {s}")))
}
pub(crate) fn escape(s: &str) -> Result<String> {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c if c >= '\u{20}' && c != '\u{fffe}' && c != '\u{ffff}' => out.push(c),
            _ => {
                return Err(Error(
                    "Text contains a character XML cannot represent.".into(),
                ));
            }
        }
    }
    Ok(out)
}
pub(crate) fn attribute(out: &mut String, name: &str, value: impl std::fmt::Display) -> Result<()> {
    writeln!(
        out,
        "      <a name=\"{name}\" val=\"{}\"/>",
        escape(&value.to_string())?
    )
    .unwrap();
    Ok(())
}
