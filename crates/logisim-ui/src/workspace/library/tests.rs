use super::*;
#[test]
fn catalogue_has_unique_stable_groups_and_no_duplicate_tools() {
    let mut ids = std::collections::BTreeSet::new();
    let mut tools = Vec::new();
    for group in groups() {
        assert!(ids.insert(group.key), "Duplicate group ID: {}", group.key);
        assert!(!group.tools.is_empty());
        for &kind in &group.tools {
            assert!(!tools.contains(&kind), "Duplicate library tool: {kind:?}");
            assert!(!matches!(kind, Kind::Subcircuit(_)));
            tools.push(kind);
        }
    }
}
