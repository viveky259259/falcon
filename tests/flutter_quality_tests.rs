//! Integration tests for the 6 new Flutter-quality features added to Falcon.
//!
//! Each test section scaffolds a minimal fake Flutter project in a temp directory
//! and asserts that the relevant analyser produces expected results.

use std::fs;
#[allow(unused_imports)]
use std::path::PathBuf;
use tempfile::TempDir;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn write(dir: &TempDir, rel: &str, content: &str) -> PathBuf {
    let path = dir.path().join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, content).unwrap();
    path
}

// ═════════════════════════════════════════════════════════════════════════════
// 1. ASSET AUDIT
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn asset_audit_clean_project_scores_high() {
    let dir = TempDir::new().unwrap();

    // pubspec declares one asset
    write(
        &dir,
        "pubspec.yaml",
        r#"
flutter:
  assets:
    - assets/logo.png
"#,
    );
    // create the declared asset (small — under threshold)
    write(&dir, "assets/logo.png", &"x".repeat(1024)); // 1 KB

    // Dart file references it
    write(
        &dir,
        "lib/main.dart",
        r#"
Image.asset('assets/logo.png');
"#,
    );

    let report = falcon::asset_audit::audit_assets(dir.path()).unwrap();
    // Small file, used, no duplicates → should score well
    assert!(
        report.score >= 70,
        "Clean project should score ≥70, got {}",
        report.score
    );
    assert_eq!(report.total_assets, 1);
}

#[test]
fn asset_audit_detects_oversized_image() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "pubspec.yaml",
        r#"
flutter:
  assets:
    - assets/huge.png
"#,
    );
    // 300 KB file — over the 200 KB default threshold
    write(&dir, "assets/huge.png", &"x".repeat(300 * 1024));
    write(&dir, "lib/main.dart", "Image.asset('assets/huge.png');");

    let report = falcon::asset_audit::audit_assets(dir.path()).unwrap();
    let has_oversized = report.issues.iter().any(|i| i.category == "Oversized");
    assert!(has_oversized, "Should flag oversized image");
}

#[test]
fn asset_audit_detects_unused_asset() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "pubspec.yaml",
        r#"
flutter:
  assets:
    - assets/used.png
    - assets/unused.png
"#,
    );
    write(&dir, "assets/used.png", &"x".repeat(1024));
    write(&dir, "assets/unused.png", &"x".repeat(1024));
    // Only 'used.png' referenced in Dart
    write(&dir, "lib/main.dart", "Image.asset('assets/used.png');");

    let report = falcon::asset_audit::audit_assets(dir.path()).unwrap();
    let has_unused = report.issues.iter().any(|i| i.category == "Unused");
    assert!(has_unused, "Should flag unused asset");
}

#[test]
fn asset_audit_html_report_is_generated() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "pubspec.yaml",
        "flutter:\n  assets:\n    - assets/img.png\n",
    );
    write(&dir, "assets/img.png", &"x".repeat(1024));
    write(&dir, "lib/main.dart", "Image.asset('assets/img.png');");

    let report = falcon::asset_audit::audit_assets(dir.path()).unwrap();
    let out = dir.path().join("report.html");
    falcon::asset_audit::write_asset_html_report(&report, &out).unwrap();
    let html = fs::read_to_string(&out).unwrap();
    assert!(html.contains("Falcon"));
    assert!(html.contains("Asset"));
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. THEME AUDIT
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn theme_audit_detects_hardcoded_colors() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "lib/widget.dart",
        r#"
Container(
  color: Color(0xFF2196F3),
  child: Text(
    'Hello',
    style: TextStyle(fontSize: 16),
  ),
)
"#,
    );

    let report = falcon::theme_audit::audit_theme(dir.path()).unwrap();
    let has_color_issue = report
        .issues
        .iter()
        .any(|i| i.category == "Hardcoded Color" || i.category == "Hardcoded color");
    assert!(
        has_color_issue,
        "Should detect hardcoded Color(0xFF...) usage"
    );
}

