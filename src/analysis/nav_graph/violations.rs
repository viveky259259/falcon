use super::{DanglingRef, Edge, Screen, UnguardedSensitive};
use std::collections::HashSet;

pub(super) fn find(
    screens: &[Screen],
    edges: &[Edge],
    initial_location: Option<&str>,
) -> (Vec<String>, Vec<DanglingRef>, Vec<UnguardedSensitive>) {
    let route_names: HashSet<_> = screens
        .iter()
        .filter_map(|screen| screen.route_name.as_deref())
        .collect();
    let dangling = edges
        .iter()
        .filter(|edge| !edge.dynamic)
        .filter_map(|edge| {
            let route_name = edge.to_route_name.as_deref()?;
            (!route_names.contains(route_name)).then(|| DanglingRef {
                route_name: route_name.to_string(),
                file: edge.file.clone(),
                line: edge.line,
            })
        })
        .collect();
    let orphans = find_orphans(screens, edges, initial_location);
    let unguarded = screens.iter().filter_map(sensitive_violation).collect();
    (orphans, dangling, unguarded)
}

fn find_orphans(screens: &[Screen], edges: &[Edge], initial_location: Option<&str>) -> Vec<String> {
    let initial_id = initial_location
        .and_then(|location| {
            screens
                .iter()
                .find(|screen| screen.route_path.as_deref() == Some(location))
        })
        .or_else(|| screens.first())
        .map(|screen| screen.id.as_str());
    let incoming: HashSet<_> = edges
        .iter()
        .filter_map(|edge| edge.to_screen.as_deref())
        .collect();
    screens
        .iter()
        .filter(|screen| Some(screen.id.as_str()) != initial_id)
        .filter(|screen| !incoming.contains(screen.id.as_str()))
        .map(|screen| {
            screen
                .route_name
                .clone()
                .unwrap_or_else(|| screen.id.clone())
        })
        .collect()
}

fn sensitive_violation(screen: &Screen) -> Option<UnguardedSensitive> {
    if screen.require_auth || screen.has_redirect {
        return None;
    }
    let haystack = format!(
        "{} {} {}",
        screen.widget,
        screen.route_name.as_deref().unwrap_or_default(),
        screen.route_path.as_deref().unwrap_or_default()
    )
    .to_lowercase();
    let matched = ["admin", "payment", "billing", "checkout", "account"]
        .into_iter()
        .find(|keyword| haystack.contains(keyword))?;
    Some(UnguardedSensitive {
        screen: screen.id.clone(),
        matched: matched.to_string(),
        reason: "sensitive route has no require_auth flag or redirect".to_string(),
    })
}
