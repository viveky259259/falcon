pub mod cyclomatic;
pub mod lines;
pub mod maintainability;
pub mod nesting;
pub mod parameters;
pub mod methods;

use crate::config::{MetricsConfig, Severity};
use crate::reporters::Issue;
use tree_sitter::Node;

#[derive(Debug, Clone)]
pub struct FunctionMetrics {
    pub name: String,
    pub line: usize,
    pub cyclomatic_complexity: u32,
    pub lines_of_code: u32,
    pub source_lines_of_code: u32,
    pub maintainability_index: f64,
    pub max_nesting_level: u32,
    pub number_of_parameters: u32,
}

#[derive(Debug, Clone)]
pub struct ClassMetrics {
    pub name: String,
    pub line: usize,
    pub number_of_methods: u32,
    pub lines_of_code: u32,
}

#[derive(Debug, Clone)]
pub struct MetricsResults {
    pub file_lines_of_code: u32,
    pub file_source_lines_of_code: u32,
    pub functions: Vec<FunctionMetrics>,
    pub classes: Vec<ClassMetrics>,
}

impl MetricsResults {
    pub fn violations(&self) -> Vec<Issue> {
        Vec::new()
    }
}

pub fn calculate_file_metrics(
    root: Node,
    source: &str,
    config: &MetricsConfig,
) -> MetricsResults {
    let (loc, sloc) = lines::count_lines(source);

    let functions = collect_function_metrics(root, source, config);
    let classes = collect_class_metrics(root, source, config);

    MetricsResults {
        file_lines_of_code: loc,
        file_source_lines_of_code: sloc,
        functions,
        classes,
    }
}

fn collect_function_metrics(
    root: Node,
    source: &str,
    _config: &MetricsConfig,
) -> Vec<FunctionMetrics> {
    let mut results = Vec::new();
    let function_nodes = crate::parser::find_descendants_by_kind(root, "function_signature");
    let method_nodes = crate::parser::find_descendants_by_kind(root, "method_signature");

    let all_nodes: Vec<Node> = function_nodes
        .into_iter()
        .chain(method_nodes.into_iter())
        .collect();

    for node in all_nodes {
        let name = crate::parser::dart_ast::get_declaration_name(node, source)
            .unwrap_or("<anonymous>")
            .to_string();
        let line = crate::parser::node_start_line(node);

        let body = crate::parser::dart_ast::get_function_body(node);
        let cc = body
            .map(|b| cyclomatic::calculate(b, source))
            .unwrap_or(1);

        let (func_loc, func_sloc) = if let Some(parent) = node.parent() {
            let text = &source[parent.byte_range()];
            lines::count_lines(text)
        } else {
            (0, 0)
        };

        let max_nesting = body
            .map(|b| nesting::calculate(b))
            .unwrap_or(0);

        let num_params = parameters::count(node);

        let mi = maintainability::calculate(cc, func_sloc, 0.0);

        results.push(FunctionMetrics {
            name,
            line,
            cyclomatic_complexity: cc,
            lines_of_code: func_loc,
            source_lines_of_code: func_sloc,
            maintainability_index: mi,
            max_nesting_level: max_nesting,
            number_of_parameters: num_params,
        });
    }

    results
}

fn collect_class_metrics(
    root: Node,
    source: &str,
    _config: &MetricsConfig,
) -> Vec<ClassMetrics> {
    let mut results = Vec::new();
    let class_nodes =
        crate::parser::find_descendants_by_kind(root, "class_declaration");

    for node in class_nodes {
        let name = crate::parser::dart_ast::get_declaration_name(node, source)
            .unwrap_or("<anonymous>")
            .to_string();
        let line = crate::parser::node_start_line(node);

        let num_methods = methods::count(node) as u32;
        let class_text = &source[node.byte_range()];
        let (class_loc, _) = lines::count_lines(class_text);

        results.push(ClassMetrics {
            name,
            line,
            number_of_methods: num_methods,
            lines_of_code: class_loc,
        });
    }

    results
}

pub fn check_metric_threshold(
    value: u32,
    threshold: u32,
    metric_name: &str,
    entity_name: &str,
    file: &std::path::Path,
    line: usize,
) -> Option<Issue> {
    if value > threshold {
        Some(Issue {
            rule: format!("metrics/{}", metric_name),
            message: format!(
                "{} has {} of {} (threshold: {})",
                entity_name, metric_name, value, threshold
            ),
            severity: Severity::Warning,
            file: file.to_path_buf(),
            line,
            column: 1,
        })
    } else {
        None
    }
}
