//! Tool definitions (names, descriptions, input schemas) for the runtime
//! tools dispatched by `runtime_inspect`.

use serde_json::{json, Value};

use super::runtime_tools::runtime_schema;
use super::schema::JSON_SCHEMA_DRAFT_07;
use super::tools::ToolDefinition;

pub const METRIC_KINDS: &[&str] = &[
    "memory",
    "network",
    "performance",
    "profiler",
    "rebuilds",
    "trace",
    "routes",
];

fn def(name: &str, description: &str, input_schema: Value) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        input_schema,
    }
}

/// Definitions of every runtime tool `runtime_inspect` dispatches.
pub fn definitions() -> Vec<ToolDefinition> {
    let mut defs = lifecycle_definitions();
    defs.extend(observe_definitions());
    defs
}

/// `run_app`, `stop_app` and `reload`.
fn lifecycle_definitions() -> Vec<ToolDefinition> {
    vec![
        def(
            "run_app",
            "Start the Flutter app with `flutter run` and keep it running for the other runtime tools. Returns once the app is up (or fails with the build output). Calling it again for the same project returns the running app.",
            json!({
                "$schema": JSON_SCHEMA_DRAFT_07,
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": { "type": "string", "default": ".", "description": "Flutter project directory" },
                    "device": { "type": "string", "description": "Device id from `flutter devices`, e.g. emulator-5554, macos, chrome, or flutter-tester for a headless run" },
                    "target": { "type": "string", "description": "Entry point, e.g. lib/main_dev.dart" },
                    "flavor": { "type": "string", "description": "Build flavor, passed as --flavor" },
                    "extra_args": { "type": "array", "items": { "type": "string" }, "description": "More `flutter run` arguments, e.g. [\"--dart-define=ENV=dev\"]" },
                    "timeout_secs": { "type": "integer", "default": 300, "description": "How long to wait for the build and launch" }
                }
            }),
        ),
        def(
            "stop_app",
            "Stop the app started by run_app.",
            json!({ "$schema": JSON_SCHEMA_DRAFT_07, "type": "object", "additionalProperties": false, "properties": {} }),
        ),
        def(
            "reload",
            "Hot reload the running app after editing Dart code (or hot restart with full_restart). Through run_app this recompiles your changes and reports compile errors.",
            runtime_schema(json!({ "full_restart": { "type": "boolean", "default": false, "description": "Hot restart instead of hot reload (resets app state)" } }), &[]),
        ),
    ]
}

/// The tools that read or measure the running app.
fn observe_definitions() -> Vec<ToolDefinition> {
    vec![
        def(
            "widget_tree",
            "Read the running app's screen as text. mode=visible (default) lists on-screen text, buttons, fields and controls with their positions, ready to use with interact. mode=inspector returns the Flutter inspector's widget tree summary.",
            runtime_schema(
                json!({
                    "mode": { "type": "string", "enum": ["visible", "inspector"], "default": "visible", "description": "visible: on-screen elements with positions; inspector: widget tree summary" },
                    "limit": { "type": "integer", "default": 150, "description": "Most elements to list in visible mode" }
                }),
                &[],
            ),
        ),
        def(
            "logs",
            "Read what the app printed, including Flutter framework errors and exceptions. With run_app, returns buffered output since `since` (pass back next_cursor to get only new lines). Otherwise listens on the VM Service for listen_secs.",
            runtime_schema(
                json!({
                    "since": { "type": "integer", "default": 0, "description": "Cursor from a previous call's next_cursor" },
                    "errors_only": { "type": "boolean", "default": false, "description": "Only errors, with each Flutter error block kept whole" },
                    "limit": { "type": "integer", "default": 200, "description": "Most lines to return (the newest)" },
                    "listen_secs": { "type": "integer", "default": 3, "description": "Listening window when not using run_app" }
                }),
                &[],
            ),
        ),
        def(
            "tree_diff",
            "Compare the widget tree before and after a short window (duration_secs, during which you or the app can act) and report which widgets appeared or disappeared.",
            runtime_schema(json!({ "duration_secs": { "type": "integer", "default": 5, "description": "Seconds between the two snapshots (max 120)" } }), &[]),
        ),
        def(
            "app_metrics",
            "Measure the running app: memory, network, performance (frame timeline), profiler (CPU), rebuilds (widgets rebuilding most), trace (janky frames tied to rebuilds) or routes (navigation events).",
            runtime_schema(
                json!({
                    "kind": { "type": "string", "enum": METRIC_KINDS, "description": "What to measure" },
                    "duration_secs": { "type": "integer", "default": 5, "description": "Recording window for network, performance, profiler, trace and routes (max 120)" },
                    "jank_ms": { "type": "number", "default": 16, "description": "Frame time that counts as jank (trace)" }
                }),
                &["kind"],
            ),
        ),
        def(
            "journey",
            "Record the running app for duration_secs, taking a screenshot every interval_secs, and report each distinct screen with heap and dropped-frame figures. Returns the first screenshots of new screens as images.",
            runtime_schema(
                json!({
                    "device": { "type": "string", "description": "Device id for the `flutter screenshot` fallback" },
                    "duration_secs": { "type": "integer", "default": 15, "description": "Max 120" },
                    "interval_secs": { "type": "integer", "default": 3, "description": "Seconds between screenshots" },
                    "output_dir": { "type": "string", "description": "Defaults to a new temp directory" },
                    "max_images": { "type": "integer", "default": 4, "description": "Most new-screen screenshots to return inline (max 10)" }
                }),
                &[],
            ),
        ),
        def(
            "verify",
            "Run one of the app's integration tests (integration_test/...) on an exact device and keep the evidence: pass/fail per test, device details and logs.",
            json!({
                "$schema": JSON_SCHEMA_DRAFT_07,
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": { "type": "string", "default": ".", "description": "Flutter project directory" },
                    "target": { "type": "string", "description": "Test file inside integration_test/, e.g. integration_test/login_test.dart" },
                    "device": { "type": "string", "description": "Exact device id; never falls back to another device" },
                    "timeout_secs": { "type": "integer", "default": 180, "description": "Deadline in seconds, device discovery included" },
                    "output_dir": { "type": "string", "description": "New directory for evidence; defaults to a temp directory" },
                    "physical_camera": { "type": "boolean", "default": false, "description": "Require a physical iOS/Android device (for camera tests)" }
                },
                "required": ["target", "device"]
            }),
        ),
    ]
}
