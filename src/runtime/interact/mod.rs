//! Drive a running Flutter app: describe the screen, tap, type, scroll, go
//! back and wait for widgets.
//!
//! Works on any debug-mode app reachable over the VM Service, with nothing
//! added to the app: each step evaluates a small Dart expression inside the
//! app (see [`dart`]). A finder that matches more than one widget is refused
//! with the candidates listed, unless the caller picks one with `index`.

pub mod dart;
mod select;

pub use select::{choose, label_changes, visible_only, Candidate};
use select::{describe_finder, hittable, labels, num, parse_candidates, records};

use super::connection::VmServiceClient;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Library whose scope the expressions are evaluated in.
const EVAL_LIBRARY: &str = "package:flutter/src/widgets/binding.dart";
/// How long to wait for the screen to stop changing after an action.
const SETTLE_MAX: Duration = Duration::from_millis(2500);
const SETTLE_POLL: Duration = Duration::from_millis(250);
/// Pages to scroll in each direction while searching a lazy list.
const MAX_SEARCH_PAGES: usize = 40;
const LONG_PRESS: Duration = Duration::from_millis(700);

static NEXT_POINTER: AtomicU32 = AtomicU32::new(7_000);

fn next_pointer() -> u32 {
    NEXT_POINTER.fetch_add(1, Ordering::Relaxed)
}

/// How to pick a widget. Every field given must match.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finder {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_contains: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl Finder {
    pub fn is_empty(&self) -> bool {
        self == &Finder::default()
    }
}

/// One step to perform.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Describe {
        limit: usize,
    },
    Tap {
        finder: Finder,
        index: Option<usize>,
    },
    TapAt {
        x: f64,
        y: f64,
    },
    LongPress {
        finder: Finder,
        index: Option<usize>,
    },
    Type {
        finder: Finder,
        index: Option<usize>,
        text: String,
        submit: bool,
    },
    ScrollTo {
        finder: Finder,
        index: Option<usize>,
    },
    ScrollBy {
        within: Option<Finder>,
        dx: f64,
        dy: f64,
    },
    Back,
    Wait {
        finder: Finder,
        gone: bool,
        timeout: Duration,
    },
}

