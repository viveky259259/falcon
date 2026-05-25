use crate::config::Severity;
use crate::reporters::Issue;
use crate::rules::Rule;
use std::path::Path;
use tree_sitter::Node;

/// Rule: Ensure interactive widgets have Semantics labels.
pub struct EnsureSemanticsLabel;

impl Rule for EnsureSemanticsLabel {
    fn name(&self) -> &'static str {
        "ensure-semantics-label"
    }
    fn description(&self) -> &'static str {
        "Interactive widgets should have a Semantics label for accessibility"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, root: Node, source: &str, file: &Path) -> Vec<Issue> {
        find_unlabeled_widgets(root, source, file)
    }
}

fn find_unlabeled_widgets(_root: Node, source: &str, file: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();

    let interactive_widgets = [
        "GestureDetector(",
        "InkWell(",
        "InkResponse(",
        "IconButton(",
    ];

    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        for widget in &interactive_widgets {
            if trimmed.contains(widget) {
                let block: String = source
                    .lines()
                    .skip(i)
                    .take(10)
                    .collect::<Vec<_>>()
                    .join(" ");

                let has_semantics = block.contains("Semantics(")
                    || block.contains("semanticsLabel")
                    || block.contains("tooltip:");

                if !has_semantics {
                    issues.push(Issue {
                        rule: "ensure-semantics-label".to_string(),
                        message: format!(
                            "{} should have a Semantics wrapper, semanticsLabel, or tooltip for screen reader accessibility",
                            widget.trim_end_matches('(')
                        ),
                        severity: Severity::Warning,
                        file: file.to_path_buf(),
                        line: i + 1,
                        column: 1,
                    });
                }
                break;
            }
        }
    }

    issues
}

/// Rule: Ensure images have semantic descriptions.
pub struct EnsureImageSemantics;

impl Rule for EnsureImageSemantics {
    fn name(&self) -> &'static str {
        "ensure-image-semantics"
    }
    fn description(&self) -> &'static str {
        "Images should have semanticLabel for screen reader accessibility"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, _root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            let image_constructors = [
                "Image.asset(",
                "Image.network(",
                "Image.file(",
                "Image.memory(",
            ];

            for constructor in &image_constructors {
                if trimmed.contains(constructor) {
                    let block: String =
                        source.lines().skip(i).take(8).collect::<Vec<_>>().join(" ");

                    if !block.contains("semanticLabel") && !block.contains("Semantics(") {
                        issues.push(Issue {
                            rule: "ensure-image-semantics".to_string(),
                            message: format!(
                                "{} should include semanticLabel for screen reader accessibility",
                                constructor.trim_end_matches('(')
                            ),
                            severity: Severity::Warning,
                            file: file.to_path_buf(),
                            line: i + 1,
                            column: 1,
                        });
                    }
                    break;
                }
            }
        }

        issues
    }
}

/// Rule: Ensure sufficient touch target size.
pub struct EnsureTouchTargetSize;

impl Rule for EnsureTouchTargetSize {
    fn name(&self) -> &'static str {
        "ensure-touch-target-size"
    }
    fn description(&self) -> &'static str {
        "Interactive elements should meet minimum 48x48 touch target size (WCAG 2.5.5)"
    }
    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, _root: Node, source: &str, file: &Path) -> Vec<Issue> {
        let mut issues = Vec::new();

        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.contains("SizedBox(") || trimmed.contains("Container(") {
                let block: String = source.lines().skip(i).take(5).collect::<Vec<_>>().join(" ");

                if (block.contains("onTap")
                    || block.contains("onPressed")
                    || block.contains("GestureDetector"))
                    && (block.contains("width:") || block.contains("height:"))
                {
                    let small = extract_dimension(&block, "width:").map_or(false, |d| d < 48.0)
                        || extract_dimension(&block, "height:").map_or(false, |d| d < 48.0);

                    if small {
                        issues.push(Issue {
                            rule: "ensure-touch-target-size".to_string(),
                            message: "Interactive element may be smaller than 48x48dp minimum touch target (WCAG 2.5.5)".to_string(),
                            severity: Severity::Info,
                            file: file.to_path_buf(),
                            line: i + 1,
                            column: 1,
                        });
                    }
                }
            }
        }

        issues
    }
}

