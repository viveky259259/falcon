use crate::parser::walk_tree;
use std::collections::HashSet;
use tree_sitter::Node;

const FLUTTER_WIDGETS: &[&str] = &[
    "Widget", "StatelessWidget", "StatefulWidget",
    "Container", "Row", "Column", "Stack", "Wrap",
    "Scaffold", "AppBar", "BottomNavigationBar", "TabBar", "Drawer",
    "Text", "RichText", "SelectableText",
    "Icon", "IconButton", "Image",
    "ElevatedButton", "TextButton", "OutlinedButton", "FloatingActionButton",
    "TextField", "TextFormField",
    "ListView", "GridView", "SingleChildScrollView", "CustomScrollView",
    "Card", "ListTile", "ExpansionTile",
    "Padding", "Center", "Align", "Positioned",
    "Expanded", "Flexible", "Spacer", "SizedBox",
    "Opacity", "Visibility", "Offstage",
    "GestureDetector", "InkWell", "Dismissible", "Draggable",
    "AnimatedContainer", "AnimatedOpacity", "AnimatedBuilder",
    "Hero", "FadeTransition", "SlideTransition",
    "StreamBuilder", "FutureBuilder", "ValueListenableBuilder",
    "Form", "Checkbox", "Radio", "Switch", "Slider", "DropdownButton",
    "Dialog", "AlertDialog", "BottomSheet", "SnackBar",
    "PageView", "TabBarView",
    "ClipRRect", "ClipOval", "ClipPath",
    "Transform", "RotatedBox",
    "Table", "DataTable",
    "Divider", "CircularProgressIndicator", "LinearProgressIndicator",
    "Tooltip", "PopupMenuButton",
    "SafeArea", "MediaQuery", "LayoutBuilder",
    "IntrinsicHeight", "IntrinsicWidth",
    "AspectRatio", "FittedBox", "FractionallySizedBox",
    "ConstrainedBox", "UnconstrainedBox", "LimitedBox", "OverflowBox",
    "DecoratedBox", "ColoredBox", "PhysicalModel",
    "Navigator", "MaterialApp", "CupertinoApp",
    "Theme", "Material", "Ink",
];

/// Counts widgets nesting level: the maximum depth of widget constructors
/// nested inside a build method or function body.
pub fn widgets_nesting_level(node: Node, source: &str) -> u32 {
    let mut max_depth = 0u32;
    walk_widget_nesting(node, source, 0, &mut max_depth);
    max_depth
}

fn walk_widget_nesting(node: Node, source: &str, depth: u32, max_depth: &mut u32) {
    if depth > *max_depth {
        *max_depth = depth;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if is_widget_scope(child, source) {
            walk_widget_nesting(child, source, depth + 1, max_depth);
        } else {
            walk_widget_nesting(child, source, depth, max_depth);
        }
    }
}

/// A node is a widget scope if it has a direct identifier/type_identifier child
/// whose text matches a Flutter widget name. This covers return_statement,
/// expression_statement, named_argument, etc. that contain `WidgetName(...)`.
fn is_widget_scope(node: Node, source: &str) -> bool {
    let mut has_widget_name = false;
    let mut has_selector = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "type_identifier" {
            let text = &source[child.byte_range()];
            if FLUTTER_WIDGETS.contains(&text) {
                has_widget_name = true;
            }
        }
        if child.kind() == "selector" || child.kind() == "arguments" || child.kind() == "argument_part" {
            has_selector = true;
        }
    }

    has_widget_name && has_selector
}

/// Counts the number of distinct widget types used in the given node.
pub fn count_used_widgets(node: Node, source: &str) -> u32 {
    let mut used: HashSet<String> = HashSet::new();

    walk_tree(node, &mut |n| {
        if n.kind() == "identifier" || n.kind() == "type_identifier" {
            let text = &source[n.byte_range()];
            if FLUTTER_WIDGETS.contains(&text) {
                used.insert(text.to_string());
            }
        }
    });

    used.len() as u32
}
