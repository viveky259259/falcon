use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Default, Serialize)]
pub(super) struct Tests {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub names: Vec<String>,
    pub errors: Vec<String>,
}

pub(super) fn summarize(bytes: &[u8], tests: &mut Tests) -> Result<()> {
    let text = std::str::from_utf8(bytes).context("Test events are not UTF-8")?;
    let mut active = HashMap::new();
    let mut seen = std::collections::HashSet::new();
    let mut started = false;
    let mut done = false;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        // Flutter also prints build progress while the loading test is active.
        // Only machine events contribute to test proof; retain all raw output.
        if !line.trim_start().starts_with(['{', '[']) {
            continue;
        }
        let event: Value = serde_json::from_str(line).context("Malformed test event JSON")?;
        if let Some(events) = event.as_array() {
            for event in events {
                match event["event"].as_str() {
                    Some("test.startedProcess") => {}
                    Some("test.crashedProcess") => tests
                        .errors
                        .push(format!("Native process crashed: {event}")),
                    _ => bail!("Unknown Flutter process event: {event}"),
                }
            }
            continue;
        }
        let kind = event["type"].as_str().context("Test event has no type")?;
        if done {
            bail!("Test event after final done event");
        }
        match kind {
            "start" => {
                if started {
                    bail!("Duplicate protocol start");
                }
                let version = event["protocolVersion"]
                    .as_str()
                    .context("Missing protocolVersion")?;
                if !version.starts_with("0.1.") {
                    bail!("Unsupported protocolVersion {version}");
                }
                started = true;
            }
            "testStart" => {
                if !started {
                    bail!("Test started before protocol start");
                }
                let id = event["test"]["id"].as_u64().context("Missing test ID")?;
                let name = event["test"]["name"]
                    .as_str()
                    .context("Missing test name")?;
                if !seen.insert(id) {
                    bail!("Duplicate test ID {id}");
                }
                active.insert(id, name.to_owned());
            }
            "testDone" => finish_test(&event, &mut active, tests)?,
            "error" => tests.errors.push(
                event["error"]
                    .as_str()
                    .unwrap_or("Unspecified test error")
                    .into(),
            ),
            "done" => {
                if !started || !active.is_empty() {
                    bail!("Incomplete test lifecycle");
                }
                if event["success"].as_bool() != Some(true) {
                    bail!("Test runner did not report success");
                }
                done = true;
            }
            "suite" | "group" | "allSuites" | "print" | "debug" => {}
            _ => bail!("Unknown test event type {kind}"),
        }
    }
    if !done {
        bail!("Missing final done event");
    }
    if tests.passed == 0 || tests.failed > 0 || tests.skipped > 0 || !tests.errors.is_empty() {
        bail!(
            "No complete passing run: {} passed, {} failed, {} skipped, {} errors",
            tests.passed,
            tests.failed,
            tests.skipped,
            tests.errors.len()
        );
    }
    Ok(())
}

fn finish_test(event: &Value, active: &mut HashMap<u64, String>, tests: &mut Tests) -> Result<()> {
    let id = event["testID"]
        .as_u64()
        .context("Missing completed test ID")?;
    let name = active
        .remove(&id)
        .with_context(|| format!("Unknown completed test ID {id}"))?;
    let result = event["result"].as_str().context("Missing test result")?;
    let hidden = event["hidden"].as_bool().context("Missing hidden flag")?;
    let skipped = event["skipped"].as_bool().context("Missing skipped flag")?;
    if result != "success" {
        tests.failed += 1;
    }
    if hidden {
        return Ok(());
    }
    tests.names.push(name);
    if skipped {
        tests.skipped += 1;
    } else if result == "success" {
        tests.passed += 1;
    }
    Ok(())
}
