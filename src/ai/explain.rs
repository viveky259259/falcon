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
        "falcon explain".bright_cyan().bold(),
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

    db
}
