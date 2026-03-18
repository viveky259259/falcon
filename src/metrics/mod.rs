pub mod class_metrics;
pub mod cyclomatic;
pub mod halstead;
pub mod lines;
pub mod maintainability;
pub mod methods;
pub mod nesting;
pub mod parameters;
pub mod widget_metrics;

use crate::config::{MetricsConfig, Severity};
use crate::reporters::Issue;
use tree_sitter::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdLevel {
    Ok,
    Noted,
    Warning,
    Alarm,
}

impl ThresholdLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ThresholdLevel::Ok => "ok",
            ThresholdLevel::Noted => "noted",
            ThresholdLevel::Warning => "warning",
            ThresholdLevel::Alarm => "alarm",
        }
    }

    pub fn to_severity(&self) -> Option<Severity> {
        match self {
            ThresholdLevel::Ok => None,
            ThresholdLevel::Noted => Some(Severity::Info),
            ThresholdLevel::Warning => Some(Severity::Warning),
            ThresholdLevel::Alarm => Some(Severity::Error),
        }
    }
}

pub fn threshold_level(value: u32, noted: u32, warning: u32, alarm: u32) -> ThresholdLevel {
    if value >= alarm {
        ThresholdLevel::Alarm
    } else if value >= warning {
        ThresholdLevel::Warning
    } else if value >= noted {
        ThresholdLevel::Noted
    } else {
        ThresholdLevel::Ok
    }
}

pub fn threshold_level_f64(value: f64, noted: f64, warning: f64, alarm: f64) -> ThresholdLevel {
    if value >= alarm {
        ThresholdLevel::Alarm
    } else if value >= warning {
        ThresholdLevel::Warning
    } else if value >= noted {
        ThresholdLevel::Noted
    } else {
        ThresholdLevel::Ok
    }
}

/// For inverted metrics where lower is worse (e.g. maintainability index)
pub fn threshold_level_inverted(value: f64, alarm: f64, warning: f64, noted: f64) -> ThresholdLevel {
    if value <= alarm {
        ThresholdLevel::Alarm
    } else if value <= warning {
        ThresholdLevel::Warning
    } else if value <= noted {
        ThresholdLevel::Noted
    } else {
        ThresholdLevel::Ok
    }
}

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
    pub halstead_volume: f64,
    pub halstead_difficulty: f64,
    pub widgets_nesting_level: u32,
    pub number_of_used_widgets: u32,
}

#[derive(Debug, Clone)]
pub struct ClassMetrics {
    pub name: String,
    pub line: usize,
    pub number_of_methods: u32,
    pub lines_of_code: u32,
    pub coupling_between_objects: u32,
    pub depth_of_inheritance: u32,
    pub number_of_added_methods: u32,
    pub number_of_interfaces: u32,
    pub number_of_overridden_methods: u32,
    pub response_for_class: u32,
    pub tight_class_cohesion: f64,
    pub weight_of_class: f64,
    pub weighted_methods_per_class: u32,
    pub lack_of_cohesion: u32,
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

        let max_nesting = body.map(|b| nesting::calculate(b)).unwrap_or(0);
        let num_params = parameters::count(node);

        let halstead = body
            .map(|b| halstead::calculate(b, source))
            .unwrap_or(halstead::HalsteadMetrics {
                unique_operators: 0,
                unique_operands: 0,
                total_operators: 0,
                total_operands: 0,
                vocabulary: 0,
                length: 0,
                volume: 0.0,
                difficulty: 0.0,
                effort: 0.0,
            });

        let mi = maintainability::calculate(cc, func_sloc, halstead.volume);

        let wnl = body
            .map(|b| widget_metrics::widgets_nesting_level(b, source))
            .unwrap_or(0);
        let wc = body
            .map(|b| widget_metrics::count_used_widgets(b, source))
            .unwrap_or(0);

        results.push(FunctionMetrics {
            name,
            line,
            cyclomatic_complexity: cc,
            lines_of_code: func_loc,
            source_lines_of_code: func_sloc,
            maintainability_index: mi,
            max_nesting_level: max_nesting,
            number_of_parameters: num_params,
            halstead_volume: halstead.volume,
            halstead_difficulty: halstead.difficulty,
            widgets_nesting_level: wnl,
            number_of_used_widgets: wc,
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

        let cbo = class_metrics::coupling_between_objects(node, source);
        let dit = class_metrics::depth_of_inheritance(node, source);
        let added = class_metrics::number_of_added_methods(node, source);
        let ifaces = class_metrics::number_of_interfaces(node, source);
        let overridden = class_metrics::number_of_overridden_methods(node, source);
        let rfc = class_metrics::response_for_class(node, source);
        let tcc = class_metrics::tight_class_cohesion(node, source);
        let woc = class_metrics::weight_of_class(node, source);
        let wmc = class_metrics::weighted_methods_per_class(node, source);
        let lcom = class_metrics::lack_of_cohesion(node, source);

        results.push(ClassMetrics {
            name,
            line,
            number_of_methods: num_methods,
            lines_of_code: class_loc,
            coupling_between_objects: cbo,
            depth_of_inheritance: dit,
            number_of_added_methods: added,
            number_of_interfaces: ifaces,
            number_of_overridden_methods: overridden,
            response_for_class: rfc,
            tight_class_cohesion: tcc,
            weight_of_class: woc,
            weighted_methods_per_class: wmc,
            lack_of_cohesion: lcom,
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
