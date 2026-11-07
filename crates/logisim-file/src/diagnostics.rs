use std::collections::BTreeMap;

#[derive(Default)]
struct Group {
    count: usize,
    locations: Vec<String>,
}

/// Aggregate repeated display notes without hiding later electrical omissions.
#[derive(Default)]
pub(crate) struct Warnings(BTreeMap<(bool, String), Group>);

impl Warnings {
    pub fn add(&mut self, message: String) {
        self.0.entry((false, message)).or_default().count += 1;
    }

    pub fn component(
        &mut self,
        circuit: &str,
        kind: &str,
        location: String,
        reason: &str,
        omitted: bool,
    ) {
        let action = if omitted { "Skipped " } else { "" };
        let key = format!("Circuit '{circuit}': {action}{kind}: {reason}");
        let group = self.0.entry((!omitted, key)).or_default();
        group.count += 1;
        if group.locations.len() < 3 {
            group.locations.push(location);
        }
    }

    pub fn finish(self) -> Vec<String> {
        let mut result = vec![];
        let mut remaining = (0, 0);
        for ((_, message), group) in self.0 {
            if result.len() == 200 {
                remaining.0 += 1;
                remaining.1 += group.count;
                continue;
            }
            let mut message = message;
            if group.count > 1 {
                message.push_str(&format!(" ({} occurrences)", group.count));
            }
            if !group.locations.is_empty() {
                message.push_str(&format!("; at {}", group.locations.join(", ")));
            }
            result.push(message);
        }
        if remaining.0 > 0 {
            result.push(format!(
                "{} further diagnostic groups ({} occurrences) are not listed.",
                remaining.0, remaining.1
            ));
        }
        result
    }
}
