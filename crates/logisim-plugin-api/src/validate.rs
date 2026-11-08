use super::*;
use std::collections::BTreeSet;
type Result = std::result::Result<(), String>;
fn require(ok: bool, message: &str) -> Result {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn label(s: &str) -> bool {
    !s.is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control)
}
fn point(p: [i32; 2]) -> bool {
    p.into_iter().all(|n| (-4096..=4096).contains(&n))
}
fn rect(a: [i32; 2], b: [i32; 2]) -> bool {
    point(a) && point(b) && a[0] < b[0] && a[1] < b[1]
}
pub fn validate_graphics(shapes: &[Shape]) -> Result {
    require(shapes.len() <= 256, "Too many plugin graphics.")?;
    for shape in shapes {
        let valid = match shape {
            Shape::Rect { min, max, .. } | Shape::Ellipse { min, max, .. } => rect(*min, *max),
            Shape::Line { a, b, .. } => point(*a) && point(*b),
            Shape::Text { at, text, size, .. } => {
                point(*at) && (6..=64).contains(size) && text.len() <= 1024 && !text.contains('\0')
            }
        };
        require(valid, "Invalid plugin graphic.")?;
    }
    Ok(())
}
impl Signal {
    pub fn validate(&self) -> Result {
        require(
            (1..=32).contains(&self.width),
            "Invalid plugin signal width.",
        )?;
        let mask = u32::MAX >> (32 - self.width);
        require(
            (self.value | self.unknown | self.error) & !mask == 0
                && self.value & (self.unknown | self.error) == 0
                && self.unknown & self.error == 0,
            "Invalid plugin signal masks.",
        )
    }
}
impl Property {
    pub fn accepts(&self, value: &str) -> bool {
        if value.len() > 1024 {
            return false;
        }
        match &self.kind {
            PropertyKind::Boolean => matches!(value, "true" | "false"),
            PropertyKind::Integer { min, max } => value
                .parse::<i64>()
                .is_ok_and(|n| (*min..=*max).contains(&n)),
            PropertyKind::Choice { values } => values.iter().any(|v| v == value),
        }
    }
}
impl Tool {
    pub fn validate_evaluation(&self, request: &Evaluation) -> Result {
        self.validate_request(
            &request.tool,
            &request.properties,
            &request.inputs,
            &request.state,
        )?;
        if let Event::Action { id, argument } = &request.event {
            require(
                argument.len() <= 1024
                    && self
                        .actions
                        .iter()
                        .any(|a| a.id == *id && (a.argument.is_some() || argument.is_empty())),
                "Invalid plugin action.",
            )?;
        }
        Ok(())
    }
    pub(crate) fn validate_request(
        &self,
        tool: &str,
        properties: &Properties,
        inputs: &[Signal],
        state: &[u8],
    ) -> Result {
        self.validate_properties(properties)?;
        require(
            tool == self.id && inputs.len() == self.ports.len() && state.len() <= MAX_STATE,
            "Invalid plugin evaluation request.",
        )?;
        for (v, p) in inputs.iter().zip(&self.ports) {
            v.validate()?;
            require(v.width == p.width, "Invalid plugin input width.")?;
        }
        Ok(())
    }
    pub fn validate(&self) -> Result {
        require(
            id(&self.id) && label(&self.name) && rect(self.bounds[0], self.bounds[1]),
            "Invalid plugin tool identity or bounds.",
        )?;
        require(
            self.ports.len() <= 64 && self.properties.len() <= 32 && self.actions.len() <= 16,
            "Plugin tool exceeds interface limits.",
        )?;
        let mut ids = BTreeSet::new();
        let mut positions = BTreeSet::new();
        for p in &self.ports {
            require(
                id(&p.id)
                    && ids.insert(&p.id)
                    && point(p.at)
                    && positions.insert(p.at)
                    && p.at.iter().all(|n| n % 10 == 0)
                    && (1..=32).contains(&p.width)
                    && label(&p.label),
                "Invalid or duplicate plugin port.",
            )?;
        }
        ids.clear();
        for p in &self.properties {
            require(
                id(&p.id) && ids.insert(&p.id) && label(&p.label) && p.accepts(&p.default),
                "Invalid plugin property.",
            )?;
            match &p.kind {
                PropertyKind::Integer { min, max } => {
                    require(min <= max, "Invalid plugin integer range.")?
                }
                PropertyKind::Choice { values } => require(
                    !values.is_empty()
                        && values.len() <= 32
                        && values.iter().all(|v| label(v))
                        && values.iter().collect::<BTreeSet<_>>().len() == values.len(),
                    "Invalid plugin choices.",
                )?,
                _ => {}
            }
        }
        ids.clear();
        for a in &self.actions {
            require(
                id(&a.id)
                    && ids.insert(&a.id)
                    && label(&a.label)
                    && a.argument.as_ref().is_none_or(|s| label(s)),
                "Invalid plugin action.",
            )?;
        }
        validate_graphics(&self.graphics)
    }
    pub fn validate_properties(&self, properties: &Properties) -> Result {
        require(
            properties.len() == self.properties.len()
                && self
                    .properties
                    .iter()
                    .all(|p| properties.get(&p.id).is_some_and(|v| p.accepts(v))),
            "Plugin properties do not match the schema.",
        )
    }
    pub fn validate_update(&self, update: &Update) -> Result {
        require(
            update.state.len() <= MAX_STATE,
            "Invalid plugin output/state size.",
        )?;
        self.validate_outputs(&update.outputs, &update.graphics)
    }
    pub(crate) fn validate_outputs(&self, outputs: &[Signal], graphics: &[Shape]) -> Result {
        require(
            outputs.len() == self.ports.len(),
            "Invalid plugin output/state size.",
        )?;
        for (port, value) in self.ports.iter().zip(outputs) {
            value.validate()?;
            require(
                port.width == value.width,
                "Plugin output width does not match its port.",
            )?;
        }
        validate_graphics(graphics)
    }
}
impl Component {
    pub fn validate(&self) -> Result {
        require(
            id(&self.plugin) && id(&self.version),
            "Invalid plugin identity/version.",
        )?;
        self.tool.validate()?;
        self.tool.validate_properties(&self.properties)
    }
}
impl Manifest {
    pub fn validate(&self) -> Result {
        require(supports_abi(self.abi), "Unsupported plugin ABI version.")?;
        require(
            id(&self.id)
                && id(&self.version)
                && label(&self.name)
                && !self.tools.is_empty()
                && self.tools.len() <= 64,
            "Invalid plugin manifest.",
        )?;
        let mut ids = BTreeSet::new();
        for t in &self.tools {
            require(ids.insert(&t.id), "Duplicate plugin tool ID.")?;
            t.validate()?;
        }
        Ok(())
    }
}
