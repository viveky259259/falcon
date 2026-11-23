//! The `interact` MCP tool: tap, type, scroll, go back and wait in the
//! running app. See `crate::runtime::interact` for how it works.

use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

use super::runtime_tools::{connect, refuse_remote, resolve_target, run_async, runtime_schema};
use super::tools::ToolDefinition;
use crate::doctor::Trust;
use crate::runtime::interact::{self, Action, Finder};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InteractArgs {
    #[serde(default)]
    attach: Option<String>,
    #[serde(default)]
    path: Option<String>,
    action: String,
    #[serde(default)]
    find: Option<Finder>,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    submit: bool,
    #[serde(default)]
    x: Option<f64>,
    #[serde(default)]
    y: Option<f64>,
    #[serde(default)]
    dx: f64,
    #[serde(default)]
    dy: f64,
    #[serde(default)]
    within: Option<Finder>,
    #[serde(default)]
    gone: bool,
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    5_000
}

pub const ACTIONS: &[&str] = &["tap", "long_press", "type", "scroll", "back", "wait"];

fn finder_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "description": description,
        "properties": {
            "text": { "type": "string", "description": "Exact text of a Text widget or text field" },
            "text_contains": { "type": "string" },
            "key": { "type": "string", "description": "ValueKey value, e.g. 'login_button'" },
            "type": { "type": "string", "description": "Widget type, e.g. ElevatedButton" },
            "tooltip": { "type": "string", "description": "Tooltip message, e.g. 'Increment' on a FloatingActionButton" },
            "label": { "type": "string", "description": "Semantics label" }
        }
    })
}

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "interact".to_string(),
        description: "Use the running app like a person: tap, long_press, type into a field, scroll, go back, or wait for a widget. Find the widget with `find` (text, key, type, tooltip or label; all given fields must match). A find that matches several widgets is refused and lists them, so pick one with `index`; nothing is tapped on a guess. `tap` and `long_press` also take `index` alone, meaning that element of `widget_tree`. Returns what it acted on, what appeared and disappeared, and the screen afterwards. Needs a debug build.".to_string(),
        input_schema: runtime_schema(
            json!({
                "action": { "type": "string", "enum": ACTIONS, "description": "What to do" },
                "find": finder_schema("Which widget to act on (tap, long_press, type, scroll into view, wait)"),
                "index": { "type": "integer", "description": "With `find`: which match to use when it matches several. Without `find` (tap, long_press): the element `widget_tree` lists at this index" },
                "text": { "type": "string", "description": "Text to enter (type). Replaces the field's text." },
                "submit": { "type": "boolean", "default": false, "description": "Press done after typing" },
                "x": { "type": "number", "description": "Tap at this logical-pixel x instead of using find" },
                "y": { "type": "number", "description": "Tap at this logical-pixel y (with x)" },
                "dx": { "type": "number", "default": 0, "description": "Scroll by this many logical pixels (horizontal lists)" },
                "dy": { "type": "number", "default": 0, "description": "Scroll by this many logical pixels; positive scrolls forward" },
                "within": finder_schema("Scroll the list inside this widget instead of the first one on screen"),
                "gone": { "type": "boolean", "default": false, "description": "wait: wait for the widget to disappear instead" },
                "timeout_ms": { "type": "integer", "default": 5000, "description": "wait: how long to wait" }
            }),
            &["action"],
        ),
    }
}

/// Build the action from the tool arguments.
fn to_action(a: &InteractArgs) -> Result<Action, String> {
    let finder = || -> Result<Finder, String> {
        a.find
            .clone()
            .filter(|f| !f.is_empty())
            .ok_or_else(|| format!("`{}` needs `find` with at least one field", a.action))
    };
    // `index` without `find` is a number from `widget_tree`.
    let no_find = a.find.as_ref().is_none_or(Finder::is_empty);
    if let (Some(index), true, None, None) = (a.index, no_find, a.x, a.y) {
        match a.action.as_str() {
            "tap" => return Ok(Action::TapElement { index, long: false }),
            "long_press" => return Ok(Action::TapElement { index, long: true }),
            _ => {}
        }
    }
    Ok(match a.action.as_str() {
        "tap" => match (a.x, a.y) {
            (Some(x), Some(y)) => Action::TapAt { x, y },
            (None, None) => Action::Tap {
                finder: finder()?,
                index: a.index,
            },
            _ => return Err("tap at a position needs both `x` and `y`".to_string()),
        },
        "long_press" => Action::LongPress {
            finder: finder()?,
            index: a.index,
        },
        "type" => Action::Type {
            finder: finder()?,
            index: a.index,
            text: a.text.clone().ok_or("`type` needs `text`")?,
            submit: a.submit,
        },
        "scroll" => match &a.find {
            Some(f) if !f.is_empty() => Action::ScrollTo {
                finder: f.clone(),
                index: a.index,
            },
            _ if a.dx != 0.0 || a.dy != 0.0 => Action::ScrollBy {
                within: a.within.clone().filter(|f| !f.is_empty()),
                dx: a.dx,
                dy: a.dy,
            },
            _ => return Err("`scroll` needs `find` (scroll it into view) or `dx`/`dy`".to_string()),
        },
        "back" => Action::Back,
        "wait" => Action::Wait {
            finder: finder()?,
            gone: a.gone,
            timeout: Duration::from_millis(a.timeout_ms.clamp(100, 60_000)),
        },
        other => {
            return Err(format!(
                "Unknown action '{other}': use one of {}",
                ACTIONS.join(", ")
            ))
        }
    })
}

