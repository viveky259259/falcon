use super::syntax::{call_bodies, capture, line_number, route_widget_name, screen_id};
use super::{Edge, EdgeKind, GraphStats, NavGraph, Router, Screen, SCHEMA_VERSION};
use anyhow::{Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug)]
struct DartFile {
    path: PathBuf,
    source: String,
    classes: Vec<ClassSpan>,
}

#[derive(Debug, Clone)]
struct ClassSpan {
    widget: String,
    start: usize,
    end: usize,
    line: usize,
    route_name: Option<String>,
    route_path: Option<String>,
}

#[derive(Debug)]
struct RouteDecl {
    name: Option<String>,
    path: Option<String>,
    widget: Option<String>,
    require_auth: bool,
    has_redirect: bool,
    file: PathBuf,
    line: usize,
}

pub(super) fn build_graph(project_root: &Path) -> Result<NavGraph> {
    let files = read_dart_files(project_root)?;
    let router = detect_router(&files);
    let initial_location = find_initial_location(&files);
    let mut screens = collect_class_screens(&files);

    for declaration in collect_route_declarations(&files)? {
        merge_route(&mut screens, declaration);
    }
    screens.sort_by(|left, right| left.id.cmp(&right.id));

    let mut edges = collect_edges(&files, &screens)?;
    edges.sort_by(|left, right| {
        (&left.from, &left.to_route_name, &left.via, left.line).cmp(&(
            &right.from,
            &right.to_route_name,
            &right.via,
            right.line,
        ))
    });
    edges.dedup_by(|left, right| {
        left.from == right.from
            && left.to_route_name == right.to_route_name
            && left.via == right.via
            && left.file == right.file
            && left.line == right.line
    });

    let (orphans, dangling, unguarded_sensitive) =
        super::violations::find(&screens, &edges, initial_location.as_deref());
    let stats = GraphStats {
        screens: screens.len(),
        edges: edges.len(),
        dynamic_edges: edges.iter().filter(|edge| edge.dynamic).count(),
    };

    Ok(NavGraph {
        version: SCHEMA_VERSION,
        router,
        initial_location,
        screens,
        edges,
        orphans,
        dangling,
        unguarded_sensitive,
        stats,
    })
}

fn read_dart_files(project_root: &Path) -> Result<Vec<DartFile>> {
    let lib = project_root.join("lib");
    let search_root = if lib.is_dir() {
        lib
    } else {
        project_root.to_path_buf()
    };
    let mut paths: Vec<_> = WalkDir::new(&search_root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "dart")
        })
        .collect();
    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("reading Dart source {}", path.display()))?;
            let relative = path
                .strip_prefix(project_root)
                .unwrap_or(&path)
                .to_path_buf();
            let classes = class_spans(&source)?;
            Ok(DartFile {
                path: relative,
                source,
                classes,
            })
        })
        .collect()
}