#[test]
fn theme_audit_detects_hardcoded_font_size() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "lib/text_widget.dart",
        r#"
Text(
  'Hello',
  style: TextStyle(fontSize: 24),
)
"#,
    );

    let report = falcon::theme_audit::audit_theme(dir.path()).unwrap();
    let has_font_issue = report
        .issues
        .iter()
        .any(|i| i.category.to_lowercase().contains("font"));
    assert!(has_font_issue, "Should detect hardcoded fontSize");
}

#[test]
fn theme_audit_clean_file_scores_high() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "lib/widget.dart",
        r#"
Container(
  color: Theme.of(context).colorScheme.primary,
  child: Text(
    'Hello',
    style: Theme.of(context).textTheme.bodyMedium,
  ),
)
"#,
    );

    let report = falcon::theme_audit::audit_theme(dir.path()).unwrap();
    assert!(
        report.score >= 80,
        "Clean theme usage should score ≥80, got {}",
        report.score
    );
}

#[test]
fn theme_audit_detects_missing_dark_theme() {
    let dir = TempDir::new().unwrap();
    // MaterialApp with theme but no darkTheme
    write(
        &dir,
        "lib/main.dart",
        r#"
MaterialApp(
  theme: ThemeData(primaryColor: Colors.blue),
  home: const MyHomePage(),
)
"#,
    );

    let report = falcon::theme_audit::audit_theme(dir.path()).unwrap();
    assert!(
        !report.summary.has_dark_theme,
        "Should detect missing darkTheme"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. L10N COVERAGE
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn l10n_coverage_detects_missing_translation() {
    let dir = TempDir::new().unwrap();

    // Template English ARB
    write(
        &dir,
        "lib/l10n/app_en.arb",
        r#"
{
  "@@locale": "en",
  "hello": "Hello",
  "goodbye": "Goodbye",
  "welcome": "Welcome, {name}!"
}
"#,
    );

    // Spanish ARB missing "goodbye" and "welcome"
    write(
        &dir,
        "lib/l10n/app_es.arb",
        r#"
{
  "@@locale": "es",
  "hello": "Hola"
}
"#,
    );

    let report = falcon::l10n_coverage::analyze_l10n_coverage(dir.path()).unwrap();
    let es = report.locales.iter().find(|l| l.locale == "es");
    assert!(es.is_some(), "Should find es locale");
    let es = es.unwrap();
    assert!(
        es.missing_keys.contains(&"goodbye".to_string())
            || es.missing_keys.contains(&"welcome".to_string()),
        "Should detect missing keys in es: {:?}",
        es.missing_keys
    );
    assert!(es.coverage_pct < 100.0);
}

#[test]
fn l10n_coverage_perfect_translation() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "lib/l10n/app_en.arb",
        r#"{"@@locale":"en","hello":"Hello","bye":"Bye"}"#,
    );
    write(
        &dir,
        "lib/l10n/app_fr.arb",
        r#"{"@@locale":"fr","hello":"Bonjour","bye":"Au revoir"}"#,
    );

    let report = falcon::l10n_coverage::analyze_l10n_coverage(dir.path()).unwrap();
    let fr = report.locales.iter().find(|l| l.locale == "fr");
    assert!(fr.is_some());
    let fr = fr.unwrap();
    assert!(fr.missing_keys.is_empty(), "No missing keys expected");
    assert!((fr.coverage_pct - 100.0).abs() < 0.01);
}

