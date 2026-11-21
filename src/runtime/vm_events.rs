//! Parsing and accumulation of VM Service stream events that Falcon relies on:
//!
//! - `Flutter.Frame` (Extension stream) — one event per rendered frame with
//!   build/raster timings in microseconds. Flutter posts these in debug and
//!   profile builds; there is no RPC that returns frame statistics.
//! - `Flutter.RebuiltWidgets` (Extension stream) — rebuild counts per widget
//!   location, sent once `ext.flutter.inspector.trackRebuildDirtyWidgets` is on.
//! - `ServiceRegistered` / `ServiceUnregistered` (Service stream) — methods such
//!   as `s1.reloadSources` that `flutter run` registers for real hot reload.

use serde_json::{json, Value};
use std::collections::HashMap;

/// Frames whose build or raster phase exceeds this budget count as dropped.
pub const FRAME_BUDGET_US: u64 = 16_667;

/// Upper bound on buffered stream notifications kept between RPC calls.
pub const MAX_BUFFERED_NOTIFICATIONS: usize = 10_000;

/// Timing for one rendered frame, from a `Flutter.Frame` event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameTiming {
    pub number: u64,
    pub elapsed_us: u64,
    pub build_us: u64,
    pub raster_us: u64,
}

impl FrameTiming {
    pub fn is_dropped(&self) -> bool {
        self.build_us > FRAME_BUDGET_US || self.raster_us > FRAME_BUDGET_US
    }
}

/// The `extensionKind` of a stream notification (`params` of `streamNotify`).
pub fn extension_kind(notification: &Value) -> Option<&str> {
    notification["event"]["extensionKind"].as_str()
}

/// Parse a `Flutter.Frame` notification into a `FrameTiming`.
pub fn parse_frame_event(notification: &Value) -> Option<FrameTiming> {
    if extension_kind(notification) != Some("Flutter.Frame") {
        return None;
    }
    let data = &notification["event"]["extensionData"];
    let field = |name: &str| {
        data[name]
            .as_u64()
            .or_else(|| data[name].as_f64().map(|v| v as u64))
    };
    Some(FrameTiming {
        number: field("number").unwrap_or(0),
        elapsed_us: field("elapsed")?,
        build_us: field("build").unwrap_or(0),
        raster_us: field("raster").unwrap_or(0),
    })
}

/// Running frame totals since the client started tracking frames.
#[derive(Debug, Clone, Default)]
pub struct FrameAccumulator {
    pub total: u64,
    pub dropped: u64,
    build_sum_us: u64,
    raster_sum_us: u64,
    pub build_max_us: u64,
    pub raster_max_us: u64,
}

impl FrameAccumulator {
    pub fn add(&mut self, frame: &FrameTiming) {
        self.total += 1;
        if frame.is_dropped() {
            self.dropped += 1;
        }
        self.build_sum_us += frame.build_us;
        self.raster_sum_us += frame.raster_us;
        self.build_max_us = self.build_max_us.max(frame.build_us);
        self.raster_max_us = self.raster_max_us.max(frame.raster_us);
    }

    pub fn avg_build_ms(&self) -> f64 {
        avg_ms(self.build_sum_us, self.total)
    }

    pub fn avg_raster_ms(&self) -> f64 {
        avg_ms(self.raster_sum_us, self.total)
    }
}

fn avg_ms(sum_us: u64, count: u64) -> f64 {
    if count == 0 {
        0.0
    } else {
        sum_us as f64 / count as f64 / 1000.0
    }
}

/// Where a tracked widget was created, from `widgetLocationIdMap`.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetLocation {
    pub name: String,
    pub file: String,
    pub line: i64,
}

/// Parse a location map (`{file: {ids, lines, names, ...}}`), as returned by
/// `ext.flutter.inspector.widgetLocationIdMap` (possibly wrapped in
/// `{"result": ...}`) or sent as `newLocations` in `Flutter.RebuiltWidgets`.
pub fn parse_location_map(value: &Value) -> HashMap<i64, WidgetLocation> {
    let mut map = HashMap::new();
    let files = match value.get("result") {
        Some(inner) if inner.is_object() => inner,
        _ => value,
    };
    let Some(files) = files.as_object() else {
        return map;
    };
    for (file, entry) in files {
        let (Some(ids), Some(names)) = (entry["ids"].as_array(), entry["names"].as_array()) else {
            continue;
        };
        let lines = entry["lines"].as_array();
        for (i, id) in ids.iter().enumerate() {
            let Some(id) = id.as_i64() else { continue };
            let name = names.get(i).and_then(Value::as_str).unwrap_or("<unknown>");
            let line = lines
                .and_then(|l| l.get(i))
                .and_then(Value::as_i64)
                .unwrap_or(0);
            map.insert(
                id,
                WidgetLocation {
                    name: name.to_string(),
                    file: file.clone(),
                    line,
                },
            );
        }
    }
    map
}

/// Running rebuild counts per widget location id.
#[derive(Debug, Clone, Default)]
pub struct RebuildTracker {
    pub enabled: bool,
    counts: HashMap<i64, u64>,
    locations: HashMap<i64, WidgetLocation>,
}