fn class_spans(source: &str) -> Result<Vec<ClassSpan>> {
    let class_re = Regex::new(r"(?m)\bclass\s+([A-Za-z_][A-Za-z0-9_]*)")?;
    let route_name_re =
        Regex::new(r#"\bstatic\s+(?:const\s+)?(?:String\s+)?routeName\s*=\s*['"]([^'"]+)['"]"#)?;
    let route_path_re =
        Regex::new(r#"\bstatic\s+(?:const\s+)?(?:String\s+)?routePath\s*=\s*['"]([^'"]+)['"]"#)?;
    let matches: Vec<_> = class_re.captures_iter(source).collect();
    let mut classes = Vec::with_capacity(matches.len());

    for (index, captures) in matches.iter().enumerate() {
        let whole = captures.get(0).expect("class regex has a whole match");
        let start = whole.start();
        let end = matches
            .get(index + 1)
            .and_then(|next| next.get(0))
            .map_or(source.len(), |next| next.start());
        let body = &source[start..end];
        classes.push(ClassSpan {
            widget: captures[1].to_string(),
            start,
            end,
            line: line_number(source, start),
            route_name: capture(&route_name_re, body),
            route_path: capture(&route_path_re, body),
        });
    }
    Ok(classes)
}

fn collect_class_screens(files: &[DartFile]) -> Vec<Screen> {
    files
        .iter()
        .flat_map(|file| {
            file.classes
                .iter()
                .filter(|class| class.route_name.is_some() || class.route_path.is_some())
                .map(|class| Screen {
                    id: screen_id(&class.widget),
                    widget: class.widget.clone(),
                    file: file.path.clone(),
                    line: class.line,
                    route_name: class.route_name.clone(),
                    route_path: class.route_path.clone(),
                    require_auth: false,
                    has_redirect: false,
                    description: None,
                })
        })
        .collect()
}

fn detect_router(files: &[DartFile]) -> Router {
    if files.iter().any(|file| {
        file.source.contains("package:go_router")
            || file.source.contains("GoRouter(")
            || file.source.contains("GoRoute(")
    }) {
        Router::GoRouter
    } else if files
        .iter()
        .any(|file| file.source.contains("AutoRoute(") || file.source.contains("@AutoRouterConfig"))
    {
        Router::AutoRoute
    } else if files.iter().any(|file| file.source.contains("Navigator.")) {
        Router::Navigator1
    } else {
        Router::Unknown
    }
}

fn find_initial_location(files: &[DartFile]) -> Option<String> {
    let regex = Regex::new(r#"\binitialLocation\s*:\s*['"]([^'"]+)['"]"#).ok()?;
    files.iter().find_map(|file| capture(&regex, &file.source))
}

fn collect_route_declarations(files: &[DartFile]) -> Result<Vec<RouteDecl>> {
    let name_re = Regex::new(r#"\bname\s*:\s*['"]([^'"]+)['"]"#)?;
    let path_re = Regex::new(r#"\bpath\s*:\s*['"]([^'"]+)['"]"#)?;
    let widget_re = Regex::new(
        r"(?s)\b(?:builder|pageBuilder)\s*:[^=]*=>\s*(?:const\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*\(",
    )?;
    let mut declarations = Vec::new();

    for file in files {
        for call_name in ["GoRoute", "FFRoute", "AutoRoute"] {
            for (start, body) in call_bodies(&file.source, call_name) {
                declarations.push(RouteDecl {
                    name: capture(&name_re, body),
                    path: capture(&path_re, body),
                    widget: capture(&widget_re, body),
                    require_auth: body.contains("requireAuth: true")
                        || body.contains("require_auth: true"),
                    has_redirect: body.contains("redirect:"),
                    file: file.path.clone(),
                    line: line_number(&file.source, start),
                });
            }
        }
    }
    Ok(declarations)
}

fn merge_route(screens: &mut Vec<Screen>, declaration: RouteDecl) {
    let existing = screens.iter_mut().find(|screen| {
        declaration
            .widget
            .as_ref()
            .is_some_and(|widget| screen.widget == *widget)
            || declaration
                .name
                .as_ref()
                .is_some_and(|name| screen.route_name.as_ref() == Some(name))
            || declaration
                .path
                .as_ref()
                .is_some_and(|path| screen.route_path.as_ref() == Some(path))
    });

    if let Some(screen) = existing {
        if declaration.name.is_some() {
            screen.route_name = declaration.name;
        }
        if declaration.path.is_some() {
            screen.route_path = declaration.path;
        }
        screen.require_auth |= declaration.require_auth;
        screen.has_redirect |= declaration.has_redirect;
        return;
    }

    let widget = declaration
        .widget
        .clone()
        .or_else(|| declaration.name.as_deref().map(route_widget_name))
        .or_else(|| declaration.path.as_deref().map(route_widget_name))
        .unwrap_or_else(|| "UnknownRoute".to_string());
    screens.push(Screen {
        id: screen_id(&widget),
        widget,
        file: declaration.file,
        line: declaration.line,
        route_name: declaration.name,
        route_path: declaration.path,
        require_auth: declaration.require_auth,
        has_redirect: declaration.has_redirect,
        description: None,
    });
}

fn collect_edges(files: &[DartFile], screens: &[Screen]) -> Result<Vec<Edge>> {
    let named_re = Regex::new(
        r#"\b(pushNamedAuth|pushNamed|goNamed|replaceNamed|pushReplacementNamed)\s*\(\s*['"]([^'"]*)['"]"#,
    )?;
    let path_re = Regex::new(r#"\b(go|push|replace)\s*\(\s*['"]([^'"]*)['"]"#)?;
    let navigator_re = Regex::new(
        r#"\bNavigator\s*\.\s*(pushNamed|pushReplacementNamed)\s*\([^,]+,\s*['"]([^'"]*)['"]"#,
    )?;
    let mut edges = Vec::new();

    for file in files {
        for captures in named_re.captures_iter(&file.source) {
            let whole = captures.get(0).expect("edge regex has a whole match");
            edges.push(named_edge(
                file,
                screens,
                whole.start(),
                &captures[1],
                &captures[2],
            ));
        }
        for captures in path_re.captures_iter(&file.source) {
            let whole = captures.get(0).expect("edge regex has a whole match");
            edges.push(path_edge(
                file,
                screens,
                whole.start(),
                &captures[1],
                &captures[2],
            ));
        }
        for captures in navigator_re.captures_iter(&file.source) {
            let whole = captures.get(0).expect("edge regex has a whole match");
            edges.push(named_edge(
                file,
                screens,
                whole.start(),
                &captures[1],
                &captures[2],
            ));
        }
    }
    Ok(edges)
}

fn named_edge(
    file: &DartFile,
    screens: &[Screen],
    start: usize,
    method: &str,
    route_name: &str,
) -> Edge {
    let dynamic = route_name.contains('$');
    let target = (!dynamic)
        .then(|| {
            screens
                .iter()
                .find(|screen| screen.route_name.as_deref() == Some(route_name))
        })
        .flatten();
    Edge {
        from: edge_source(file, screens, start),
        to_route_name: Some(route_name.to_string()),
        to_screen: target.map(|screen| screen.id.clone()),
        kind: method_kind(method),
        via: method.to_string(),
        file: file.path.clone(),
        line: line_number(&file.source, start),
        dynamic,
    }
}

fn path_edge(
    file: &DartFile,
    screens: &[Screen],
    start: usize,
    method: &str,
    route_path: &str,
) -> Edge {
    let dynamic = route_path.contains('$');
    let target = (!dynamic)
        .then(|| {
            screens
                .iter()
                .find(|screen| screen.route_path.as_deref() == Some(route_path))
        })
        .flatten();
    Edge {
        from: edge_source(file, screens, start),
        to_route_name: target.and_then(|screen| screen.route_name.clone()),
        to_screen: target.map(|screen| screen.id.clone()),
        kind: method_kind(method),
        via: method.to_string(),
        file: file.path.clone(),
        line: line_number(&file.source, start),
        dynamic,
    }
}

fn edge_source(file: &DartFile, screens: &[Screen], offset: usize) -> String {
    if let Some(class) = file
        .classes
        .iter()
        .find(|class| class.start <= offset && offset < class.end)
    {
        return screen_id(&class.widget);
    }
    screens
        .iter()
        .find(|screen| screen.file == file.path)
        .map_or_else(
            || format!("file:{}", file.path.display()),
            |screen| screen.id.clone(),
        )
}

fn method_kind(method: &str) -> EdgeKind {
    if method.starts_with("go") {
        EdgeKind::Go
    } else if method.contains("replace") || method.contains("Replacement") {
        EdgeKind::Replace
    } else {
        EdgeKind::Push
    }
}
