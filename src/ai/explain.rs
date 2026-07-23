use colored::Colorize;
use std::collections::HashMap;

#[derive(Clone)]
pub struct RuleExplanation {
    pub name: String,
    pub category: String,
    pub severity: String,
    pub summary: String,
    pub why: String,
    pub bad_example: String,
    pub good_example: String,
    pub exceptions: Vec<String>,
    pub references: Vec<String>,
}

pub fn explain_rule(rule_name: &str) -> Option<RuleExplanation> {
    let db = build_explanation_db();
    db.get(rule_name).cloned()
}

pub fn print_explanation(explanation: &RuleExplanation) {
    println!();
    println!(
        "  {} {}",
        "Rule:".bright_cyan().bold(),
        explanation.name.bright_white().bold()
    );
    println!("  {} {}", "Category:".bright_cyan(), explanation.category);
    println!("  {} {}", "Severity:".bright_cyan(), explanation.severity);
    println!();
    println!("  {}", "Summary".bright_green().bold());
    println!("  {}", explanation.summary);
    println!();
    println!("  {}", "Why this matters".bright_yellow().bold());
    println!("  {}", explanation.why);
    println!();
    println!(
        "  {} {}",
        "Bad".bright_red().bold(),
        "(avoid this)".dimmed()
    );
    for line in explanation.bad_example.lines() {
        println!("    {}", line);
    }
    println!();
    println!(
        "  {} {}",
        "Good".bright_green().bold(),
        "(prefer this)".dimmed()
    );
    for line in explanation.good_example.lines() {
        println!("    {}", line);
    }

    if !explanation.exceptions.is_empty() {
        println!();
        println!("  {}", "Exceptions".bright_blue().bold());
        for exc in &explanation.exceptions {
            println!("    • {}", exc);
        }
    }

    if !explanation.references.is_empty() {
        println!();
        println!("  {}", "References".dimmed());
        for r in &explanation.references {
            println!("    {}", r.dimmed());
        }
    }
    println!();
}