impl RebuildTracker {
    /// Fold one `Flutter.RebuiltWidgets` notification into the totals.
    pub fn add_event(&mut self, notification: &Value) {
        if extension_kind(notification) != Some("Flutter.RebuiltWidgets") {
            return;
        }
        let data = &notification["event"]["extensionData"];
        for key in ["locations", "newLocations"] {
            if data[key].is_object() {
                self.locations.extend(parse_location_map(&data[key]));
            }
        }
        // `events` is a flat list of [locationId, count, locationId, count, ...].
        if let Some(flat) = data["events"].as_array() {
            for pair in flat.chunks(2) {
                if let [id, count] = pair {
                    if let (Some(id), Some(count)) = (id.as_i64(), count.as_u64()) {
                        *self.counts.entry(id).or_insert(0) += count;
                    }
                }
            }
        }
    }

    pub fn unknown_ids(&self) -> bool {
        self.counts
            .keys()
            .any(|id| !self.locations.contains_key(id))
    }

    pub fn add_locations(&mut self, locations: HashMap<i64, WidgetLocation>) {
        self.locations.extend(locations);
    }

    /// Render the totals in the `{"events": [{name, count, location}]}` shape
    /// that `summarize_rebuilds` understands, most-rebuilt first.
    pub fn to_json(&self) -> Value {
        let mut rows: Vec<(i64, u64)> = self.counts.iter().map(|(k, v)| (*k, *v)).collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let events: Vec<Value> = rows
            .into_iter()
            .map(|(id, count)| match self.locations.get(&id) {
                Some(loc) => json!({
                    "name": loc.name,
                    "count": count,
                    "location": { "file": loc.file, "line": loc.line },
                }),
                None => json!({ "name": format!("<location {id}>"), "count": count }),
            })
            .collect();
        json!({ "enabled": self.enabled, "events": events })
    }
}

/// Apply a Service-stream notification to the registry of service methods
/// (`service name` → `registered method`, e.g. `reloadSources` → `s1.reloadSources`).
/// Returns true when the notification was a Service event.
pub fn apply_service_event(services: &mut HashMap<String, String>, notification: &Value) -> bool {
    if notification["streamId"].as_str() != Some("Service") {
        return false;
    }
    let event = &notification["event"];
    let (Some(service), Some(method)) = (event["service"].as_str(), event["method"].as_str())
    else {
        return true;
    };
    match event["kind"].as_str() {
        Some("ServiceRegistered") => {
            services.insert(service.to_string(), method.to_string());
        }
        Some("ServiceUnregistered") => {
            services.retain(|_, m| m != method);
        }
        _ => {}
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(number: u64, build: u64, raster: u64) -> Value {
        json!({"streamId": "Extension", "event": {
            "extensionKind": "Flutter.Frame",
            "extensionData": {"number": number, "startTime": 1, "elapsed": build + raster + 600,
                               "build": build, "raster": raster, "vsyncOverhead": 600}}})
    }

    #[test]
    fn parses_flutter_frame_events_and_ignores_others() {
        let f = parse_frame_event(&frame(8, 253, 533)).unwrap();
        assert_eq!(
            (f.number, f.build_us, f.raster_us, f.elapsed_us),
            (8, 253, 533, 1386)
        );
        let nav = json!({"event": {"extensionKind": "Flutter.Navigation", "extensionData": {}}});
        assert!(parse_frame_event(&nav).is_none());
    }

    #[test]
    fn accumulator_counts_dropped_frames_and_averages() {
        let mut acc = FrameAccumulator::default();
        for (b, r) in [(4_000, 6_000), (20_000, 5_000), (3_000, 30_000)] {
            acc.add(&parse_frame_event(&frame(1, b, r)).unwrap());
        }
        assert_eq!((acc.total, acc.dropped), (3, 2));
        assert!((acc.avg_build_ms() - 9.0).abs() < 1e-9);
        assert_eq!((acc.build_max_us, acc.raster_max_us), (20_000, 30_000));
    }

    #[test]
    fn rebuild_tracker_sums_pairs_and_names_locations() {
        let mut t = RebuildTracker {
            enabled: true,
            ..Default::default()
        };
        let ev = |events: Value| {
            json!({"event": {"extensionKind": "Flutter.RebuiltWidgets",
            "extensionData": {"frameNumber": 1, "events": events}}})
        };
        t.add_event(&ev(json!([184, 1, 7, 3])));
        t.add_event(&ev(json!([184, 5])));
        assert!(t.unknown_ids());
        t.add_locations(parse_location_map(&json!({"result": {
            "file:///app/lib/main.dart": {"ids": [184, 7], "lines": [40, 12], "names": ["Text", "MyHomePage"]}}})));
        assert!(!t.unknown_ids());
        let out = t.to_json();
        assert_eq!(out["enabled"], true);
        assert_eq!(out["events"][0]["name"], "Text");
        assert_eq!(out["events"][0]["count"], 6);
        assert_eq!(out["events"][0]["location"]["line"], 40);
        assert_eq!(out["events"][1]["name"], "MyHomePage");
    }

    #[test]
    fn service_registry_tracks_register_and_unregister() {
        let mut services = HashMap::new();
        let reg = json!({"streamId": "Service", "event": {"kind": "ServiceRegistered",
            "service": "reloadSources", "method": "s1.reloadSources", "alias": "Flutter Tools"}});
        assert!(apply_service_event(&mut services, &reg));
        assert_eq!(
            services.get("reloadSources").map(String::as_str),
            Some("s1.reloadSources")
        );
        let unreg = json!({"streamId": "Service", "event": {"kind": "ServiceUnregistered",
            "service": "reloadSources", "method": "s1.reloadSources"}});
        apply_service_event(&mut services, &unreg);
        assert!(services.is_empty());
        assert!(!apply_service_event(
            &mut services,
            &json!({"streamId": "Extension"})
        ));
    }
}
