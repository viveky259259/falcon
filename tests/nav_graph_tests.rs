// Contract tests for `falcon nav-graph`.
//
// These tests pin the public API described in the v2 proposal:
//   - JSON contract (schema version, Screen/Edge/violation shapes)
//   - Build (extract from go_router + FFRoute + static routeName/routePath)
//   - Diff (added/removed/changed/broke)
//   - Check (CI gate semantics)
//   - Render (mermaid scope rules, pr-comment format)
//
// Implementation is stubbed in src/analysis/nav_graph/. These tests fail
// until the extractor, differ, checker, and renderers are wired up.

use std::path::Path;

use falcon::analysis::nav_graph::{
    build_graph, check, diff, render_mermaid, render_pr_comment, CheckOptions, EdgeKind,
    MermaidScope, Router, SCHEMA_VERSION,
};

// ─── helpers ────────────────────────────────────────────────────────────────

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn fixture_simple_go_router() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");

    write(
        &lib.join("pages/home/home_page.dart"),
        r#"
import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

class HomePage extends StatelessWidget {
  static String routeName = 'home';
  static String routePath = '/home';

  Widget build(BuildContext context) {
    return ElevatedButton(
      onPressed: () => context.pushNamed('editProfile'),
      child: const Text('Edit'),
    );
  }
}
"#,
    );

    write(
        &lib.join("pages/edit/edit_profile_page.dart"),
        r#"
import 'package:flutter/material.dart';

class EditProfilePage extends StatelessWidget {
  static String routeName = 'editProfile';
  static String routePath = '/editProfile';

  Widget build(BuildContext context) => const Placeholder();
}
"#,
    );

    write(
        &lib.join("router.dart"),
        r#"
import 'package:go_router/go_router.dart';

final router = GoRouter(
  initialLocation: '/home',
  routes: [
    GoRoute(name: 'home',         path: '/home',         builder: (_, __) => HomePage()),
    GoRoute(name: 'editProfile',  path: '/editProfile',  builder: (_, __) => EditProfilePage()),
  ],
);
"#,
    );

    tmp
}

// ─── build: schema + router detection ────────────────────────────────────────

#[test]
fn schema_version_is_pinned_to_one() {
    assert_eq!(SCHEMA_VERSION, 1, "schema version is the public contract");
}

#[test]
fn detects_go_router_when_imported() {
    let fx = fixture_simple_go_router();
    let g = build_graph(fx.path()).unwrap();
    assert_eq!(g.version, 1);
    assert!(matches!(g.router, Router::GoRouter));
}

#[test]
fn captures_initial_location() {
    let fx = fixture_simple_go_router();
    let g = build_graph(fx.path()).unwrap();
    assert_eq!(g.initial_location.as_deref(), Some("/home"));
}

// ─── build: screen extraction ───────────────────────────────────────────────

#[test]
fn extracts_screens_from_static_route_members() {
    let fx = fixture_simple_go_router();
    let g = build_graph(fx.path()).unwrap();
    let names: Vec<_> = g.screens.iter().map(|s| s.widget.as_str()).collect();
    assert!(names.contains(&"HomePage"), "got {:?}", names);
    assert!(names.contains(&"EditProfilePage"), "got {:?}", names);

    let home = g.screens.iter().find(|s| s.widget == "HomePage").unwrap();
    assert_eq!(home.route_name.as_deref(), Some("home"));
    assert_eq!(home.route_path.as_deref(), Some("/home"));
}

#[test]
fn screen_id_is_stable_across_runs() {
    let fx = fixture_simple_go_router();
    let a = build_graph(fx.path()).unwrap();
    let b = build_graph(fx.path()).unwrap();
    let ids_a: Vec<_> = a.screens.iter().map(|s| s.id.clone()).collect();
    let ids_b: Vec<_> = b.screens.iter().map(|s| s.id.clone()).collect();
    assert_eq!(ids_a, ids_b);
}

// ─── build: edge extraction ─────────────────────────────────────────────────

#[test]
fn extracts_push_named_edge() {
    let fx = fixture_simple_go_router();
    let g = build_graph(fx.path()).unwrap();
    let edge = g
        .edges
        .iter()
        .find(|e| e.to_route_name.as_deref() == Some("editProfile"))
        .unwrap_or_else(|| panic!("expected edge to editProfile, got {:?}", g.edges));
    assert_eq!(edge.kind, EdgeKind::Push);
    assert!(edge.via.contains("pushNamed"), "via was {:?}", edge.via);
    assert!(!edge.dynamic);
}

