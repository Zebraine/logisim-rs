//! The vector subset emitted by the observed appearance editor, not a general SVG importer.
use crate::{Error, Result, write::escape};
use logisim_core::{
    Appearance, ArtworkFont, Circuit, Facing, Geometry, Kind, Paint, Point, TextAnchor,
};
use roxmltree::Node;
use std::{collections::BTreeMap, fmt::Write};

fn required<'a>(n: Node<'a, '_>, key: &str) -> Result<&'a str> {
    n.attribute(key)
        .ok_or_else(|| Error(format!("Appearance {} missing {key}", n.tag_name().name())))
}
fn number(s: &str) -> Result<f64> {
    s.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && v.abs() <= 2_000_000.0)
        .ok_or_else(|| Error(format!("Invalid appearance number {s}")))
}
fn integer(v: f64) -> Result<i32> {
    if v.fract() == 0.0 && v.abs() <= 1_000_000.0 {
        Ok(v as i32)
    } else {
        Err(Error(
            "Unsupported fractional/out-of-range appearance coordinate".into(),
        ))
    }
}
fn n(n: Node<'_, '_>, key: &str) -> Result<f64> {
    number(required(n, key)?)
}
fn xy(node: Node<'_, '_>) -> Result<Point> {
    Ok(Point::new(integer(n(node, "x")?)?, integer(n(node, "y")?)?))
}
fn pair(s: &str) -> Result<Point> {
    let (x, y) = s
        .split_once(',')
        .ok_or_else(|| Error("Invalid appearance coordinate pair".into()))?;
    Ok(Point::new(integer(number(x)?)?, integer(number(y)?)?))
}
fn color(s: &str) -> Result<Option<[u8; 3]>> {
    if s == "none" {
        return Ok(None);
    }
    let rgb = s
        .strip_prefix('#')
        .filter(|s| s.len() == 6)
        .and_then(|s| u32::from_str_radix(s, 16).ok())
        .ok_or_else(|| Error("Unsupported appearance color".into()))?;
    Ok(Some([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]))
}
fn facing(s: &str) -> Result<Facing> {
    match s {
        "east" => Ok(Facing::East),
        "south" => Ok(Facing::South),
        "west" => Ok(Facing::West),
        "north" => Ok(Facing::North),
        _ => Err(Error("Invalid appearance facing".into())),
    }
}
fn allowed(node: Node<'_, '_>, keys: &[&str]) -> Result<()> {
    for a in node.attributes() {
        if !keys.contains(&a.name()) {
            return Err(Error(format!(
                "Unsupported appearance attribute {}",
                a.name()
            )));
        }
    }
    if node.children().any(|n| n.is_element()) {
        return Err(Error("Nested appearance objects are unsupported".into()));
    }
    Ok(())
}
pub fn read(node: Node<'_, '_>, circuit: &Circuit) -> Result<Appearance> {
    let mut result = Appearance {
        shapes: vec![],
        ports: BTreeMap::new(),
        anchor: Point::ZERO,
        facing: Facing::East,
    };
    let mut anchor = false;
    for object in node.children().filter(Node::is_element) {
        let tag = object.tag_name().name();
        if tag == "circ-port" || tag == "circ-anchor" {
            allowed(
                object,
                if tag == "circ-port" {
                    &["x", "y", "width", "height", "pin"]
                } else {
                    &["x", "y", "width", "height", "facing"]
                },
            )?;
            let a = xy(object)?;
            let width = integer(n(object, "width")?)?;
            let height = integer(n(object, "height")?)?;
            let at = Point::new(a.x + width / 2, a.y + height / 2);
            if tag == "circ-anchor" {
                if anchor || width != 6 || height != 6 {
                    return Err(Error("Invalid/duplicate appearance anchor".into()));
                }
                anchor = true;
                result.anchor = at;
                result.facing = facing(required(object, "facing")?)?;
            } else {
                let pos = pair(required(object, "pin")?)?;
                let pins: Vec<_> = circuit
                    .components
                    .iter()
                    .filter(|p| p.kind == Kind::Pin && p.position == pos)
                    .collect();
                if pins.len() != 1 {
                    return Err(Error("Missing or ambiguous appearance Pin binding".into()));
                }
                let pin = pins[0];
                let size = if pin.output { 10 } else { 8 };
                if width != size || height != size || result.ports.insert(pin.id, at).is_some() {
                    return Err(Error("Invalid or duplicate appearance port".into()));
                }
            }
            continue;
        }
        let geometry = match tag {
            "rect" => {
                allowed(
                    object,
                    &[
                        "x",
                        "y",
                        "width",
                        "height",
                        "rx",
                        "ry",
                        "stroke",
                        "stroke-width",
                        "fill",
                    ],
                )?;
                let a = xy(object)?;
                let width = integer(n(object, "width")?)?;
                let height = integer(n(object, "height")?)?;
                if width <= 0 || height <= 0 {
                    return Err(Error("Invalid appearance rectangle size".into()));
                }
                let rx = integer(number(object.attribute("rx").unwrap_or("0"))?)?;
                let ry = integer(number(object.attribute("ry").unwrap_or("0"))?)?;
                if rx != ry {
                    return Err(Error("Unequal corner radii are not supported".into()));
                }
                Geometry::Rect {
                    a,
                    b: a + Point::new(width, height),
                    radius: rx,
                }
            }
            "ellipse" => {
                allowed(
                    object,
                    &["cx", "cy", "rx", "ry", "stroke", "stroke-width", "fill"],
                )?;
                let (x, y, rx, ry) = (
                    n(object, "cx")?,
                    n(object, "cy")?,
                    n(object, "rx")?,
                    n(object, "ry")?,
                );
                if rx <= 0.0 || ry <= 0.0 {
                    return Err(Error("Invalid appearance ellipse size".into()));
                }
                Geometry::Ellipse {
                    a: Point::new(integer(x - rx)?, integer(y - ry)?),
                    b: Point::new(integer(x + rx)?, integer(y + ry)?),
                }
            }
            "polyline" | "polygon" => {
                allowed(object, &["points", "stroke", "stroke-width", "fill"])?;
                let points = required(object, "points")?
                    .split_whitespace()
                    .map(pair)
                    .collect::<Result<Vec<_>>>()?;
                Geometry::Poly {
                    points,
                    closed: tag == "polygon",
                }
            }
            "path" => {
                allowed(object, &["d", "stroke", "stroke-width", "fill"])?;
                let d = required(object, "d")?;
                let (start, rest) = d
                    .strip_prefix('M')
                    .and_then(|s| s.split_once('Q'))
                    .ok_or_else(|| {
                        Error("Only observed quadratic appearance paths are supported".into())
                    })?;
                let points = rest
                    .split_whitespace()
                    .map(pair)
                    .collect::<Result<Vec<_>>>()?;
                if points.len() != 2 {
                    return Err(Error("Invalid quadratic appearance path".into()));
                }
                Geometry::Curve {
                    a: pair(start.trim())?,
                    control: points[0],
                    b: points[1],
                }
            }
            "text" => {
                allowed(
                    object,
                    &[
                        "x",
                        "y",
                        "font-family",
                        "font-size",
                        "font-weight",
                        "font-style",
                        "text-anchor",
                        "fill",
                    ],
                )?;
                let size = integer(n(object, "font-size")?)?;
                if !(1..=96).contains(&size) {
                    return Err(Error("Unsupported appearance font size".into()));
                }
                let bold = match object.attribute("font-weight") {
                    None | Some("normal") => false,
                    Some("bold") => true,
                    _ => return Err(Error("Unsupported font weight".into())),
                };
                let italic = match object.attribute("font-style") {
                    None | Some("normal") => false,
                    Some("italic") => true,
                    _ => return Err(Error("Unsupported font style".into())),
                };
                let anchor = match required(object, "text-anchor")? {
                    "start" => TextAnchor::Start,
                    "middle" => TextAnchor::Middle,
                    "end" => TextAnchor::End,
                    _ => return Err(Error("Unsupported text anchor".into())),
                };
                Geometry::Text {
                    at: xy(object)?,
                    text: object
                        .children()
                        .filter(Node::is_text)
                        .filter_map(|n| n.text())
                        .collect(),
                    font: ArtworkFont {
                        family: required(object, "font-family")?.into(),
                        size: size as u8,
                        bold,
                        italic,
                    },
                    anchor,
                }
            }
            _ => return Err(Error(format!("Unsupported appearance object <{tag}>"))),
        };
        let width = integer(number(object.attribute("stroke-width").unwrap_or("1"))?)?;
        if !(1..=64).contains(&width) {
            return Err(Error("Unsupported appearance stroke width".into()));
        }
        let paint = Paint {
            stroke: color(object.attribute("stroke").unwrap_or("none"))?,
            fill: color(object.attribute("fill").unwrap_or("#000000"))?,
            width: width as u8,
        };
        if result.add(geometry, paint).is_none() {
            return Err(Error("Invalid or oversized appearance artwork".into()));
        }
    }
    if !anchor {
        return Err(Error("Missing appearance anchor".into()));
    }
    result
        .validate(
            &circuit
                .components
                .iter()
                .filter(|p| p.kind == Kind::Pin)
                .map(|p| p.id)
                .collect(),
        )
        .map_err(Error)?;
    Ok(result)
}
fn rgb(c: Option<[u8; 3]>) -> String {
    c.map(|c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]))
        .unwrap_or_else(|| "none".into())
}
pub fn write(out: &mut String, appearance: &Appearance, circuit: &Circuit) -> Result<()> {
    let mut positions = std::collections::BTreeSet::new();
    if circuit
        .components
        .iter()
        .filter(|p| p.kind == Kind::Pin)
        .any(|p| !positions.insert(p.position))
    {
        return Err(Error(
            "Custom appearance Pins must have unique internal positions before saving.".into(),
        ));
    }
    out.push_str("    <appear>\n");
    for shape in &appearance.shapes {
        let p = &shape.paint;
        let paint = format!(
            " fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"",
            rgb(p.fill),
            rgb(p.stroke),
            p.width
        );
        match &shape.geometry {
            Geometry::Rect { a, b, radius } => {
                writeln!(out,"      <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{radius}\" ry=\"{radius}\"{paint}/>",a.x.min(b.x),a.y.min(b.y),(a.x-b.x).abs(),(a.y-b.y).abs()).unwrap();
            }
            Geometry::Ellipse { a, b } => {
                writeln!(
                    out,
                    "      <ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"{paint}/>",
                    (a.x + b.x) as f64 / 2.0,
                    (a.y + b.y) as f64 / 2.0,
                    (a.x - b.x).abs() as f64 / 2.0,
                    (a.y - b.y).abs() as f64 / 2.0
                )
                .unwrap();
            }
            Geometry::Poly { points, closed } => {
                let tag = if *closed { "polygon" } else { "polyline" };
                let points = points
                    .iter()
                    .map(|p| format!("{},{}", p.x, p.y))
                    .collect::<Vec<_>>()
                    .join(" ");
                writeln!(out, "      <{tag} points=\"{points}\"{paint}/>").unwrap();
            }
            Geometry::Curve { a, control, b } => {
                writeln!(
                    out,
                    "      <path d=\"M{},{} Q{},{} {},{}\"{paint}/>",
                    a.x, a.y, control.x, control.y, b.x, b.y
                )
                .unwrap();
            }
            Geometry::Text {
                at,
                text,
                font,
                anchor,
            } => {
                let anchor = match anchor {
                    TextAnchor::Start => "start",
                    TextAnchor::Middle => "middle",
                    TextAnchor::End => "end",
                };
                writeln!(out,"      <text x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"{}\"{}{} text-anchor=\"{anchor}\" fill=\"{}\">{}</text>",at.x,at.y,escape(&font.family)?,font.size,if font.bold {" font-weight=\"bold\""}else{""},if font.italic {" font-style=\"italic\""}else{""},rgb(p.fill),escape(text)?).unwrap();
            }
        }
    }
    for (id, at) in &appearance.ports {
        let pin = circuit
            .component(*id)
            .ok_or_else(|| Error("Missing appearance pin".into()))?;
        let size = if pin.output { 10 } else { 8 };
        writeln!(
            out,
            "      <circ-port pin=\"{},{}\" x=\"{}\" y=\"{}\" width=\"{size}\" height=\"{size}\"/>",
            pin.position.x,
            pin.position.y,
            at.x - size / 2,
            at.y - size / 2
        )
        .unwrap();
    }
    writeln!(out,"      <circ-anchor x=\"{}\" y=\"{}\" width=\"6\" height=\"6\" facing=\"{}\"/>\n    </appear>",appearance.anchor.x-3,appearance.anchor.y-3,appearance.facing.name().to_ascii_lowercase()).unwrap();
    Ok(())
}