pub fn execute(args: &Value, trust: Trust) -> Result<Value, String> {
    refuse_remote(trust, "interact")?;
    let a: InteractArgs = serde_json::from_value(args.clone())
        .map_err(|e| format!("Invalid interact arguments: {}", e))?;
    let action = to_action(&a)?;
    let (uri, _) = resolve_target(a.attach.as_deref(), a.path.as_deref())?;
    let deadline = 30 + a.timeout_ms / 1000;
    run_async("interact", deadline, async {
        let client = connect(&uri).await?;
        interact::perform(&client, &action).await
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(v: Value) -> Result<Action, String> {
        to_action(&serde_json::from_value(v).unwrap())
    }

    #[test]
    fn builds_actions() {
        assert!(matches!(
            action(json!({"action": "tap", "find": {"tooltip": "Increment"}})).unwrap(),
            Action::Tap { .. }
        ));
        assert_eq!(
            action(json!({"action": "tap", "x": 10.0, "y": 20.0})).unwrap(),
            Action::TapAt { x: 10.0, y: 20.0 }
        );
        assert!(matches!(
            action(json!({"action": "scroll", "dy": 300})).unwrap(),
            Action::ScrollBy { .. }
        ));
        assert!(matches!(
            action(json!({"action": "scroll", "find": {"text": "Item 40"}})).unwrap(),
            Action::ScrollTo { .. }
        ));
        assert_eq!(action(json!({"action": "back"})).unwrap(), Action::Back);
    }

    #[test]
    fn index_alone_means_a_widget_tree_element() {
        assert_eq!(
            action(json!({"action": "tap", "index": 3})).unwrap(),
            Action::TapElement {
                index: 3,
                long: false
            }
        );
        assert_eq!(
            action(json!({"action": "tap", "index": 0, "find": {}})).unwrap(),
            Action::TapElement {
                index: 0,
                long: false
            }
        );
        assert_eq!(
            action(json!({"action": "long_press", "index": 2})).unwrap(),
            Action::TapElement {
                index: 2,
                long: true
            }
        );
        // With `find`, `index` still picks among the matches.
        assert!(matches!(
            action(json!({"action": "tap", "index": 1, "find": {"type": "IconButton"}})).unwrap(),
            Action::Tap { index: Some(1), .. }
        ));
        // `type` needs a text field, so it still asks for `find`.
        assert!(action(json!({"action": "type", "index": 1, "text": "a"}))
            .unwrap_err()
            .contains("find"));
    }

    #[test]
    fn rejects_incomplete_actions() {
        assert!(action(json!({"action": "tap"}))
            .unwrap_err()
            .contains("find"));
        assert!(action(json!({"action": "tap", "find": {}}))
            .unwrap_err()
            .contains("find"));
        assert!(action(json!({"action": "tap", "x": 1.0}))
            .unwrap_err()
            .contains("both"));
        assert!(action(json!({"action": "type", "find": {"key": "email"}}))
            .unwrap_err()
            .contains("text"));
        assert!(action(json!({"action": "scroll"}))
            .unwrap_err()
            .contains("dx"));
        assert!(action(json!({"action": "fly"}))
            .unwrap_err()
            .contains("Unknown action"));
    }

    #[test]
    fn rejects_unknown_fields_and_remote() {
        let err = execute(&json!({"action": "back", "force": true}), Trust::Local).unwrap_err();
        assert!(err.contains("unknown field"), "{}", err);
        let err = execute(
            &json!({"action": "back", "find": {"txt": "a"}}),
            Trust::Local,
        )
        .unwrap_err();
        assert!(err.contains("unknown field"), "{}", err);
        let err = execute(&json!({"action": "back"}), Trust::Remote).unwrap_err();
        assert!(err.contains("not permitted"), "{}", err);
    }
}