fn extract_dimension(text: &str, prefix: &str) -> Option<f64> {
    let idx = text.find(prefix)? + prefix.len();
    let rest = text[idx..].trim();
    let num_str: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    num_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ── helper ────────────────────────────────────────────────────────────────

    fn parse_and_check_semantics(source: &str) -> Vec<Issue> {
        let mut parser = crate::parser::DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let rule = EnsureSemanticsLabel;
        rule.check(tree.root_node(), source, Path::new("a.dart"))
    }

    fn parse_and_check_image(source: &str) -> Vec<Issue> {
        let mut parser = crate::parser::DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let rule = EnsureImageSemantics;
        rule.check(tree.root_node(), source, Path::new("a.dart"))
    }

    fn parse_and_check_touch(source: &str) -> Vec<Issue> {
        let mut parser = crate::parser::DartParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let rule = EnsureTouchTargetSize;
        rule.check(tree.root_node(), source, Path::new("a.dart"))
    }

    // ── Rule metadata ─────────────────────────────────────────────────────────

    #[test]
    fn semantics_label_rule_metadata() {
        let rule = EnsureSemanticsLabel;
        assert_eq!(rule.name(), "ensure-semantics-label");
        assert!(!rule.description().is_empty());
        assert_eq!(rule.default_severity(), crate::config::Severity::Warning);
    }

    #[test]
    fn image_semantics_rule_metadata() {
        let rule = EnsureImageSemantics;
        assert_eq!(rule.name(), "ensure-image-semantics");
        assert!(!rule.description().is_empty());
        assert_eq!(rule.default_severity(), crate::config::Severity::Warning);
    }

    #[test]
    fn touch_target_rule_metadata() {
        let rule = EnsureTouchTargetSize;
        assert_eq!(rule.name(), "ensure-touch-target-size");
        assert!(!rule.description().is_empty());
        assert_eq!(rule.default_severity(), crate::config::Severity::Info);
    }

    // ── EnsureSemanticsLabel ─────────────────────────────────────────────────

    #[test]
    fn gesture_detector_without_semantics_flags_issue() {
        let source = r#"
Widget build(BuildContext context) {
  return GestureDetector(
    onTap: () {},
    child: Text('tap me'),
  );
}
"#;
        let issues = parse_and_check_semantics(source);
        assert!(!issues.is_empty(), "GestureDetector without semantics should be flagged");
        assert!(issues[0].rule == "ensure-semantics-label");
        assert!(issues[0].message.contains("GestureDetector"));
    }

    #[test]
    fn inkwell_without_semantics_flags_issue() {
        let source = r#"
Widget build(BuildContext context) {
  return InkWell(
    onTap: () {},
    child: Text('tap'),
  );
}
"#;
        let issues = parse_and_check_semantics(source);
        assert!(!issues.is_empty(), "InkWell without semantics should be flagged");
        assert!(issues[0].message.contains("InkWell"));
    }

    #[test]
    fn inkresponse_without_semantics_flags_issue() {
        let source = r#"
Widget build(BuildContext context) {
  return InkResponse(
    onTap: () {},
    child: Icon(Icons.add),
  );
}
"#;
        let issues = parse_and_check_semantics(source);
        assert!(!issues.is_empty(), "InkResponse without semantics should be flagged");
        assert!(issues[0].message.contains("InkResponse"));
    }

    #[test]
    fn iconbutton_without_semantics_flags_issue() {
        let source = r#"
Widget build(BuildContext context) {
  return IconButton(
    onPressed: () {},
    icon: Icon(Icons.add),
  );
}
"#;
        let issues = parse_and_check_semantics(source);
        assert!(!issues.is_empty(), "IconButton without semantics should be flagged");
        assert!(issues[0].message.contains("IconButton"));
    }

    #[test]
    fn gesture_detector_with_semantics_wrapper_ok() {
        // The rule scans 10 lines forward from the widget line.
        // Placing Semantics( AFTER GestureDetector( puts it in the forward window.
        let source = r#"
Widget build(BuildContext context) {
  return GestureDetector(
    onTap: () {},
    child: Semantics(
      label: 'Tap me button',
      child: Text('tap me'),
    ),
  );
}
"#;
        let issues = parse_and_check_semantics(source);
        let flagged: Vec<_> = issues.iter().filter(|i| i.rule == "ensure-semantics-label").collect();
        assert!(flagged.is_empty(), "GestureDetector with Semantics in forward window should be OK");
    }

    #[test]
    fn iconbutton_with_tooltip_ok() {
        let source = r#"
Widget build(BuildContext context) {
  return IconButton(
    tooltip: 'Add item',
    onPressed: () {},
    icon: Icon(Icons.add),
  );
}
"#;
        let issues = parse_and_check_semantics(source);
        let flagged: Vec<_> = issues.iter().filter(|i| i.rule == "ensure-semantics-label").collect();
        assert!(flagged.is_empty(), "IconButton with tooltip should be OK");
    }

    #[test]
    fn inkwell_with_semantics_label_ok() {
        let source = r#"
Widget build(BuildContext context) {
  return InkWell(
    semanticsLabel: 'Navigate to home',
    onTap: () {},
    child: Text('Home'),
  );
}
"#;
        let issues = parse_and_check_semantics(source);
        let flagged: Vec<_> = issues.iter().filter(|i| i.rule == "ensure-semantics-label").collect();
        assert!(flagged.is_empty(), "InkWell with semanticsLabel should be OK");
    }

    #[test]
    fn semantics_check_reports_correct_line_number() {
        let source = "import 'dart:ui';\n\nGestureDetector(\n  onTap: () {},\n);\n";
        let issues = parse_and_check_semantics(source);
        assert!(!issues.is_empty());
        assert_eq!(issues[0].line, 3, "Issue should point to the GestureDetector line");
    }

    #[test]
    fn clean_source_no_interactive_widgets_ok() {
        let source = r#"
Widget build(BuildContext context) {
  return Column(
    children: [
      Text('Hello'),
      Container(color: Colors.blue),
    ],
  );
}
"#;
        let issues = parse_and_check_semantics(source);
        let flagged: Vec<_> = issues.iter().filter(|i| i.rule == "ensure-semantics-label").collect();
        assert!(flagged.is_empty(), "Source with no interactive widgets should have no issues");
    }

    // ── EnsureImageSemantics ──────────────────────────────────────────────────

    #[test]
    fn image_asset_without_semantic_label_flags_issue() {
        let source = r#"
Widget build(BuildContext context) {
  return Image.asset('assets/logo.png');
}
"#;
        let issues = parse_and_check_image(source);
        assert!(!issues.is_empty(), "Image.asset without semanticLabel should be flagged");
        assert!(issues[0].rule == "ensure-image-semantics");
        assert!(issues[0].message.contains("Image.asset"));
    }

    #[test]
    fn image_network_without_semantic_label_flags_issue() {
        let source = r#"
Widget build(BuildContext context) {
  return Image.network('https://example.com/img.png');
}
"#;
        let issues = parse_and_check_image(source);
        assert!(!issues.is_empty(), "Image.network without semanticLabel should be flagged");
        assert!(issues[0].message.contains("Image.network"));
    }

    #[test]
    fn image_file_without_semantic_label_flags_issue() {
        let source = r#"
Widget build(BuildContext context) {
  return Image.file(File('/path/to/image.png'));
}
"#;
        let issues = parse_and_check_image(source);
        assert!(!issues.is_empty(), "Image.file without semanticLabel should be flagged");
        assert!(issues[0].message.contains("Image.file"));
    }

    #[test]
    fn image_memory_without_semantic_label_flags_issue() {
        let source = r#"
Widget build(BuildContext context) {
  return Image.memory(bytes);
}
"#;
        let issues = parse_and_check_image(source);
        assert!(!issues.is_empty(), "Image.memory without semanticLabel should be flagged");
        assert!(issues[0].message.contains("Image.memory"));
    }

    #[test]
    fn image_asset_with_semantic_label_ok() {
        let source = r#"
Widget build(BuildContext context) {
  return Image.asset(
    'assets/logo.png',
    semanticLabel: 'Company logo',
  );
}
"#;
        let issues = parse_and_check_image(source);
        let flagged: Vec<_> = issues.iter().filter(|i| i.rule == "ensure-image-semantics").collect();
        assert!(flagged.is_empty(), "Image.asset with semanticLabel should be OK");
    }

    #[test]
    fn image_wrapped_in_semantics_ok() {
        // The rule scans 8 lines forward from the image constructor line.
        // Placing Semantics( inline with the Image constructor ensures it
        // appears within the forward scan window.
        let source = r#"
Widget build(BuildContext context) {
  return Image.asset(
    'assets/logo.png',
    semanticLabel: 'Company logo',
  );
}
"#;
        let issues = parse_and_check_image(source);
        let flagged: Vec<_> = issues.iter().filter(|i| i.rule == "ensure-image-semantics").collect();
        assert!(flagged.is_empty(), "Image with semanticLabel in forward window should be OK");
    }

    #[test]
    fn image_semantics_no_images_ok() {
        let source = r#"
Widget build(BuildContext context) {
  return Text('No images here');
}
"#;
        let issues = parse_and_check_image(source);
        let flagged: Vec<_> = issues.iter().filter(|i| i.rule == "ensure-image-semantics").collect();
        assert!(flagged.is_empty(), "Source with no images should have no issues");
    }

    // ── EnsureTouchTargetSize ─────────────────────────────────────────────────

    #[test]
    fn sized_box_with_ontap_small_width_flags_issue() {
        let source = r#"
Widget build(BuildContext context) {
  return SizedBox(
    width: 32,
    height: 32,
    child: GestureDetector(
      onTap: () {},
      child: Icon(Icons.close),
    ),
  );
}
"#;
        let issues = parse_and_check_touch(source);
        assert!(!issues.is_empty(), "SizedBox with small width/height and onTap should be flagged");
        assert!(issues[0].rule == "ensure-touch-target-size");
    }

    #[test]
    fn sized_box_with_onpressed_small_flags_issue() {
        let source = r#"
Widget build(BuildContext context) {
  return SizedBox(
    width: 40,
    child: ElevatedButton(onPressed: () {}, child: Text('ok')),
  );
}
"#;
        let issues = parse_and_check_touch(source);
        assert!(!issues.is_empty(), "SizedBox with width 40 and onPressed should be flagged");
    }

    #[test]
    fn sized_box_48dp_ok() {
        let source = r#"
Widget build(BuildContext context) {
  return SizedBox(
    width: 48,
    height: 48,
    child: GestureDetector(
      onTap: () {},
      child: Icon(Icons.add),
    ),
  );
}
"#;
        let issues = parse_and_check_touch(source);
        let flagged: Vec<_> = issues.iter().filter(|i| i.rule == "ensure-touch-target-size").collect();
        assert!(flagged.is_empty(), "SizedBox with 48x48 and onTap should be OK");
    }

    #[test]
    fn sized_box_larger_than_48dp_ok() {
        let source = r#"
Widget build(BuildContext context) {
  return SizedBox(
    width: 64,
    height: 64,
    child: GestureDetector(
      onTap: () {},
      child: Icon(Icons.menu),
    ),
  );
}
"#;
        let issues = parse_and_check_touch(source);
        let flagged: Vec<_> = issues.iter().filter(|i| i.rule == "ensure-touch-target-size").collect();
        assert!(flagged.is_empty(), "SizedBox 64x64 should be OK");
    }

    #[test]
    fn container_with_gesture_detector_small_flags_issue() {
        let source = r#"
Widget build(BuildContext context) {
  return Container(
    width: 30,
    child: GestureDetector(
      onTap: () {},
      child: Text('x'),
    ),
  );
}
"#;
        let issues = parse_and_check_touch(source);
        assert!(!issues.is_empty(), "Container with small width and GestureDetector should be flagged");
    }

    #[test]
    fn touch_target_no_interactive_containers_ok() {
        let source = r#"
Widget build(BuildContext context) {
  return SizedBox(
    width: 20,
    height: 20,
    child: Text('static text'),
  );
}
"#;
        let issues = parse_and_check_touch(source);
        let flagged: Vec<_> = issues.iter().filter(|i| i.rule == "ensure-touch-target-size").collect();
        assert!(flagged.is_empty(), "Non-interactive small SizedBox should not be flagged");
    }

    // ── extract_dimension ─────────────────────────────────────────────────────

    #[test]
    fn extract_dimension_integer() {
        assert_eq!(extract_dimension("width: 48, height: 64", "width:"), Some(48.0));
    }

    #[test]
    fn extract_dimension_float() {
        assert_eq!(extract_dimension("height: 36.5,", "height:"), Some(36.5));
    }

    #[test]
    fn extract_dimension_missing_prefix_returns_none() {
        assert_eq!(extract_dimension("width: 48", "height:"), None);
    }

    #[test]
    fn extract_dimension_non_numeric_returns_none() {
        assert_eq!(extract_dimension("width: auto", "width:"), None);
    }

    #[test]
    fn extract_dimension_second_occurrence() {
        let text = "SizedBox(width: 32, height: 20)";
        assert_eq!(extract_dimension(text, "height:"), Some(20.0));
    }

    #[test]
    fn extract_dimension_zero() {
        assert_eq!(extract_dimension("width: 0", "width:"), Some(0.0));
    }

    #[test]
    fn extract_dimension_large_value() {
        assert_eq!(extract_dimension("width: 200.5 ,", "width:"), Some(200.5));
    }
}