#[test]
fn extracts_context_go_literal_path_edge() {
    let tmp = tempfile::tempdir().unwrap();
    write(
        &tmp.path().join("lib/router.dart"),
        r#"
import 'package:go_router/go_router.dart';
final router = GoRouter(initialLocation: '/home', routes: [
  GoRoute(name: 'home', path: '/home', builder: (_, __) => HomePage()),
]);
"#,
    );
    write(
        &tmp.path().join("lib/home.dart"),
        r#"
class HomePage extends StatelessWidget {
  static String routeName = 'home';
  static String routePath = '/home';
  void onTap(BuildContext c) => c.go('/home');
}
"#,
    );
    let g = build_graph(tmp.path()).unwrap();
    assert!(
        g.edges.iter().any(|e| e.kind == EdgeKind::Go),
        "got {:?}",
        g.edges
    );
}

#[test]
fn marks_dynamic_edge_when_route_name_is_interpolated() {
    let tmp = tempfile::tempdir().unwrap();
    write(
        &tmp.path().join("lib/router.dart"),
        r#"
final router = GoRouter(initialLocation: '/home', routes: [
  GoRoute(name: 'home', path: '/home', builder: (_, __) => HomePage()),
]);
"#,
    );
    write(
        &tmp.path().join("lib/home.dart"),
        r#"
import 'package:go_router/go_router.dart';
class HomePage extends StatelessWidget {
  static String routeName = 'home';
  static String routePath = '/home';
  void open(BuildContext c, String tab) => c.pushNamed('$tab/edit');
}
"#,
    );
    let g = build_graph(tmp.path()).unwrap();
    assert!(
        g.edges.iter().any(|e| e.dynamic),
        "interpolated names must be marked dynamic; got {:?}",
        g.edges
    );
}

// ─── build: FFRoute (FlutterFlow) ───────────────────────────────────────────

#[test]
fn extracts_ffroute_list_with_require_auth() {
    let tmp = tempfile::tempdir().unwrap();
    write(
        &tmp.path().join("lib/flutter_flow/nav/nav.dart"),
        r#"
final routes = [
  FFRoute(name: 'login', path: '/login', requireAuth: false),
  FFRoute(name: 'admin', path: '/admin', requireAuth: true),
];
"#,
    );
    let g = build_graph(tmp.path()).unwrap();
    let admin = g
        .screens
        .iter()
        .find(|s| s.route_name.as_deref() == Some("admin"))
        .expect("admin route must be extracted from FFRoute list");
    assert!(admin.require_auth);
}

#[test]
fn detects_redirect_callback_on_route() {
    let tmp = tempfile::tempdir().unwrap();
    write(
        &tmp.path().join("lib/router.dart"),
        r#"
final routes = [
  GoRoute(
    name: 'profile',
    path: '/profile',
    redirect: (ctx, st) => appStateNotifier.shouldRedirect ? '/login' : null,
    builder: (_, __) => ProfilePage(),
  ),
];
"#,
    );
    let g = build_graph(tmp.path()).unwrap();
    let profile = g
        .screens
        .iter()
        .find(|s| s.route_name.as_deref() == Some("profile"))
        .unwrap();
    assert!(profile.has_redirect);
}

// ─── build: custom auth-extension wrappers ──────────────────────────────────

#[test]
fn push_named_auth_extension_call_is_treated_as_push_named() {
    let tmp = tempfile::tempdir().unwrap();
    write(
        &tmp.path().join("lib/router.dart"),
        r#"
final routes = [
  GoRoute(name: 'cart', path: '/cart', builder: (_, __) => CartPage()),
];
"#,
    );
    write(
        &tmp.path().join("lib/home.dart"),
        r#"
class HomePage extends StatelessWidget {
  static String routeName = 'home';
  static String routePath = '/home';
  void open(BuildContext c) => c.pushNamedAuth('cart');
}
"#,
    );
    let g = build_graph(tmp.path()).unwrap();
    assert!(
        g.edges
            .iter()
            .any(|e| e.to_route_name.as_deref() == Some("cart") && e.kind == EdgeKind::Push),
        "pushNamedAuth must resolve to a Push edge; got {:?}",
        g.edges
    );
}

// ─── violations: orphans, dangling, unguarded sensitive ─────────────────────

