# Falcon MCP server

`falcon-mcp` is the Model Context Protocol server for Falcon. It speaks
JSON-RPC over stdio so any MCP-aware agent (Claude Code, Cursor, etc.) can
call Falcon as a tool.

As of v0.5 the surface is **locked at 5 tools**. Older names (`falcon_*`) are
still accepted for one release as aliases — they are dispatched to the same
handler but do **not** appear in `tools/list` and emit a `log::warn!` line
when invoked.

## The 5 canonical tools

| Name        | What it does                                                                                                   | Required args     |
|-------------|----------------------------------------------------------------------------------------------------------------|-------------------|
| `lint_file` | Analyze a single Dart file. Fast — meant for "after you generated/edited a file" loops.                        | `file_path`       |
| `lint_diff` | Analyze only Dart files changed vs. `base_ref` (default `origin/main`).                                        | `path`            |
| `review`    | Full-project Falcon analysis. Returns issues with rule, severity, file, line, message.                         | `path`            |
| `explain`   | Explain a Falcon lint rule — rationale, good/bad examples, exceptions.                                         | `rule`            |
| `fix_safe`  | Generate auto-fix suggestions. Preview-only by default; pass `preview:false` to write changes.                 | `path`            |

All schemas are JSON-Schema draft-07 and surfaced via the standard MCP
`inputSchema` field on `tools/list`.

### `lint_diff`

`lint_diff` shells out to `git diff --name-only --diff-filter=ACMR
<base_ref>...HEAD`, keeps existing `.dart` files, and analyzes only that
scoped file list. It returns:

```json
{
  "path": "...",
  "base_ref": "origin/main",
  "changed_file_count": 1,
  "file_count": 1,
  "issue_count": 0,
  "changed_files": ["lib/main.dart"],
  "issues": []
}
```

If no Dart files changed, the tool returns zero counts and an empty issue
list. If the base ref is missing, the git error is returned to the caller.

## Deprecation table

These names continue to work for one release. They are **not advertised** in
`tools/list` and will be removed in a future version.

| Old name               | New canonical name | Status                                                       |
|------------------------|--------------------|--------------------------------------------------------------|
| `falcon_check_file`    | `lint_file`        | Alias. Logs deprecation warning.                             |
| `falcon_analyze`       | `review`           | Alias. Logs deprecation warning.                             |
| `falcon_explain_rule`  | `explain`          | Alias. Logs deprecation warning.                             |
| `falcon_fix`           | `fix_safe`         | Alias. Logs deprecation warning.                             |
| `falcon_ai_score`      | _none_             | Reachable by old name only; no canonical replacement.        |
| `falcon_conventions`   | _none_             | Reachable by old name only; no canonical replacement.        |
| `falcon_provenance`    | _none_             | Reachable by old name only; no canonical replacement.        |

## Client setup

### Claude Code (`.mcp.json`)

Drop this at the root of your repo (or merge into an existing `.mcp.json`):

```json
{
  "mcpServers": {
    "falcon": {
      "command": "falcon-mcp",
      "args": [],
      "env": {}
    }
  }
}
```

Then in Claude Code, `falcon` will appear as an MCP server and the 5 tools
above will be available as `mcp__falcon__lint_file`, `mcp__falcon__review`,
etc.

### Cursor (`.mcp.json`)

Cursor uses the same `.mcp.json` shape. Place it at the project root:

```json
{
  "mcpServers": {
    "falcon": {
      "command": "falcon-mcp",
      "args": []
    }
  }
}
```

If `falcon-mcp` is not on PATH, point `command` at the absolute binary path
(e.g. `/usr/local/bin/falcon-mcp` or `target/release/falcon-mcp`).

## Notes

- Transport is **stdio only**. There is no HTTP/WebSocket variant.
- The server name reported during `initialize` is `falcon`; the version
  matches the crate version.
- `cargo test mcp` exercises the surface contract; see
  `tests/mcp_tool_surface.rs` and the in-file tests in `src/mcp/tools.rs`.
