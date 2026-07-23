use super::{BrokenEdge, ChangedField, Edge, NavGraph, NavGraphDiff, Screen, SCHEMA_VERSION};
use serde_json::json;
use std::collections::{HashMap, HashSet};

pub(super) fn diff(old: &NavGraph, new: &NavGraph) -> NavGraphDiff {
    let old_screens: HashMap<_, _> = old
        .screens
        .iter()
        .map(|screen| (screen.id.as_str(), screen))
        .collect();
    let new_screens: HashMap<_, _> = new
        .screens
        .iter()
        .map(|screen| (screen.id.as_str(), screen))
        .collect();
    let mut result = NavGraphDiff {
        version: SCHEMA_VERSION,
        added_screens: new
            .screens
            .iter()
            .filter(|screen| !old_screens.contains_key(screen.id.as_str()))
            .cloned()
            .collect(),
        removed_screens: old
            .screens
            .iter()
            .filter(|screen| !new_screens.contains_key(screen.id.as_str()))
            .cloned()
            .collect(),
        ..NavGraphDiff::default()
    };

    for (id, old_screen) in &old_screens {
        if let Some(new_screen) = new_screens.get(id) {
            changed_fields(old_screen, new_screen, &mut result.changed);
        }
    }
    diff_edges(old, new, &mut result);
    find_broken_edges(old, new, &mut result.broke);
    sort_diff(&mut result);
    result
}

fn changed_fields(old: &Screen, new: &Screen, changed: &mut Vec<ChangedField>) {
    push_change(
        changed,
        old,
        "route_name",
        json!(old.route_name),
        json!(new.route_name),
    );
    push_change(
        changed,
        old,
        "route_path",
        json!(old.route_path),
        json!(new.route_path),
    );
    push_change(
        changed,
        old,
        "require_auth",
        json!(old.require_auth),
        json!(new.require_auth),
    );
    push_change(
        changed,
        old,
        "has_redirect",
        json!(old.has_redirect),
        json!(new.has_redirect),
    );
    push_change(
        changed,
        old,
        "description",
        json!(old.description),
        json!(new.description),
    );
}

fn push_change(
    changed: &mut Vec<ChangedField>,
    screen: &Screen,
    field: &str,
    from: serde_json::Value,
    to: serde_json::Value,
) {
    if from != to {
        changed.push(ChangedField {
            screen: screen.id.clone(),
            field: field.to_string(),
            from,
            to,
        });
    }
}

fn diff_edges(old: &NavGraph, new: &NavGraph, result: &mut NavGraphDiff) {
    let old_keys: HashSet<_> = old.edges.iter().map(edge_key).collect();
    let new_keys: HashSet<_> = new.edges.iter().map(edge_key).collect();
    result.added_edges = new
        .edges
        .iter()
        .filter(|edge| !old_keys.contains(&edge_key(edge)))
        .cloned()
        .collect();
    result.removed_edges = old
        .edges
        .iter()
        .filter(|edge| !new_keys.contains(&edge_key(edge)))
        .cloned()
        .collect();
}

fn find_broken_edges(old: &NavGraph, new: &NavGraph, broke: &mut Vec<BrokenEdge>) {
    let old_routes: HashSet<_> = old
        .screens
        .iter()
        .filter_map(|screen| screen.route_name.as_deref())
        .collect();
    let new_routes: HashSet<_> = new
        .screens
        .iter()
        .filter_map(|screen| screen.route_name.as_deref())
        .collect();
    for edge in &new.edges {
        let Some(route_name) = edge.to_route_name.as_deref() else {
            continue;
        };
        if old_routes.contains(route_name) && !new_routes.contains(route_name) {
            broke.push(BrokenEdge {
                from: edge.from.clone(),
                to_route_name: route_name.to_string(),
                reason: "target route removed".to_string(),
            });
        }
    }
}

fn edge_key(edge: &Edge) -> (&str, Option<&str>) {
    (&edge.from, edge.to_route_name.as_deref())
}

fn sort_diff(result: &mut NavGraphDiff) {
    result
        .added_screens
        .sort_by(|left, right| left.id.cmp(&right.id));
    result
        .removed_screens
        .sort_by(|left, right| left.id.cmp(&right.id));
    result
        .changed
        .sort_by(|left, right| (&left.screen, &left.field).cmp(&(&right.screen, &right.field)));
    result.broke.sort_by(|left, right| {
        (&left.from, &left.to_route_name).cmp(&(&right.from, &right.to_route_name))
    });
}