#[test]
fn flags_orphan_route_when_declared_but_never_invoked() {
    let tmp = tempfile::tempdir().unwrap();
    write(
        &tmp.path().join("lib/router.dart"),
        r#"
final routes = [
  GoRoute(name: 'home',     path: '/home',     builder: (_, __) => HomePage()),
  GoRoute(name: 'settings', path: '/settings', builder: (_, __) => SettingsPage()),
];
"#,
    );
    write(
        &tmp.path().join("lib/home.dart"),
        r#"
class HomePage extends StatelessWidget {
  static String routeName = 'home';
  static String routePath = '/home';
}
class SettingsPage extends StatelessWidget {
  static String routeName = 'settings';
  static String routePath = '/settings';
}
"#,
    );
    let g = build_graph(tmp.path()).unwrap();
    assert!(
        g.orphans.iter().any(|o| o.contains("settings")),
        "settings is unreachable from initial; got {:?}",
        g.orphans
    );
    assert!(
        !g.orphans.iter().any(|o| o.contains("home")),
        "home is the initial location; cannot be orphan"
    );
}

#[test]
fn flags_dangling_when_push_named_targets_undeclared_route() {
    let tmp = tempfile::tempdir().unwrap();
    write(
        &tmp.path().join("lib/router.dart"),
        r#"
final routes = [
  GoRoute(name: 'home', path: '/home', builder: (_, __) => HomePage()),
];
"#,
    );
    write(
        &tmp.path().join("lib/home.dart"),
        r#"
import 'package:go_router/go_router.dart';
class HomePage extends StatelessWidget {
  static String routeName = 'home';
  static String routePath = '/home';
  void open(BuildContext c) => c.pushNamed('checkout');
}
"#,
    );
    let g = build_graph(tmp.path()).unwrap();
    assert!(
        g.dangling.iter().any(|d| d.route_name == "checkout"),
        "checkout is undeclared; got {:?}",
        g.dangling
    );
}

#[test]
fn flags_unguarded_admin_route() {
    let tmp = tempfile::tempdir().unwrap();
    write(
        &tmp.path().join("lib/router.dart"),
        r#"
final routes = [
  GoRoute(name: 'home', path: '/home', builder: (_, __) => HomePage()),
  GoRoute(name: 'adminPanel', path: '/admin', builder: (_, __) => AdminPanel()),
];
"#,
    );
    write(
        &tmp.path().join("lib/admin.dart"),
        r#"
class AdminPanel extends StatelessWidget {
  static String routeName = 'adminPanel';
  static String routePath = '/admin';
}
"#,
    );
    let g = build_graph(tmp.path()).unwrap();
    assert!(
        g.unguarded_sensitive
            .iter()
            .any(|u| u.matched.to_lowercase().contains("admin")),
        "adminPanel has no requireAuth and no redirect; got {:?}",
        g.unguarded_sensitive
    );
}

#[test]
fn admin_route_with_redirect_is_not_unguarded() {
    let tmp = tempfile::tempdir().unwrap();
    write(
        &tmp.path().join("lib/router.dart"),
        r#"
final routes = [
  GoRoute(name: 'home', path: '/home', builder: (_, __) => HomePage()),
  GoRoute(
    name: 'adminPanel',
    path: '/admin',
    redirect: (c, s) => isAdmin() ? null : '/login',
    builder: (_, __) => AdminPanel(),
  ),
];
"#,
    );
    write(
        &tmp.path().join("lib/admin.dart"),
        r#"
class AdminPanel extends StatelessWidget {
  static String routeName = 'adminPanel';
  static String routePath = '/admin';
}
"#,
    );
    let g = build_graph(tmp.path()).unwrap();
    assert!(
        g.unguarded_sensitive.is_empty(),
        "redirect makes adminPanel guarded; got {:?}",
        g.unguarded_sensitive
    );
}

// ─── stats ──────────────────────────────────────────────────────────────────

#[test]
fn stats_match_collected_counts() {
    let fx = fixture_simple_go_router();
    let g = build_graph(fx.path()).unwrap();
    assert_eq!(g.stats.screens, g.screens.len());
    assert_eq!(g.stats.edges, g.edges.len());
    assert_eq!(
        g.stats.dynamic_edges,
        g.edges.iter().filter(|e| e.dynamic).count()
    );
}

// ─── diff ───────────────────────────────────────────────────────────────────

