//! Detect whether `SemanticsBinding.instance.ensureSemantics()` is called in
//! the project's `main()` function.

/// Result of the main-file scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MainCheckResult {
    /// `ensureSemantics()` is called (active, uncommented).
    Present,
    /// The call is in source but is part of a commented-out line.
    CommentedOut,
    /// No call is found anywhere in the file.
    Missing,
    /// The file could not be read or had no `main()` function.
    NoMainFunction,
}

/// Scan a `main.dart` source string and report whether `ensureSemantics()` is
/// present, commented out, or missing.
pub fn check_main_dart(source: &str) -> MainCheckResult {
    if !source.contains("main(") && !source.contains("main (") {
        return MainCheckResult::NoMainFunction;
    }

    let mut found_commented = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            if line.contains("ensureSemantics") {
                found_commented = true;
            }
            continue;
        }
        let code_only = match trimmed.find("//") {
            Some(idx) => &trimmed[..idx],
            None => trimmed,
        };
        if code_only.contains("ensureSemantics") {
            return MainCheckResult::Present;
        }
        if trimmed.contains("ensureSemantics") {
            found_commented = true;
        }
    }

    if found_commented {
        MainCheckResult::CommentedOut
    } else {
        MainCheckResult::Missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_active_ensure_semantics_call() {
        let src = r#"
import 'package:flutter/widgets.dart';
void main() {
  SemanticsBinding.instance.ensureSemantics();
  runApp(MyApp());
}
"#;
        assert_eq!(check_main_dart(src), MainCheckResult::Present);
    }

    #[test]
    fn detects_commented_out_call_line_comment() {
        let src = r#"
void main() {
  // SemanticsBinding.instance.ensureSemantics();
  runApp(MyApp());
}
"#;
        assert_eq!(check_main_dart(src), MainCheckResult::CommentedOut);
    }

    #[test]
    fn detects_commented_out_call_trailing_comment() {
        let src = r#"
void main() {
  runApp(MyApp()); // SemanticsBinding.instance.ensureSemantics();
}
"#;
        assert_eq!(check_main_dart(src), MainCheckResult::CommentedOut);
    }

    #[test]
    fn returns_missing_when_no_call_anywhere() {
        let src = r#"
void main() {
  runApp(MyApp());
}
"#;
        assert_eq!(check_main_dart(src), MainCheckResult::Missing);
    }

    #[test]
    fn returns_no_main_function_when_main_absent() {
        let src = "class Foo {}\n";
        assert_eq!(check_main_dart(src), MainCheckResult::NoMainFunction);
    }

    #[test]
    fn active_call_takes_precedence_over_commented() {
        let src = r#"
void main() {
  // ensureSemantics() — legacy comment
  SemanticsBinding.instance.ensureSemantics();
  runApp(MyApp());
}
"#;
        assert_eq!(check_main_dart(src), MainCheckResult::Present);
    }

    #[test]
    fn handles_arrow_body_main() {
        let src = "void main() => runApp(MyApp());\n";
        assert_eq!(check_main_dart(src), MainCheckResult::Missing);
    }

    #[test]
    fn handles_async_main() {
        let src = r#"
Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  SemanticsBinding.instance.ensureSemantics();
  runApp(MyApp());
}
"#;
        assert_eq!(check_main_dart(src), MainCheckResult::Present);
    }
}