pub fn list_all_rules() {
    let db = build_explanation_db();
    let mut rules: Vec<_> = db.values().collect();
    rules.sort_by_key(|r| &r.name);

    let mut categories: HashMap<&str, Vec<&RuleExplanation>> = HashMap::new();
    for rule in &rules {
        categories.entry(&rule.category).or_default().push(rule);
    }

    println!();
    println!(
        "  {} {} rules with explanations",
        "falcon x explain".bright_cyan().bold(),
        rules.len()
    );
    println!();

    let mut cats: Vec<_> = categories.into_iter().collect();
    cats.sort_by_key(|(c, _)| *c);

    for (cat, cat_rules) in cats {
        println!("  {} ({})", cat.bright_green().bold(), cat_rules.len());
        for rule in cat_rules {
            println!(
                "    {} — {}",
                rule.name.bright_white(),
                truncate(&rule.summary, 60)
            );
        }
        println!();
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

fn build_explanation_db() -> HashMap<String, RuleExplanation> {
    let mut db = HashMap::new();

    db.insert("avoid-long-functions".to_string(), RuleExplanation {
        name: "avoid-long-functions".to_string(),
        category: "Dart".to_string(),
        severity: "warning".to_string(),
        summary: "Functions exceeding the configured line threshold are flagged.".to_string(),
        why: "Long functions are hard to understand, test, and maintain. They often do too many things and should be broken into smaller, focused functions. Studies show code comprehension drops sharply after ~50 lines.".to_string(),
        bad_example: "void processOrder(Order order) {\n  // 200 lines of validation, calculation,\n  // API calls, logging, and error handling\n}".to_string(),
        good_example: "void processOrder(Order order) {\n  validateOrder(order);\n  final total = calculateTotal(order);\n  await submitPayment(order, total);\n  notifyCustomer(order);\n}".to_string(),
        exceptions: vec!["Build methods in complex widgets may naturally be longer.".to_string(), "Generated code is excluded.".to_string()],
        references: vec!["Clean Code by Robert C. Martin, Chapter 3".to_string()],
    });

    db.insert("no-magic-numbers".to_string(), RuleExplanation {
        name: "no-magic-numbers".to_string(),
        category: "Dart".to_string(),
        severity: "info".to_string(),
        summary: "Numeric literals in code should be extracted into named constants.".to_string(),
        why: "Magic numbers make code harder to understand and change. When you see `if (retries > 3)`, the intent is unclear. `if (retries > maxRetries)` is self-documenting. Constants also prevent bugs from changing the same value in multiple places.".to_string(),
        bad_example: "final padding = EdgeInsets.all(16.0);\nif (statusCode == 401) { redirect(); }\nawait Future.delayed(Duration(seconds: 30));".to_string(),
        good_example: "const kDefaultPadding = 16.0;\nconst kUnauthorized = 401;\nconst kTimeoutSeconds = 30;\n\nfinal padding = EdgeInsets.all(kDefaultPadding);\nif (statusCode == kUnauthorized) { redirect(); }".to_string(),
        exceptions: vec![
            "0, 1, -1, 2, 0.0, 1.0 are allowed by default.".to_string(),
            "HTTP status codes (100-599) are auto-skipped with context-aware mode.".to_string(),
            "Numbers in const/final declarations are excluded.".to_string(),
        ],
        references: vec!["Effective Dart: Style".to_string()],
    });

    db.insert("avoid-late-keyword".to_string(), RuleExplanation {
        name: "avoid-late-keyword".to_string(),
        category: "Dart".to_string(),
        severity: "info".to_string(),
        summary: "The `late` keyword defers initialization checks to runtime.".to_string(),
        why: "`late` variables throw LateInitializationError at runtime if accessed before assignment. This converts a compile-time safety check into a runtime crash. Nullable types with proper null handling are safer.".to_string(),
        bad_example: "class MyWidget extends StatefulWidget {\n  late final TextEditingController _controller;\n  \n  @override\n  void initState() {\n    _controller = TextEditingController();\n  }\n}".to_string(),
        good_example: "class MyWidget extends StatefulWidget {\n  TextEditingController? _controller;\n  \n  @override\n  void initState() {\n    _controller = TextEditingController();\n  }\n}".to_string(),
        exceptions: vec![
            "Test files are auto-skipped (setUp/tearDown pattern is idiomatic).".to_string(),
            "`late final` with initializer (`late final x = compute()`) is lazy, not unsafe.".to_string(),
            "Framework-required patterns (e.g. AnimationController) are excluded.".to_string(),
        ],
        references: vec!["Dart Language Tour: Late variables".to_string()],
    });

    db.insert("avoid-dynamic".to_string(), RuleExplanation {
        name: "avoid-dynamic".to_string(),
        category: "Dart".to_string(),
        severity: "warning".to_string(),
        summary: "Avoid using `dynamic` type — it disables type checking.".to_string(),
        why: "The `dynamic` type bypasses Dart's entire type system. Method calls on dynamic are not checked at compile time, leading to runtime NoSuchMethodError crashes. Use `Object` when you need a generic type, or generics for flexibility.".to_string(),
        bad_example: "dynamic parseResult(String input) {\n  return json.decode(input);\n}".to_string(),
        good_example: "Map<String, Object?> parseResult(String input) {\n  return json.decode(input) as Map<String, Object?>;\n}".to_string(),
        exceptions: vec!["Interop with untyped APIs (rare).".to_string()],
        references: vec!["Effective Dart: Usage — Avoid using dynamic".to_string()],
    });

    db.insert("avoid-global-state".to_string(), RuleExplanation {
        name: "avoid-global-state".to_string(),
        category: "Dart".to_string(),
        severity: "warning".to_string(),
        summary: "Mutable top-level variables create hidden global state.".to_string(),
        why: "Global mutable state makes code unpredictable, untestable, and prone to race conditions. Any code anywhere can change the value, making bugs hard to track. Use dependency injection or state management instead.".to_string(),
        bad_example: "var currentUser = User();\nvar appTheme = ThemeData();\nList<String> cachedItems = [];".to_string(),
        good_example: "// Use dependency injection\nclass AppState {\n  final User currentUser;\n  final ThemeData theme;\n  AppState({required this.currentUser, required this.theme});\n}".to_string(),
        exceptions: vec!["Top-level `const` and `final` are safe.".to_string()],
        references: vec![],
    });

    db.insert("avoid-returning-widgets".to_string(), RuleExplanation {
        name: "avoid-returning-widgets".to_string(),
        category: "Flutter".to_string(),
        severity: "warning".to_string(),
        summary: "Methods that return Widget should be extracted to separate widget classes.".to_string(),
        why: "Widget-returning methods prevent Flutter from optimizing rebuilds. A method inside a widget means the returned subtree rebuilds every time the parent rebuilds. Extract to a separate widget class for proper `shouldRebuild` lifecycle.".to_string(),
        bad_example: "class MyPage extends StatelessWidget {\n  Widget _buildHeader() {\n    return Container(child: Text('Header'));\n  }\n  Widget build(BuildContext context) {\n    return Column(children: [_buildHeader()]);\n  }\n}".to_string(),
        good_example: "class MyPage extends StatelessWidget {\n  Widget build(BuildContext context) {\n    return Column(children: [HeaderWidget()]);\n  }\n}\n\nclass HeaderWidget extends StatelessWidget {\n  Widget build(BuildContext context) {\n    return Container(child: Text('Header'));\n  }\n}".to_string(),
        exceptions: vec!["Simple helper methods in builders.".to_string()],
        references: vec!["Flutter performance best practices".to_string()],
    });

    db.insert("prefer-const-constructors".to_string(), RuleExplanation {
        name: "prefer-const-constructors".to_string(),
        category: "Flutter".to_string(),
        severity: "info".to_string(),
        summary: "Use `const` constructors where possible for Flutter widgets.".to_string(),
        why: "Const widgets are canonicalized by the compiler — the same const expression always produces the identical object. This means Flutter can skip rebuilding const subtrees entirely, significantly improving performance in large widget trees.".to_string(),
        bad_example: "return Padding(\n  padding: EdgeInsets.all(8.0),\n  child: Text('Hello'),\n);".to_string(),
        good_example: "return const Padding(\n  padding: EdgeInsets.all(8.0),\n  child: Text('Hello'),\n);".to_string(),
        exceptions: vec!["Widgets with runtime values cannot be const.".to_string()],
        references: vec!["Flutter docs: Performance best practices".to_string()],
    });

    db.insert("avoid-unnecessary-setstate".to_string(), RuleExplanation {
        name: "avoid-unnecessary-setstate".to_string(),
        category: "Flutter".to_string(),
        severity: "warning".to_string(),
        summary: "Don't call setState in lifecycle methods where it's unnecessary.".to_string(),
        why: "Calling setState in initState or didChangeDependencies triggers an extra rebuild. The framework already schedules a build after these methods. Direct state mutation without setState is correct here.".to_string(),
        bad_example: "@override\nvoid initState() {\n  super.initState();\n  setState(() { _counter = 0; });\n}".to_string(),
        good_example: "@override\nvoid initState() {\n  super.initState();\n  _counter = 0;\n}".to_string(),
        exceptions: vec![],
        references: vec!["Flutter API: State.initState".to_string()],
    });

    db.insert("dead-code-path".to_string(), RuleExplanation {
        name: "dead-code-path".to_string(),
        category: "Detection".to_string(),
        severity: "warning".to_string(),
        summary: "Code that can never execute: after return/throw, or inside always-false branches.".to_string(),
        why: "Dead code is noise — it confuses maintainers, inflates metrics, and sometimes masks bugs. Unreachable code after a return statement will never execute. Trivial conditions like `if (false)` indicate debug leftovers or logic errors.".to_string(),
        bad_example: "int compute(int x) {\n  return x * 2;\n  print('done'); // unreachable\n}\n\nif (false) {\n  dangerousOperation(); // dead branch\n}".to_string(),
        good_example: "int compute(int x) {\n  return x * 2;\n}\n\n// Remove dead branches or use feature flags".to_string(),
        exceptions: vec![
            "Debug/development code may use `if (kDebugMode)`.".to_string(),
        ],
        references: vec![],
    });

    db.insert("avoid-ref-read-inside-build".to_string(), RuleExplanation {
        name: "avoid-ref-read-inside-build".to_string(),
        category: "Provider/Riverpod".to_string(),
        severity: "warning".to_string(),
        summary: "Use `ref.watch` instead of `ref.read` inside build methods.".to_string(),
        why: "`ref.read` only gets the value once and doesn't subscribe to changes. Inside build, this means the widget won't rebuild when the provider's state changes, leading to stale UI. Use `ref.watch` to automatically rebuild.".to_string(),
        bad_example: "Widget build(BuildContext context, WidgetRef ref) {\n  final count = ref.read(counterProvider);\n  return Text('$count');\n}".to_string(),
        good_example: "Widget build(BuildContext context, WidgetRef ref) {\n  final count = ref.watch(counterProvider);\n  return Text('$count');\n}".to_string(),
        exceptions: vec![],
        references: vec!["Riverpod docs: Reading a provider".to_string()],
    });

    db.insert("avoid-bloc-public-methods".to_string(), RuleExplanation {
        name: "avoid-bloc-public-methods".to_string(),
        category: "BLoC".to_string(),
        severity: "warning".to_string(),
        summary: "BLoC classes should only expose `add` for events, not public methods.".to_string(),
        why: "The BLoC pattern enforces unidirectional data flow: Events in → States out. Public methods on a BLoC bypass this pattern, making state changes unpredictable and harder to debug/test. Use events for all interactions.".to_string(),
        bad_example: "class CounterBloc extends Bloc<Event, int> {\n  void increment() => emit(state + 1); // BAD\n}".to_string(),
        good_example: "class CounterBloc extends Bloc<Event, int> {\n  CounterBloc() : super(0) {\n    on<Increment>((event, emit) => emit(state + 1));\n  }\n}".to_string(),
        exceptions: vec![],
        references: vec!["BLoC library: Best practices".to_string()],
    });

    db.insert("fake-mounted-check".to_string(), RuleExplanation {
        name: "fake-mounted-check".to_string(),
        category: "Behavioral".to_string(),
        severity: "error".to_string(),
        summary: "`if (mounted)` before an `await` is stale unless `mounted` is checked again after the async gap.".to_string(),
        why: "A State can be disposed while an awaited Future is pending. Code generated by assistants often adds a mounted guard before the await but still calls setState or Navigator afterward without re-checking, which can crash at runtime.".to_string(),
        bad_example: "if (mounted) {\n  await repository.load();\n  setState(() {});\n}".to_string(),
        good_example: "await repository.load();\nif (!mounted) return;\nsetState(() {});".to_string(),
        exceptions: vec!["Synchronous mounted checks with no await in the guarded block are allowed.".to_string()],
        references: vec!["Flutter State.mounted API".to_string()],
    });

    db.insert("silent-catch".to_string(), RuleExplanation {
        name: "silent-catch".to_string(),
        category: "Behavioral".to_string(),
        severity: "error".to_string(),
        summary: "Empty or comment-only catch blocks silently swallow exceptions.".to_string(),
        why: "A catch block with no executable handling turns production failures into missing state, stale UI, or hidden data loss. This is a common AI-generated pattern when code is patched just to make tests pass.".to_string(),
        bad_example: "try {\n  await save();\n} catch (_) {\n  // ignore\n}".to_string(),
        good_example: "try {\n  await save();\n} catch (error, stackTrace) {\n  logger.error('save failed', error, stackTrace);\n  rethrow;\n}".to_string(),
        exceptions: vec!["Intentional ignores should use a clearly named helper or logging policy rather than an empty catch body.".to_string()],
        references: vec![],
    });

    db.insert("unawaited-future-in-build".to_string(), RuleExplanation {
        name: "unawaited-future-in-build".to_string(),
        category: "Behavioral".to_string(),
        severity: "error".to_string(),
        summary: "Do not start fire-and-forget Futures from a Flutter build method.".to_string(),
        why: "Build can run many times. Starting Futures there can trigger repeated network calls, repeated mutations, and unhandled async failures each time Flutter rebuilds the widget tree.".to_string(),
        bad_example: "Widget build(BuildContext context) {\n  Future.delayed(Duration.zero).then((_) => load());\n  return const Text('loading');\n}".to_string(),
        good_example: "@override\nvoid initState() {\n  super.initState();\n  _loadFuture = load();\n}\n\nWidget build(BuildContext context) => FutureBuilder(future: _loadFuture, builder: buildBody);".to_string(),
        exceptions: vec!["Use `unawaited(...)` only for deliberate, safe fire-and-forget work outside build.".to_string()],
        references: vec!["Flutter performance best practices".to_string()],
    });

    db.insert("set-state-after-dispose".to_string(), RuleExplanation {
        name: "set-state-after-dispose".to_string(),
        category: "Behavioral".to_string(),
        severity: "error".to_string(),
        summary: "setState must not be reachable after a State has been disposed.".to_string(),
        why: "Calling setState on an unmounted State throws and usually indicates async work outliving the widget. Falcon uses resolver-backed State subclass detection and flags setState calls after an async gap when there is no mounted re-check.".to_string(),
        bad_example: "Future<void> load() async {\n  await api.fetch();\n  setState(() {});\n}".to_string(),
        good_example: "Future<void> load() async {\n  await api.fetch();\n  if (!mounted) return;\n  setState(() {});\n}".to_string(),
        exceptions: vec!["Complex aliasing and full Dart control-flow are intentionally conservative; explicit mounted guards keep the rule quiet.".to_string()],
        references: vec!["Flutter State.setState API".to_string()],
    });

    db.insert("riverpod-scope-leak".to_string(), RuleExplanation {
        name: "riverpod-scope-leak".to_string(),
        category: "Behavioral".to_string(),
        severity: "error".to_string(),
        summary: "Riverpod providers must not leak outside their intended ProviderScope or lifecycle.".to_string(),
        why: "A provider or subscription that escapes its scope can retain stale state, keep listeners alive, or rebuild widgets from the wrong container. Falcon conservatively flags provider factories that create subscriptions/controllers/timers without autoDispose or ref.onDispose cleanup.".to_string(),
        bad_example: "final subscriptionProvider = Provider((ref) {\n  return stream.listen((event) {});\n});".to_string(),
        good_example: "final subscriptionProvider = AutoDisposeProvider((ref) {\n  final sub = stream.listen((event) {});\n  ref.onDispose(sub.cancel);\n  return sub;\n});".to_string(),
        exceptions: vec!["Full generated-provider and ProviderScope symbol analysis is still future resolver work; autoDispose and ref.onDispose are accepted cleanup signals.".to_string()],
        references: vec!["Riverpod provider lifecycles".to_string()],
    });

    db.insert("dispose-not-called".to_string(), RuleExplanation {
        name: "dispose-not-called".to_string(),
        category: "Behavioral".to_string(),
        severity: "error".to_string(),
        summary: "Disposable State resources must be released and `super.dispose()` must be called.".to_string(),
        why: "Controllers, subscriptions, and focus nodes keep resources alive after a widget is removed unless dispose closes them. Resolver support is needed for the broad version; `ensure-dispose-lifecycle` covers the narrow Flutter controller case today.".to_string(),
        bad_example: "class _ScreenState extends State<Screen> {\n  final controller = TextEditingController();\n}".to_string(),
        good_example: "class _ScreenState extends State<Screen> {\n  final controller = TextEditingController();\n\n  @override\n  void dispose() {\n    controller.dispose();\n    super.dispose();\n  }\n}".to_string(),
        exceptions: vec!["Currently emits only after class-hierarchy resolution lands.".to_string()],
        references: vec!["Flutter State.dispose API".to_string()],
    });

    db
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── truncate ──────────────────────────────────────────────────────────────

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        let s = "1234567890";
        assert_eq!(truncate(s, 10), "1234567890");
    }

    #[test]
    fn truncate_long_string_adds_ellipsis() {
        let result = truncate("abcdefghij", 7);
        assert_eq!(result, "abcd...");
        assert_eq!(result.len(), 7);
    }

    #[test]
    fn truncate_empty_string() {
        assert_eq!(truncate("", 5), "");
    }

    #[test]
    fn truncate_max_zero_returns_ellipsis() {
        // max = 3 → &s[..0] = "" → "..."
        let result = truncate("hello", 3);
        assert_eq!(result, "...");
    }

    #[test]
    fn truncate_long_sentence() {
        let s = "Functions exceeding the configured line threshold are flagged.";
        let result = truncate(s, 20);
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), 20);
    }

    // ── explain_rule ─────────────────────────────────────────────────────────

    #[test]
    fn explain_rule_unknown_returns_none() {
        assert!(explain_rule("this-rule-does-not-exist").is_none());
    }

    #[test]
    fn explain_rule_empty_string_returns_none() {
        assert!(explain_rule("").is_none());
    }

    #[test]
    fn explain_rule_avoid_long_functions_exists() {
        let exp = explain_rule("avoid-long-functions").expect("rule should exist");
        assert_eq!(exp.name, "avoid-long-functions");
        assert_eq!(exp.category, "Dart");
        assert_eq!(exp.severity, "warning");
        assert!(!exp.summary.is_empty());
        assert!(!exp.why.is_empty());
        assert!(!exp.bad_example.is_empty());
        assert!(!exp.good_example.is_empty());
    }

    #[test]
    fn explain_rule_no_magic_numbers_exists() {
        let exp = explain_rule("no-magic-numbers").expect("rule should exist");
        assert_eq!(exp.name, "no-magic-numbers");
        assert_eq!(exp.severity, "info");
        assert!(!exp.exceptions.is_empty());
    }

    #[test]
    fn explain_rule_avoid_late_keyword_exists() {
        let exp = explain_rule("avoid-late-keyword").expect("rule should exist");
        assert_eq!(exp.name, "avoid-late-keyword");
        assert_eq!(exp.category, "Dart");
        assert!(!exp.exceptions.is_empty());
    }

    #[test]
    fn explain_rule_avoid_dynamic_exists() {
        let exp = explain_rule("avoid-dynamic").expect("rule should exist");
        assert_eq!(exp.name, "avoid-dynamic");
        assert_eq!(exp.severity, "warning");
    }

    #[test]
    fn explain_rule_avoid_global_state_exists() {
        let exp = explain_rule("avoid-global-state").expect("rule should exist");
        assert_eq!(exp.name, "avoid-global-state");
        assert_eq!(exp.category, "Dart");
        assert!(exp.references.is_empty(), "global-state has no references");
    }

    #[test]
    fn explain_rule_flutter_avoid_returning_widgets() {
        let exp = explain_rule("avoid-returning-widgets").expect("rule should exist");
        assert_eq!(exp.category, "Flutter");
        assert_eq!(exp.severity, "warning");
    }

    #[test]
    fn explain_rule_prefer_const_constructors() {
        let exp = explain_rule("prefer-const-constructors").expect("rule should exist");
        assert_eq!(exp.category, "Flutter");
        assert_eq!(exp.severity, "info");
    }

    #[test]
    fn explain_rule_avoid_unnecessary_setstate() {
        let exp = explain_rule("avoid-unnecessary-setstate").expect("rule should exist");
        assert_eq!(exp.category, "Flutter");
        assert!(exp.exceptions.is_empty());
    }

    #[test]
    fn explain_rule_dead_code_path() {
        let exp = explain_rule("dead-code-path").expect("rule should exist");
        assert_eq!(exp.category, "Detection");
        assert_eq!(exp.severity, "warning");
    }

    #[test]
    fn explain_rule_riverpod_ref_read() {
        let exp = explain_rule("avoid-ref-read-inside-build").expect("rule should exist");
        assert_eq!(exp.category, "Provider/Riverpod");
        assert!(!exp.references.is_empty());
    }

    #[test]
    fn explain_rule_bloc_public_methods() {
        let exp = explain_rule("avoid-bloc-public-methods").expect("rule should exist");
        assert_eq!(exp.category, "BLoC");
        assert_eq!(exp.severity, "warning");
    }

    // ── build_explanation_db ─────────────────────────────────────────────────

    #[test]
    fn db_contains_at_least_ten_rules() {
        let db = build_explanation_db();
        assert!(db.len() >= 10, "expected ≥10 rules, got {}", db.len());
    }

    #[test]
    fn db_all_rules_have_non_empty_name() {
        let db = build_explanation_db();
        for (key, rule) in &db {
            assert!(!rule.name.is_empty(), "rule {} has empty name", key);
        }
    }

    #[test]
    fn db_all_rules_have_non_empty_summary() {
        let db = build_explanation_db();
        for (key, rule) in &db {
            assert!(!rule.summary.is_empty(), "rule {} has empty summary", key);
        }
    }

    #[test]
    fn db_all_rules_have_non_empty_category() {
        let db = build_explanation_db();
        for (key, rule) in &db {
            assert!(!rule.category.is_empty(), "rule {} has empty category", key);
        }
    }

    #[test]
    fn db_all_rules_have_non_empty_severity() {
        let db = build_explanation_db();
        for (key, rule) in &db {
            assert!(!rule.severity.is_empty(), "rule {} has empty severity", key);
        }
    }

    #[test]
    fn db_key_matches_rule_name() {
        let db = build_explanation_db();
        for (key, rule) in &db {
            assert_eq!(
                key, &rule.name,
                "DB key '{}' does not match rule name '{}'",
                key, rule.name
            );
        }
    }

    #[test]
    fn db_severities_are_valid_values() {
        let valid = ["info", "warning", "error"];
        let db = build_explanation_db();
        for (key, rule) in &db {
            assert!(
                valid.contains(&rule.severity.as_str()),
                "rule {} has unexpected severity '{}'",
                key,
                rule.severity
            );
        }
    }
}
