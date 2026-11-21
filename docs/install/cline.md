# Cline MCP Install

Falcon runs in Cline through the same MCP stdio server used by Cursor and Claude
Code. Install a Falcon release so `falcon-mcp` is available on `PATH`, then add
Falcon to Cline's MCP server settings:

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

If the binary is not on `PATH`, set `command` to an absolute path such as
`/usr/local/bin/falcon-mcp` or `target/release/falcon-mcp`.

## Verify

Restart Cline, open the repo, and confirm Falcon exposes exactly these tools:

- `lint_file`
- `lint_diff`
- `review`
- `explain`
- `fix_safe`
- `doctor`
- `score`
- `conventions`
- `generate_tests`
- `run_app`
- `stop_app`
- `reload`
- `screenshot`
- `widget_tree`
- `interact`
- `logs`
- `tree_diff`
- `app_metrics`
- `journey`
- `verify`

Falcon uses stdio transport only. It does not open a local port or require auth.
