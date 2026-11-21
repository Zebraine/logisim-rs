//! Versioned native UTF-8 document. Local IDs and derived interfaces never cross this boundary.
mod component;
#[cfg(test)]
mod tests;
use self::component::Part;
use crate::{Error, MAX_BYTES, MAX_CIRCUIT_ITEMS, MAX_ITEMS, Result, valid_point};
use logisim_core::{
    AgentIdKind as Tag, Appearance, Artwork, Circuit, CircuitLabel, Facing, Geometry, Id,
    MAX_CIRCUITS, Paint, Point, Project, plugin::PluginSource,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const FORMAT: &str = "logisim-rs-project";

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Native {
    format: String,
    version: u32,
    main: String,
    circuits: Vec<Definition>,
    plugin_sources: Vec<PluginSource>,
}
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct Definition {
    agent_id: String,
    name: String,
    label: CircuitLabel,
    appearance: Option<Package>,
    components: Vec<Part>,
    wires: Vec<Segment>,
}
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct Segment {
    agent_id: String,
    from: Point,
    to: Point,
}
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct Package {
    anchor: Point,
    facing: Facing,
    shapes: Vec<Shape>,
    ports: Vec<Binding>,
}
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct Shape {
    geometry: Geometry,
    paint: Paint,
}
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct Binding {
    pin: String,
    at: Point,
}

fn identity(kind: Tag, id: Option<u128>, context: impl std::fmt::Display) -> Result<String> {
    id.map(|id| kind.encode(id))
        .ok_or_else(|| Error(format!("{context}: missing agent_id")))
}
fn register(kind: Tag, id: &str, used: &mut BTreeSet<String>) -> Result<u128> {
    let value = kind
        .decode(id)
        .ok_or_else(|| Error(format!("Invalid {kind:?} agent_id: {id}")))?;
    if !used.insert(id.to_owned()) {
        return Err(Error(format!("Duplicate agent_id: {id}")));
    }
    Ok(value)
}
fn wire_valid(a: Point, b: Point) -> bool {
    valid_point(a) && valid_point(b) && a != b && (a.x == b.x || a.y == b.y)
}
fn budget(components: usize, wires: usize, total: &mut usize) -> Result<()> {
    *total += components + wires;
    if components + wires > MAX_CIRCUIT_ITEMS || *total > MAX_ITEMS {
        return Err(Error("Project or circuit item budget exceeded".into()));
    }
    Ok(())
}
impl Native {
    fn encode(project: &Project) -> Result<Self> {
        project.validate().map_err(Error)?;
        let names: BTreeMap<_, _> = project
            .circuits
            .iter()
            .map(|d| {
                Ok((
                    d.id,
                    identity(Tag::Circuit, d.circuit.agent_ids.circuit, &d.name)?,
                ))
            })
            .collect::<Result<_>>()?;
        let mut circuits = Vec::new();
        let mut used = BTreeSet::new();
        let mut total = 0;
        for d in &project.circuits {
            let c = &d.circuit;
            budget(c.components.len(), c.wires.len(), &mut total)?;
            register(Tag::Circuit, &names[&d.id], &mut used)?;
            let tag = |kind, id| {
                identity(
                    kind,
                    c.agent_ids.items.get(&id).copied(),
                    format_args!("{} item {id}", d.name),
                )
            };
            let mut components = vec![];
            for part in &c.components {
                let id = tag(Tag::Component, part.id)?;
                register(Tag::Component, &id, &mut used)?;
                component::validate(part).map_err(|e| Error(format!("{} {id}: {e}", d.name)))?;
                components.push(Part::encode(part, id, &names)?);
            }
            let mut wires = vec![];
            let mut segments = BTreeSet::new();
            for w in &c.wires {
                let agent_id = tag(Tag::Wire, w.id)?;
                register(Tag::Wire, &agent_id, &mut used)?;
                if !wire_valid(w.a, w.b) || !segments.insert((w.a.min(w.b), w.a.max(w.b))) {
                    return Err(Error(format!("{agent_id}: invalid or duplicate wire")));
                }
                wires.push(Segment {
                    agent_id,
                    from: w.a,
                    to: w.b,
                });
            }
            let appearance = c
                .appearance
                .as_ref()
                .map(|a| -> Result<_> {
                    let mut ports = a
                        .ports
                        .iter()
                        .map(|(&id, &at)| {
                            Ok(Binding {
                                pin: tag(Tag::Component, id)?,
                                at,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    ports.sort_by(|a, b| a.pin.cmp(&b.pin));
                    Ok(Package {
                        anchor: a.anchor,
                        facing: a.facing,
                        ports,
                        shapes: a
                            .shapes
                            .iter()
                            .map(|s| Shape {
                                geometry: s.geometry.clone(),
                                paint: s.paint.clone(),
                            })
                            .collect(),
                    })
                })
                .transpose()?;
            validate_label(&c.label)?;
            circuits.push(Definition {
                agent_id: names[&d.id].clone(),
                name: d.name.clone(),
                label: c.label.clone(),
                appearance,
                components,
                wires,
            });
        }
        circuits.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
        Ok(Self {
            format: FORMAT.into(),
            version: 1,
            main: names[&project.main].clone(),
            circuits,
            plugin_sources: project.plugin_sources.clone(),
        })
    }
    fn decode(self) -> Result<Project> {
        if self.format != FORMAT {
            return Err(Error(format!(
                "Unknown native project format: {}",
                self.format
            )));
        }
        if self.version != 1 {
            return Err(Error(format!(
                "Unsupported native project version {}",
                self.version
            )));
        }
        if self.circuits.is_empty() || self.circuits.len() > MAX_CIRCUITS {
            return Err(Error("Invalid number of circuits".into()));
        }
        let mut total = 0;
        let mut used = BTreeSet::new();
        let mut names = BTreeMap::new();
        let mut p = Project::single(&self.circuits[0].name, Circuit::default());
        for (i, d) in self.circuits.iter().enumerate() {
            budget(d.components.len(), d.wires.len(), &mut total)?;
            register(Tag::Circuit, &d.agent_id, &mut used)?;
            let id = if i == 0 {
                p.main
            } else {
                p.add(d.name.clone()).map_err(Error)?
            };
            names.insert(d.agent_id.clone(), id);
        }
        p.main = *names
            .get(&self.main)
            .ok_or_else(|| Error(format!("Unknown main circuit {}", self.main)))?;
        p.plugin_sources = self.plugin_sources;
        for d in self.circuits {
            let c = &mut p.circuit_mut(names[&d.agent_id]).unwrap().circuit;
            c.agent_ids.circuit = Tag::Circuit.decode(&d.agent_id);
            validate_label(&d.label)?;
            c.label = d.label;
            let mut pins = BTreeMap::new();
            for part in d.components {
                let tag = register(Tag::Component, &part.agent_id, &mut used)?;
                let id = part.agent_id.clone();
                let value = part
                    .decode(&names)
                    .and_then(|c| {
                        component::validate(&c)?;
                        Ok(c)
                    })
                    .map_err(|e| Error(format!("{} {id}: {e}", d.name)))?;
                let pin = value.kind == logisim_core::Kind::Pin;
                let local = c.add_component(value);
                if pin {
                    pins.insert(id, local);
                }
                c.agent_ids.items.insert(local, tag);
            }
            for w in d.wires {
                let tag = register(Tag::Wire, &w.agent_id, &mut used)?;
                if !wire_valid(w.from, w.to) {
                    return Err(Error(format!("{}: invalid wire coordinates", w.agent_id)));
                }
                let id = c.add_wire(w.from, w.to).expect("checked wire geometry");
                if c.agent_ids.items.insert(id, tag).is_some() {
                    return Err(Error(format!("{}: duplicate wire segment", w.agent_id)));
                }
            }
            if let Some(a) = d.appearance {
                let mut ports = BTreeMap::new();
                for b in a.ports {
                    let id = pins.get(&b.pin).ok_or_else(|| {
                        Error(format!("{}: unknown appearance pin {}", d.name, b.pin))
                    })?;
                    if ports.insert(*id, b.at).is_some() {
                        return Err(Error(format!("Duplicate appearance pin {}", b.pin)));
                    }
                }
                c.appearance = Some(Appearance {
                    anchor: a.anchor,
                    facing: a.facing,
                    ports,
                    shapes: a
                        .shapes
                        .into_iter()
                        .enumerate()
                        .map(|(i, s)| Artwork {
                            id: i as Id + 1,
                            geometry: s.geometry,
                            paint: s.paint,
                        })
                        .collect(),
                });
            }
        }
        p.validate().map_err(Error)?;
        p.refresh_interfaces(false);
        Ok(p)
    }
}
fn validate_label(l: &CircuitLabel) -> Result<()> {
    if l.text.len() > 65536
        || l.font.family.is_empty()
        || l.font.family.len() > 256
        || l.font.size == 0
    {
        return Err(Error("Invalid circuit label/font".into()));
    }
    Ok(())
}
pub fn parse_native(bytes: &[u8]) -> Result<Project> {
    if bytes.len() > MAX_BYTES {
        return Err(Error("Native project exceeds 8 MiB".into()));
    }
    serde_json::from_slice::<Native>(bytes)
        .map_err(|e| Error(format!("Invalid .lrs JSON: {e}")))?
        .decode()
}
pub fn serialize_native(project: &Project) -> Result<Vec<u8>> {
    let mut out =
        serde_json::to_vec_pretty(&Native::encode(project)?).map_err(|e| Error(e.to_string()))?;
    out.push(b'\n');
    if out.len() > MAX_BYTES {
        return Err(Error("Native project exceeds 8 MiB".into()));
    }
    Ok(out)
}