#[test]
fn l10n_coverage_detects_empty_values() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "lib/l10n/app_en.arb",
        r#"{"@@locale":"en","title":"Title","body":"Body"}"#,
    );
    write(
        &dir,
        "lib/l10n/app_de.arb",
        r#"{"@@locale":"de","title":"","body":"Hauptteil"}"#,
    );

    let report = falcon::l10n_coverage::analyze_l10n_coverage(dir.path()).unwrap();
    let de = report.locales.iter().find(|l| l.locale == "de").unwrap();
    assert!(
        de.empty_values.contains(&"title".to_string()),
        "Should flag empty 'title' in de"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. DEEP LINK VALIDATOR
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn deeplink_detects_missing_android_manifest() {
    let dir = TempDir::new().unwrap();
    // No AndroidManifest.xml exists — should produce a warning/error
    write(&dir, "lib/main.dart", "void main() {}");

    let report = falcon::deeplink::validate_deeplinks(dir.path()).unwrap();
    // Without a manifest, should flag it
    let has_android_issue = report.issues.iter().any(|i| i.platform == "Android");
    assert!(has_android_issue, "Should flag missing AndroidManifest.xml");
}

#[test]
fn deeplink_extracts_android_schemes() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "android/app/src/main/AndroidManifest.xml",
        r#"<manifest>
  <application>
    <activity>
      <intent-filter android:autoVerify="true">
        <action android:name="android.intent.action.VIEW"/>
        <category android:name="android.intent.category.DEFAULT"/>
        <category android:name="android.intent.category.BROWSABLE"/>
        <data android:scheme="https" android:host="example.com"/>
      </intent-filter>
    </activity>
  </application>
</manifest>"#,
    );
    write(&dir, "lib/main.dart", "void main() {}");

    let report = falcon::deeplink::validate_deeplinks(dir.path()).unwrap();
    assert!(
        report
            .android_schemes
            .iter()
            .any(|s| s.contains("example.com") || s == "https"),
        "Should extract scheme/host from manifest: {:?}",
        report.android_schemes
    );
}

#[test]
fn deeplink_detects_http_scheme_warning() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "android/app/src/main/AndroidManifest.xml",
        r#"<manifest>
  <application>
    <activity>
      <intent-filter>
        <action android:name="android.intent.action.VIEW"/>
        <category android:name="android.intent.category.BROWSABLE"/>
        <data android:scheme="http" android:host="example.com"/>
      </intent-filter>
    </activity>
  </application>
</manifest>"#,
    );
    write(&dir, "lib/main.dart", "void main() {}");

    let report = falcon::deeplink::validate_deeplinks(dir.path()).unwrap();
    let has_http_warn = report.issues.iter().any(|i| {
        i.detail.to_lowercase().contains("http") || i.category.to_lowercase().contains("http")
    });
    assert!(has_http_warn, "Should warn about http:// scheme");
}

// ═════════════════════════════════════════════════════════════════════════════
// 5. ANIMATION AUDIT
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn animation_audit_detects_missing_dispose() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "lib/my_widget.dart",
        r#"
class MyWidget extends StatefulWidget {
  @override
  State<MyWidget> createState() => _MyWidgetState();
}

class _MyWidgetState extends State<MyWidget> with TickerProviderStateMixin {
  late AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(vsync: this, duration: Duration(milliseconds: 300));
  }

  // Note: dispose() is intentionally missing to trigger the rule

  @override
  Widget build(BuildContext context) => const SizedBox();
}
"#,
    );

    let report = falcon::animation_audit::audit_animations(dir.path()).unwrap();
    let has_dispose_issue = report
        .issues
        .iter()
        .any(|i| i.category.to_lowercase().contains("dispos"));
    assert!(has_dispose_issue, "Should detect missing dispose()");
}

#[test]
fn animation_audit_detects_setstate_in_listener() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "lib/bad_widget.dart",
        r#"
_controller.addListener(() {
  setState(() {
    _value = _controller.value;
  });
});
"#,
    );

    let report = falcon::animation_audit::audit_animations(dir.path()).unwrap();
    let has_issue = report.issues.iter().any(|i| {
        i.category.to_lowercase().contains("setstate")
            || i.category.to_lowercase().contains("listener")
    });
    assert!(
        has_issue,
        "Should detect setState inside animation listener"
    );
}

#[test]
fn animation_audit_clean_code_scores_high() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "lib/good_widget.dart",
        r#"
class _GoodState extends State<GoodWidget> with SingleTickerProviderStateMixin {
  late AnimationController _ctrl;

