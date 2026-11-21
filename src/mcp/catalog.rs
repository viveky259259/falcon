//! MCP prompts and resources.
//!
//! Prompts give an agent ready-made workflows (the build-check-see loop, and
//! a review of AI-written code). Resources expose the rule catalogue and the
//! state of the app started by `run_app`.

use serde_json::{json, Value};

const DEV_LOOP: &str = "dev_loop";
const REVIEW: &str = "review_ai_code";

fn prompts() -> Value {
    json!([
        {
            "name": DEV_LOOP,
            "title": "Build, check and see a Flutter change",
            "description": "Work on a Flutter change in a loop: edit, lint, hot reload, read errors, look at the screen and use the app until the change is proven.",
            "arguments": [
                { "name": "goal", "description": "What the change should do", "required": true },
                { "name": "path", "description": "Flutter project directory (default .)", "required": false },
                { "name": "device", "description": "Device id, e.g. emulator-5554 or flutter-tester", "required": false }
            ]
        },
        {
            "name": REVIEW,
            "title": "Review AI-written Dart code",
            "description": "Check changed Dart code with Falcon's rules, explain and fix what it finds, and report the quality score.",
            "arguments": [
                { "name": "path", "description": "Project directory (default .)", "required": false },
                { "name": "base_ref", "description": "Git ref to diff against (default origin/main)", "required": false }
            ]
        }
    ])
}

fn arg<'a>(params: &'a Value, name: &str) -> Option<&'a str> {
    params["arguments"][name]
        .as_str()
        .filter(|s| !s.trim().is_empty())
}

fn get_prompt(params: &Value) -> Result<Value, String> {
    let name = params["name"].as_str().ok_or("prompts/get needs `name`")?;
    let path = arg(params, "path").unwrap_or(".");
    let text = match name {
        DEV_LOOP => {
            let goal = arg(params, "goal").ok_or("dev_loop needs the `goal` argument")?;
            let device = arg(params, "device")
                .map(|d| format!(", \"device\": \"{d}\""))
                .unwrap_or_default();
            format!(
                "Goal: {goal}\n\n\
                 Work in this loop with the falcon tools, and don't call the change done until you have seen it work.\n\
                 1. run_app {{\"path\": \"{path}\"{device}}} once. It keeps the app running for the other tools.\n\
                 2. Edit the code. After each edit run lint_file on the files you changed and fix what it reports.\n\
                 3. reload. If it fails or reports errors, fix them and reload again.\n\
                 4. logs {{\"errors_only\": true, \"since\": <next_cursor from the last call>}} to catch runtime exceptions and overflows.\n\
                 5. widget_tree to read the screen, and screenshot to see it.\n\
                 6. interact to tap, type and scroll through the flow the goal describes. If a find matches several widgets, pick one with index.\n\
                 7. Repeat until the screen and logs show the goal is met, then stop_app and summarise what you verified, with the final screenshot."
            )
        }
        REVIEW => {
            let base = arg(params, "base_ref").unwrap_or("origin/main");
            format!(
                "Review the Dart code changed in {path} against {base}.\n\
                 1. lint_diff {{\"path\": \"{path}\", \"base_ref\": \"{base}\"}}.\n\
                 2. For each rule you don't recognise, call explain.\n\
                 3. fix_safe {{\"path\": \"{path}\"}} previews safe fixes; apply the ones that are right, fix the rest by hand.\n\
                 4. Run lint_diff again until it is clean, then score {{\"path\": \"{path}\"}}.\n\
                 Report what you changed, anything left and why, and the score."
            )
        }
        other => return Err(format!("Unknown prompt: {other}")),
    };
    Ok(json!({
        "messages": [{ "role": "user", "content": { "type": "text", "text": text } }]
    }))
}

fn resources() -> Value {
    json!([
        {
            "uri": "falcon://rules",
            "name": "rules",
            "title": "Falcon lint rules",
            "description": "Every rule Falcon can report, with category, severity and a one-line summary.",
            "mimeType": "application/json"
        },
        {
            "uri": "falcon://session",
            "name": "session",
            "title": "Running app",
            "description": "The app started by run_app: project, device, VM Service URI and log cursor.",
            "mimeType": "application/json"
        }
    ])
}

