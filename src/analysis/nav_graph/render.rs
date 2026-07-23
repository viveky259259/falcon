use super::{MermaidScope, NavGraph, NavGraphDiff, Screen};
use anyhow::Result;
use std::collections::HashSet;
use std::fmt::Write;

pub(super) fn mermaid(graph: &NavGraph, scope: MermaidScope<'_>) -> Result<String> {
    let selected: Vec<_> = match scope {
        MermaidScope::Feature(feature) => graph
            .screens
            .iter()
            .filter(|screen| screen_in_feature(screen, feature))
            .collect(),
        MermaidScope::Top(limit) => graph.screens.iter().take(limit).collect(),
    };
    let selected_ids: HashSet<_> = selected.iter().map(|screen| screen.id.as_str()).collect();
    let mut output = String::from("flowchart TD\n");

    for screen in selected {
        writeln!(
            output,
            "    {}[\"{}\"]",
            mermaid_id(&screen.id),
            escape_label(&screen.widget)
        )?;
    }
    for edge in &graph.edges {
        let Some(target) = edge.to_screen.as_deref() else {
            continue;
        };
        if selected_ids.contains(edge.from.as_str()) && selected_ids.contains(target) {
            writeln!(
                output,
                "    {} -->|{}| {}",
                mermaid_id(&edge.from),
                escape_label(&edge.via),
                mermaid_id(target)
            )?;
        }
    }
    Ok(output)
}

pub(super) fn pr_comment(graph_diff: &NavGraphDiff) -> String {
    let mut output = String::from("## Navigation graph changes\n\n");
    write_screens(&mut output, "Added screens", &graph_diff.added_screens);
    write_screens(&mut output, "Removed screens", &graph_diff.removed_screens);
    if !graph_diff.changed.is_empty() {
        output.push_str("### Changed contracts\n\n");
        for change in &graph_diff.changed {
            let _ = writeln!(
                output,
                "- `{}`: `{}` changed from `{}` to `{}`",
                change.screen, change.field, change.from, change.to
            );
        }
        output.push('\n');
    }
    if !graph_diff.broke.is_empty() {
        output.push_str("### broken navigation edges\n\n");
        for edge in &graph_diff.broke {
            let _ = writeln!(
                output,
                "- `{}` → `{}`: {}",
                edge.from, edge.to_route_name, edge.reason
            );
        }
        output.push('\n');
    }
    if graph_diff.added_screens.is_empty()
        && graph_diff.removed_screens.is_empty()
        && graph_diff.changed.is_empty()
        && graph_diff.broke.is_empty()
    {
        output.push_str("No navigation contract changes detected.\n");
    }
    output
}

fn write_screens(output: &mut String, heading: &str, screens: &[Screen]) {
    if screens.is_empty() {
        return;
    }
    let _ = writeln!(output, "### {heading}\n");
    for screen in screens {
        let route = screen
            .route_name
            .as_deref()
            .or(screen.route_path.as_deref())
            .unwrap_or("unnamed");
        let _ = writeln!(output, "- `{route}` ({})", screen.widget);
    }
    output.push('\n');
}

fn screen_in_feature(screen: &Screen, feature: &str) -> bool {
    screen
        .file
        .components()
        .any(|component| component.as_os_str().to_string_lossy() == feature)
}

fn mermaid_id(id: &str) -> String {
    id.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn escape_label(label: &str) -> String {
    label.replace('"', "&quot;")
}