/// The VM's expression compiler reads a single line; the expressions have
/// no line comments, so joining lines with spaces is safe.
fn one_line(expression: &str) -> String {
    expression
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn explain_eval_error(e: anyhow::Error) -> anyhow::Error {
    let msg = e.to_string();
    if msg.contains("\"code\":113") {
        anyhow::anyhow!("The app could not compile the expression (needs a debug build): {msg}")
    } else {
        e.context("Expression evaluation failed (needs a debug build started by `flutter run`)")
    }
}

/// `(isolate id, library id)` of the last [`EVAL_LIBRARY`] lookup. The id
/// is stable for the life of an isolate; a hot restart makes a new isolate.
static LIBRARY_ID: Mutex<Option<(String, String)>> = Mutex::new(None);

/// The id of [`EVAL_LIBRARY`] in the client's isolate, looked up once.
async fn eval_library(client: &VmServiceClient) -> Result<String> {
    let cached = LIBRARY_ID.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some((isolate, library)) = cached {
        if isolate == client.isolate_id {
            return Ok(library);
        }
    }
    let isolate = client.get_isolate().await?;
    let library = isolate["libraries"]
        .as_array()
        .and_then(|libs| {
            libs.iter()
                .find(|l| l["uri"].as_str() == Some(EVAL_LIBRARY))
                .and_then(|l| l["id"].as_str())
        })
        .context("Flutter widgets library not found; is this a Flutter app in debug mode?")?
        .to_string();
    *LIBRARY_ID.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((client.isolate_id.clone(), library.clone()));
    Ok(library)
}

/// Evaluate `expression` in [`EVAL_LIBRARY`] and return the string result,
/// retrying while the app is in the middle of a frame (see [`dart::BUSY`]).
pub async fn evaluate(client: &VmServiceClient, expression: &str) -> Result<String> {
    for _ in 0..BUSY_RETRIES {
        let value = evaluate_once(client, expression).await?;
        if value != dart::BUSY {
            return Ok(value);
        }
        tokio::time::sleep(BUSY_RETRY_DELAY).await;
    }
    bail!("The app stayed busy rendering frames; try again")
}

/// How often to retry an expression that ran mid-frame, and how long to wait.
const BUSY_RETRIES: usize = 100;
const BUSY_RETRY_DELAY: Duration = Duration::from_millis(7);

async fn evaluate_once(client: &VmServiceClient, expression: &str) -> Result<String> {
    let library = eval_library(client).await?;
    let resp = client
        .call(
            "evaluate",
            json!({
                "isolateId": client.isolate_id,
                "targetId": library,
                "expression": one_line(expression),
                "disableBreakpoints": true,
            }),
        )
        .await
        .map_err(explain_eval_error)?;
    if resp["type"].as_str().is_some_and(|t| t.ends_with("Error")) {
        bail!(
            "App threw while evaluating: {}",
            resp["message"].as_str().unwrap_or("unknown error")
        );
    }
    let mut value = resp["valueAsString"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    if resp["valueAsStringIsTruncated"].as_bool() == Some(true) {
        if let (Some(id), Some(len)) = (resp["id"].as_str(), resp["length"].as_u64()) {
            let full = client
                .call(
                    "getObject",
                    json!({"isolateId": client.isolate_id, "objectId": id, "offset": 0, "count": len}),
                )
                .await?;
            if let Some(s) = full["valueAsString"].as_str() {
                value = s.to_string();
            }
        }
    }
    Ok(value)
}

/// What is on screen right now.
pub async fn describe(client: &VmServiceClient, limit: usize) -> Result<Value> {
    let recs = records(&evaluate(client, &dart::describe_expression(limit)).await?)?;
    let head = &recs[0];
    let all: Vec<Candidate> = recs
        .iter()
        .skip(1)
        .map(|f| Candidate {
            index: 0,
            type_name: f.first().cloned().unwrap_or_default(),
            label: f.get(1).cloned().unwrap_or_default(),
            key: f.get(2).filter(|k| !k.is_empty()).cloned(),
            rect: [num(f, 3), num(f, 4), num(f, 5), num(f, 6)],
            hittable: hittable(f.get(7)),
        })
        .collect();
    let truncated = all.len() >= limit;
    let (elements, covered) = visible_only(all);
    Ok(json!({
        "screen": { "width": num(head, 1), "height": num(head, 2), "device_pixel_ratio": num(head, 3) },
        "coordinates": "logical pixels; rect is [left, top, width, height]; multiply by device_pixel_ratio for screenshot pixels",
        "elements": elements,
        "covered_count": covered,
        "truncated": truncated,
    }))
}

async fn find(client: &VmServiceClient, finder: &Finder, onscreen: bool) -> Result<Vec<Candidate>> {
    let raw = evaluate(client, &dart::find_expression(finder, onscreen, None, "")).await?;
    Ok(parse_candidates(&records(&raw)?))
}

async fn act_on(
    client: &VmServiceClient,
    finder: &Finder,
    index: Option<usize>,
    onscreen: bool,
    action: &str,
) -> Result<(Candidate, Vec<String>)> {
    let mut cands = find(client, finder, onscreen).await?;
    if !onscreen {
        // An off-screen widget always fails the hit test; that doesn't mean
        // it is covered, so don't let `choose` refuse it for that.
        for c in &mut cands {
            c.hittable = None;
        }
    }
    let chosen = choose(&cands, index, finder)?;
    let raw = evaluate(
        client,
        &dart::find_expression(finder, onscreen, Some(chosen), action),
    )
    .await?;
    let recs = records(&raw)?;
    Ok((cands[chosen].clone(), recs[0].clone()))
}

/// Run one action and report what it hit and what changed on screen.
pub async fn perform(client: &VmServiceClient, action: &Action) -> Result<Value> {
    if let Action::Describe { limit } = action {
        return describe(client, *limit).await;
    }
    if let Action::Wait {
        finder,
        gone,
        timeout,
    } = action
    {
        return wait(client, finder, *gone, *timeout).await;
    }
    let before = labels(&describe(client, 300).await?);
    let target = run_step(client, action).await?;
    let after_screen = settle(client).await?;
    let (appeared, disappeared) = label_changes(&before, &labels(&after_screen));
    Ok(json!({
        "done": true,
        "target": target,
        "appeared": appeared,
        "disappeared": disappeared,
        "screen_after": after_screen["elements"],
    }))
}

async fn run_step(client: &VmServiceClient, action: &Action) -> Result<Value> {
    let pid = next_pointer();
    let (cand, done) = match action {
        Action::Tap { finder, index } => {
            act_on(
                client,
                finder,
                *index,
                true,
                &dart::pointer_action(pid, true, true),
            )
            .await?
        }
        Action::LongPress { finder, index } => {
            let (cand, done) = act_on(
                client,
                finder,
                *index,
                true,
                &dart::pointer_action(pid, true, false),
            )
            .await?;
            tokio::time::sleep(LONG_PRESS).await;
            // Lift at the press position: the callback may have moved or
            // removed the widget, and the pointer must never stay down.
            let release =
                dart::pointer_at_expression(pid, num(&done, 1), num(&done, 2), false, true);
            records(&evaluate(client, &release).await?)?;
            (cand, done)
        }
        Action::Type {
            finder,
            index,
            text,
            submit,
        } => {
            act_on(
                client,
                finder,
                *index,
                true,
                &dart::type_action(pid, text, *submit),
            )
            .await?
        }
        Action::ScrollTo { finder, index } => {
            if find(client, finder, false).await?.is_empty() {
                search_by_paging(client, finder).await?;
            }
            act_on(client, finder, *index, false, dart::SHOW_ACTION).await?
        }
        Action::TapAt { x, y } => {
            records(
                &evaluate(
                    client,
                    &dart::pointer_at_expression(pid, *x, *y, true, true),
                )
                .await?,
            )?;
            return Ok(json!({ "position": [x, y] }));
        }
        Action::ScrollBy { within, dx, dy } => {
            let recs = records(
                &evaluate(
                    client,
                    &dart::scroll_by_expression(within.as_ref(), *dx, *dy, false),
                )
                .await?,
            )?;
            let f = &recs[0];
            return Ok(
                json!({ "scroll_offset_before": num(f, 1), "scroll_offset_after": num(f, 2) }),
            );
        }
        Action::Back => {
            records(&evaluate(client, &dart::back_expression()).await?)?;
            return Ok(json!({ "action": "back" }));
        }
        Action::Describe { .. } | Action::Wait { .. } => unreachable!("handled in perform"),
    };
    Ok(json!({ "widget": cand, "position": [num(&done, 1), num(&done, 2)] }))
}

/// Wait until two reads of the screen agree (animations and route
/// transitions done), up to [`SETTLE_MAX`]. Returns the last read.
async fn settle(client: &VmServiceClient) -> Result<Value> {
    let started = Instant::now();
    tokio::time::sleep(SETTLE_POLL).await;
    let mut last = describe(client, 300).await?;
    while started.elapsed() < SETTLE_MAX {
        tokio::time::sleep(SETTLE_POLL).await;
        let now = describe(client, 300).await?;
        let stable = now["elements"] == last["elements"];
        last = now;
        if stable {
            break;
        }
    }
    Ok(last)
}

/// Scroll page by page until `finder` matches a built widget: forward from
/// here to the end, then forward again from the top. Lazy lists only build
/// the rows near the viewport, so this is how far-away rows are found.
async fn search_by_paging(client: &VmServiceClient, finder: &Finder) -> Result<()> {
    for from_top in [false, true] {
        if from_top {
            evaluate(client, &dart::scroll_by_expression(None, -1e9, -1e9, false)).await?;
        }
        for _ in 0..MAX_SEARCH_PAGES {
            let raw = evaluate(client, &dart::scroll_by_expression(None, 0.8, 0.8, true)).await?;
            let moved = records(&raw)?
                .first()
                .map(|f| num(f, 1) != num(f, 2))
                .unwrap_or(false);
            tokio::time::sleep(Duration::from_millis(50)).await;
            if !find(client, finder, false).await?.is_empty() {
                return Ok(());
            }
            if !moved {
                break;
            }
        }
    }
    bail!(
        "No widget matches {}, even after scrolling through the list on screen",
        describe_finder(finder)
    )
}

async fn wait(
    client: &VmServiceClient,
    finder: &Finder,
    gone: bool,
    timeout: Duration,
) -> Result<Value> {
    let started = Instant::now();
    loop {
        let (cands, _) = visible_only(find(client, finder, true).await?);
        if cands.is_empty() == gone {
            return Ok(json!({
                "done": true,
                "waited_ms": started.elapsed().as_millis() as u64,
                "matches": cands,
            }));
        }
        if started.elapsed() >= timeout {
            bail!(
                "Timed out after {}ms waiting for {} to {}",
                timeout.as_millis(),
                describe_finder(finder),
                if gone { "disappear" } else { "appear" }
            );
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_line_joins_and_trims() {
        assert_eq!(
            one_line("(() {\n  final a = 1;\n\n  return a;\n})()"),
            "(() { final a = 1; return a; })()"
        );
    }

    #[test]
    fn finder_rejects_unknown_fields() {
        let err = serde_json::from_value::<Finder>(json!({"txt": "x"})).unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }
}