#[test]
fn diff_detects_added_and_removed_screens() {
    let mut old = falcon::analysis::nav_graph::NavGraph::default();
    old.screens.push(falcon::analysis::nav_graph::Screen {
        id: "scr:HomePage".into(),
        widget: "HomePage".into(),
        file: "lib/home.dart".into(),
        line: 1,
        route_name: Some("home".into()),
        route_path: Some("/home".into()),
        require_auth: false,
        has_redirect: false,
        description: None,
    });
    let mut new = old.clone();
    new.screens.push(falcon::analysis::nav_graph::Screen {
        id: "scr:CartPage".into(),
        widget: "CartPage".into(),
        file: "lib/cart.dart".into(),
        line: 1,
        route_name: Some("cart".into()),
        route_path: Some("/cart".into()),
        require_auth: false,
        has_redirect: false,
        description: None,
    });

    let d = diff(&old, &new);
    assert!(d.added_screens.iter().any(|s| s.widget == "CartPage"));
    assert!(d.removed_screens.is_empty());
}

#[test]
fn diff_detects_changed_require_auth() {
    let mut old = falcon::analysis::nav_graph::NavGraph::default();
    old.screens.push(falcon::analysis::nav_graph::Screen {
        id: "scr:Profile".into(),
        widget: "Profile".into(),
        file: "lib/p.dart".into(),
        line: 1,
        route_name: Some("profile".into()),
        route_path: Some("/profile".into()),
        require_auth: false,
        has_redirect: false,
        description: None,
    });
    let mut new = old.clone();
    new.screens[0].require_auth = true;

    let d = diff(&old, &new);
    assert!(
        d.changed.iter().any(|c| c.field == "require_auth"),
        "must surface require_auth flip; got {:?}",
        d.changed
    );
}

#[test]
fn diff_detects_broken_edge_when_target_route_removed() {
    let edge = falcon::analysis::nav_graph::Edge {
        from: "scr:HomePage".into(),
        to_route_name: Some("settings".into()),
        to_screen: Some("scr:Settings".into()),
        kind: EdgeKind::Push,
        via: "context.pushNamed".into(),
        file: "lib/home.dart".into(),
        line: 10,
        dynamic: false,
    };
    let mut old = falcon::analysis::nav_graph::NavGraph::default();
    old.screens.push(falcon::analysis::nav_graph::Screen {
        id: "scr:Settings".into(),
        widget: "Settings".into(),
        file: "lib/s.dart".into(),
        line: 1,
        route_name: Some("settings".into()),
        route_path: Some("/settings".into()),
        require_auth: false,
        has_redirect: false,
        description: None,
    });
    old.edges.push(edge.clone());

    let mut new = old.clone();
    new.screens.retain(|s| s.id != "scr:Settings");
    // edge still references settings → broken in new graph
    let d = diff(&old, &new);
    assert!(
        d.broke
            .iter()
            .any(|b| b.from == "scr:HomePage" && b.to_route_name == "settings"),
        "removing target route must mark edge broken; got {:?}",
        d.broke
    );
}

// ─── check (CI gate) ────────────────────────────────────────────────────────

#[test]
fn check_passes_on_clean_graph() {
    let g = falcon::analysis::nav_graph::NavGraph::default();
    let report = check(&g, None);
    assert!(!report.is_failure(CheckOptions::default()));
}

#[test]
fn check_fails_on_dangling() {
    let mut g = falcon::analysis::nav_graph::NavGraph::default();
    g.dangling.push(falcon::analysis::nav_graph::DanglingRef {
        route_name: "checkout".into(),
        file: "lib/cart.dart".into(),
        line: 45,
    });
    let report = check(&g, None);
    assert!(report.is_failure(CheckOptions::default()));
    assert_eq!(report.dangling.len(), 1);
}

#[test]
fn check_fails_on_unguarded_sensitive() {
    let mut g = falcon::analysis::nav_graph::NavGraph::default();
    g.unguarded_sensitive
        .push(falcon::analysis::nav_graph::UnguardedSensitive {
            screen: "scr:AdminPanel".into(),
            matched: "admin".into(),
            reason: "no require_auth, no redirect".into(),
        });
    let report = check(&g, None);
    assert!(report.is_failure(CheckOptions::default()));
}

#[test]
fn check_orphans_warn_by_default_fail_under_strict() {
    let mut g = falcon::analysis::nav_graph::NavGraph::default();
    g.orphans.push("scr:Lonely".into());
    let report = check(&g, None);
    assert!(!report.is_failure(CheckOptions::default()));
    assert!(report.is_failure(CheckOptions { strict: true }));
}

// ─── render: mermaid scope rules ────────────────────────────────────────────

