# Falcon Realtime Runtime Plan

## Goal

Turn Falcon into a live runtime copilot for Flutter apps:

1. attach to a running app or launch one
2. collect runtime telemetry continuously
3. detect actionable issues as they happen
4. explain likely root cause and next fix
5. verify whether the issue disappears after code changes

## Product Shape

Primary command:

```bash
falcon live --attach <vm-service-uri>
```

The command should stream issue cards while the app is running and print a final summary.

## Runtime Inputs

Falcon already has the runtime collectors needed for an MVP:

- logging streams
- HTTP/socket profiles
- memory usage
- process memory buckets
- performance timeline
- CPU profiler
- debugger state

## Issue Pipeline

### 1. Collect

Collect short-window runtime reports at a fixed interval. Each iteration should gather:

- logging report
- network report
- memory report

Later iterations can also enrich with:

- performance report
- profiler report
- debugger report

### 2. Normalize

Convert raw reports into a common issue model with:

- stable issue id
- severity
- category
- title
- summary
- evidence
- suggested fix
- optional source hint

### 3. Detect

MVP detectors:

- Flutter runtime/layout exception detector from logs
- failed HTTP request detector from network report
- slow HTTP request detector from network report
- high memory detector from memory report

Future detectors:

- jank detector
- CPU hotspot detector
- memory growth / leak detector
- stuck loading state detector
- rebuild storm detector

### 4. De-duplicate

Live mode should suppress repeated copies of the same issue within a session by using a stable fingerprint.

### 5. Verify

In a later phase, Falcon should compare issue fingerprints and metric deltas before and after a code change and mark issues:

- fixed
- improved
- unchanged

## MVP Scope

The first implementation should be intentionally narrow:

- new top-level `falcon live` command
- fixed-duration session with configurable interval
- JSON and console output
- runtime issue model
- detectors for:
  - layout/runtime exceptions
  - HTTP failures
  - slow HTTP requests
  - high memory

This keeps the first version useful without blocking on full source correlation.

## Architecture

Add a new module:

`src/runtime/live.rs`

Responsibilities:

- session config
- session runner
- issue model
- issue detectors
- console rendering

The session runner should:

1. establish a single VM service URI for the session
2. run repeated collection windows
3. create fresh clients per collector where needed
4. merge and print newly detected issues
5. emit a final summary report

## CLI Shape

```bash
falcon live [path] [--attach <uri>] [--duration 30] [--interval 10] [--json]
```

Recommended semantics:

- `duration`: total session duration in seconds
- `interval`: size of each collection window in seconds
- `--attach`: attach to a running app
- without `--attach`: launch via `flutter run`

## Detection Rules

### Layout / Runtime Exceptions

Input:

- stdout / stderr / logging entries

Patterns:

- `RenderFlex overflow`
- `A RenderFlex overflowed`
- `EXCEPTION CAUGHT BY`
- `Another exception was thrown`
- source location patterns like `/path/file.dart:123:45`

Output:

- severity: error
- category: `layout`
- suggested fix oriented around constraints, flex, wrapping, and scrollability

### HTTP Failures

Input:

- HTTP request summaries

Patterns:

- status >= 400

Output:

- severity depends on status class
- category: `network`
- fix suggestions based on status:
  - 401/403 auth
  - 404 endpoint/config
  - 429 rate limit / backoff
  - 5xx retry / resilience

### Slow HTTP Requests

Input:

- request duration

Patterns:

- duration above threshold

Output:

- severity warning
- category: `network`
- suggestions around retries, caching, pagination, background work

### High Memory

Input:

- heap usage
- external memory
- top process buckets

Patterns:

- heap usage above threshold

Output:

- severity warning/error
- category: `memory`
- suggestion to inspect top buckets / allocations and retained objects

## Next Phases

### Phase 2

- performance detector from timeline events
- CPU hotspot detector from profiler results
- memory growth detector across repeated windows
- baseline diffing

### Phase 3

- source-code correlation
- suggested file hints
- automated verification after edits
- optional fix recipes and patch generation

## Success Criteria For MVP

- Falcon can attach to a real Flutter app and surface runtime issues within a single session
- repeated identical issues are not spammed
- JSON output is machine-readable
- console output is readable for human debugging
- detectors are covered by unit tests