fn templates() -> Value {
    json!([{
        "uriTemplate": "falcon://rules/{name}",
        "name": "rule",
        "title": "One Falcon rule",
        "description": "Rationale, good and bad examples and exceptions for one rule.",
        "mimeType": "application/json"
    }])
}

fn rule_json(name: &str) -> Value {
    match crate::ai::explain::explain_rule(name) {
        Some(e) => json!({
            "rule": e.name, "category": e.category, "severity": e.severity, "summary": e.summary,
            "why": e.why, "bad_example": e.bad_example, "good_example": e.good_example,
            "exceptions": e.exceptions,
        }),
        None => json!({ "rule": name }),
    }
}

fn read_resource(params: &Value) -> Result<Value, String> {
    let uri = params["uri"].as_str().ok_or("resources/read needs `uri`")?;
    let body = if uri == "falcon://rules" {
        let rules: Vec<Value> = crate::rules::all_rule_names()
            .iter()
            .map(|name| {
                let full = rule_json(name);
                json!({
                    "rule": name,
                    "category": full["category"],
                    "severity": full["severity"],
                    "summary": full["summary"],
                })
            })
            .collect();
        json!({ "rules": rules })
    } else if uri == "falcon://session" {
        super::app_session::status()
    } else if let Some(name) = uri.strip_prefix("falcon://rules/") {
        if !crate::rules::all_rule_names().iter().any(|r| r == name) {
            return Err(format!("Unknown rule: {name}"));
        }
        rule_json(name)
    } else {
        return Err(format!("Unknown resource: {uri}"));
    };
    Ok(json!({
        "contents": [{
            "uri": uri,
            "mimeType": "application/json",
            "text": serde_json::to_string_pretty(&body).unwrap_or_default(),
        }]
    }))
}

/// Handle a prompts/* or resources/* request.
pub fn handle(method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "prompts/list" => Ok(json!({ "prompts": prompts() })),
        "prompts/get" => get_prompt(params),
        "resources/list" => Ok(json!({ "resources": resources() })),
        "resources/templates/list" => Ok(json!({ "resourceTemplates": templates() })),
        "resources/read" => read_resource(params),
        other => Err(format!("Method not found: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_loop_prompt_needs_goal_and_mentions_the_tools() {
        let err = handle("prompts/get", &json!({"name": DEV_LOOP})).unwrap_err();
        assert!(err.contains("goal"));
        let v = handle(
            "prompts/get",
            &json!({"name": DEV_LOOP, "arguments": {"goal": "add a logout button", "device": "macos"}}),
        )
        .unwrap();
        let text = v["messages"][0]["content"]["text"].as_str().unwrap();
        for tool in [
            "run_app",
            "lint_file",
            "reload",
            "logs",
            "widget_tree",
            "screenshot",
            "interact",
        ] {
            assert!(text.contains(tool), "prompt should mention {tool}");
        }
        assert!(text.contains("\"device\": \"macos\""));
    }

    #[test]
    fn unknown_prompt_is_an_error() {
        assert!(handle("prompts/get", &json!({"name": "x"})).is_err());
    }

    #[test]
    fn rules_resource_lists_every_rule() {
        let v = handle("resources/read", &json!({"uri": "falcon://rules"})).unwrap();
        let body: Value = serde_json::from_str(v["contents"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(
            body["rules"].as_array().unwrap().len(),
            crate::rules::all_rule_names().len()
        );
    }

    #[test]
    fn single_rule_and_session_resources() {
        let name = crate::rules::all_rule_names().into_iter().next().unwrap();
        let v = handle(
            "resources/read",
            &json!({"uri": format!("falcon://rules/{name}")}),
        )
        .unwrap();
        assert!(v["contents"][0]["text"].as_str().unwrap().contains(&name));
        assert!(handle(
            "resources/read",
            &json!({"uri": "falcon://rules/not-a-rule"})
        )
        .is_err());
        let s = handle("resources/read", &json!({"uri": "falcon://session"})).unwrap();
        assert!(s["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("running"));
    }
}