#[test]
fn mermaid_top_n_emits_at_most_n_screens() {
    // Build a graph with 60 screens; ask for top 10.
    let mut g = falcon::analysis::nav_graph::NavGraph::default();
    for i in 0..60 {
        g.screens.push(falcon::analysis::nav_graph::Screen {
            id: format!("scr:P{}", i),
            widget: format!("P{}", i),
            file: format!("lib/p{}.dart", i).into(),
            line: 1,
            route_name: Some(format!("p{}", i)),
            route_path: Some(format!("/p{}", i)),
            require_auth: false,
            has_redirect: false,
            description: None,
        });
    }
    let out = render_mermaid(&g, MermaidScope::Top(10)).unwrap();
    let nodes = out.matches("scr:P").count();
    assert!(
        nodes <= 10,
        "top-10 scope must cap nodes at 10; got {} in:\n{}",
        nodes,
        out
    );
}

#[test]
fn mermaid_feature_scope_filters_by_path() {
    let mut g = falcon::analysis::nav_graph::NavGraph::default();
    g.screens.push(falcon::analysis::nav_graph::Screen {
        id: "scr:Login".into(),
        widget: "LoginPage".into(),
        file: "lib/features/auth/login.dart".into(),
        line: 1,
        route_name: Some("login".into()),
        route_path: Some("/login".into()),
        require_auth: false,
        has_redirect: false,
        description: None,
    });
    g.screens.push(falcon::analysis::nav_graph::Screen {
        id: "scr:Cart".into(),
        widget: "CartPage".into(),
        file: "lib/features/cart/cart.dart".into(),
        line: 1,
        route_name: Some("cart".into()),
        route_path: Some("/cart".into()),
        require_auth: false,
        has_redirect: false,
        description: None,
    });
    let out = render_mermaid(&g, MermaidScope::Feature("auth")).unwrap();
    assert!(out.contains("LoginPage") || out.contains("scr:Login"));
    assert!(
        !out.contains("CartPage") && !out.contains("scr:Cart"),
        "auth feature must not include cart screens; got:\n{}",
        out
    );
}

// ─── render: pr-comment ─────────────────────────────────────────────────────

#[test]
fn pr_comment_summarizes_added_removed_and_broke() {
    let mut d = falcon::analysis::nav_graph::NavGraphDiff {
        version: SCHEMA_VERSION,
        ..Default::default()
    };
    d.added_screens.push(falcon::analysis::nav_graph::Screen {
        id: "scr:CartPage".into(),
        widget: "CartPage".into(),
        file: "lib/cart.dart".into(),
        line: 1,
        route_name: Some("cart".into()),
        route_path: Some("/cart".into()),
        require_auth: false,
        has_redirect: false,
        description: None,
    });
    d.removed_screens.push(falcon::analysis::nav_graph::Screen {
        id: "scr:LegacyLogin".into(),
        widget: "LegacyLoginPage".into(),
        file: "lib/legacy.dart".into(),
        line: 1,
        route_name: Some("legacyLogin".into()),
        route_path: Some("/legacy".into()),
        require_auth: false,
        has_redirect: false,
        description: None,
    });
    d.broke.push(falcon::analysis::nav_graph::BrokenEdge {
        from: "scr:HomePage".into(),
        to_route_name: "settings".into(),
        reason: "target route removed".into(),
    });

    let md = render_pr_comment(&d);
    assert!(md.contains("Navigation"), "pr comment must have a heading");
    assert!(md.contains("cart"), "added cart must appear");
    assert!(
        md.contains("legacyLogin"),
        "removed legacyLogin must appear"
    );
    assert!(
        md.contains("broken") || md.contains("broke"),
        "broken edges must surface; got:\n{}",
        md
    );
}

// ─── JSON contract round-trip ───────────────────────────────────────────────

#[test]
fn graph_json_round_trip_preserves_schema_version() {
    let g = falcon::analysis::nav_graph::NavGraph::default();
    let json = serde_json::to_string(&g).unwrap();
    let parsed: falcon::analysis::nav_graph::NavGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.version, SCHEMA_VERSION);
}

#[test]
fn graph_json_uses_snake_case_for_enums() {
    let g = falcon::analysis::nav_graph::NavGraph {
        router: Router::GoRouter,
        ..Default::default()
    };
    let json = serde_json::to_string(&g).unwrap();
    assert!(
        json.contains("\"go_router\""),
        "router enum must serialize as snake_case for the wire contract; got {}",
        json
    );
}