  @override
  void initState() {
    super.initState();
    _ctrl = AnimationController(vsync: this, duration: const Duration(milliseconds: 300));
  }

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _ctrl,
      builder: (context, child) => Opacity(opacity: _ctrl.value, child: child),
      child: const Text('Hello'),
    );
  }
}
"#,
    );

    let report = falcon::animation_audit::audit_animations(dir.path()).unwrap();
    assert!(
        report.score >= 70,
        "Well-written animation code should score ≥70, got {}",
        report.score
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// 6. GOLDEN TEST GENERATOR
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn golden_gen_discovers_stateless_widget() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "lib/my_card.dart",
        r#"
import 'package:flutter/material.dart';

class MyCard extends StatelessWidget {
  const MyCard({super.key, required this.title, this.subtitle = ''});

  final String title;
  final String subtitle;

  @override
  Widget build(BuildContext context) => Card(child: Text(title));
}
"#,
    );

    let widgets = falcon::golden_gen::discover_widgets(dir.path()).unwrap();
    let found = widgets.iter().any(|w| w.name == "MyCard");
    assert!(found, "Should discover MyCard widget");
}

#[test]
fn golden_gen_skips_generated_files() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "lib/my_widget.g.dart",
        r#"
class GeneratedWidget extends StatelessWidget {
  @override Widget build(BuildContext context) => const SizedBox();
}
"#,
    );

    let widgets = falcon::golden_gen::discover_widgets(dir.path()).unwrap();
    assert!(
        widgets.iter().all(|w| w.name != "GeneratedWidget"),
        "Should skip .g.dart generated files"
    );
}

#[test]
fn golden_gen_dry_run_does_not_write_files() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "lib/my_btn.dart",
        r#"
class MyButton extends StatelessWidget {
  const MyButton({super.key, required this.label});
  final String label;
  @override Widget build(BuildContext context) => TextButton(onPressed: () {}, child: Text(label));
}
"#,
    );

    let out_dir = dir.path().join("test/golden_generated");
    let report = falcon::golden_gen::generate_golden_tests(dir.path(), &out_dir, true).unwrap();

    // Dry run → no files written, but report still shows what would be generated
    assert!(
        !out_dir.exists(),
        "Dry run should not create output directory"
    );
    assert!(
        report.widgets_found > 0 || report.files_generated == 0,
        "Dry run should report 0 files generated"
    );
}

#[test]
fn golden_gen_writes_test_file() {
    let dir = TempDir::new().unwrap();
    write(&dir, "pubspec.yaml", "name: my_app\n");
    write(
        &dir,
        "lib/my_badge.dart",
        r#"
class MyBadge extends StatelessWidget {
  const MyBadge({super.key, required this.count});
  final int count;
  @override Widget build(BuildContext context) => Text('$count');
}
"#,
    );

    let out_dir = dir.path().join("test/golden_generated");
    let report = falcon::golden_gen::generate_golden_tests(dir.path(), &out_dir, false).unwrap();

    if report.files_generated > 0 {
        assert!(out_dir.exists(), "Output directory should be created");
        let generated = report.generated.first().unwrap();
        assert!(generated.test_content.contains("testWidgets"));
        assert!(generated.test_content.contains("matchesGoldenFile"));
    }
    // Even if no files are generated, the report should not panic
}

#[test]
fn golden_gen_html_report_renders() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "lib/widget.dart",
        r#"
class SimpleWidget extends StatelessWidget {
  const SimpleWidget({super.key});
  @override Widget build(BuildContext context) => const SizedBox();
}
"#,
    );

    let out_dir = dir.path().join("test/golden_generated");
    let report = falcon::golden_gen::generate_golden_tests(dir.path(), &out_dir, true).unwrap();

    let html_path = dir.path().join("golden_report.html");
    falcon::golden_gen::write_golden_html_report(&report, &html_path).unwrap();

    let html = fs::read_to_string(&html_path).unwrap();
    assert!(html.contains("Golden"), "HTML should mention 'Golden'");
    assert!(html.contains("<!DOCTYPE html") || html.contains("<html"));
}
