# Falcon Telemetry

Falcon telemetry is opt-in and off by default. The current implementation only
records local JSONL events for MCP tool invocations; it does not upload data.

## Enable Local Events

Set:

```bash
export FALCON_TELEMETRY=1
```

Events are written to:

```text
.falcon-data/telemetry/mcp-events.jsonl
```

Override the directory with:

```bash
export FALCON_TELEMETRY_DIR=/path/to/telemetry
```

Disable telemetry explicitly with:

```bash
export FALCON_TELEMETRY=0
```

## Event Shape

Each line is a JSON object:

```json
{
  "schema_version": 1,
  "event": "mcp_tool_invocation",
  "timestamp_unix_ms": 1780000000000,
  "source": "mcp:cursor",
  "tool": "lint_file",
  "canonical_tool": "lint_file",
  "success": true,
  "duration_ms": 12,
  "crate_version": "0.4.0"
}
```

`source` is classified from environment:

- `FALCON_INVOCATION_SOURCE` if provided
- `mcp:<FALCON_MCP_AGENT_ID>` if provided
- `cli:ci` when CI environment variables are present
- `mcp:unknown` otherwise for MCP calls

## Privacy

Local telemetry does not include source code, file paths, rule messages,
arguments, diagnostics, repository names, usernames, or hostnames. It records
only the tool name, mapped canonical tool name, success flag, duration, source
classification, timestamp, and Falcon version.

Falcon currently has no telemetry upload endpoint. If upload support is added,
it must remain opt-in and continue honoring `FALCON_TELEMETRY=0`.
