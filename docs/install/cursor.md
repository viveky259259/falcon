# Cursor MCP Install

Falcon runs in Cursor through the MCP stdio server. Install a Falcon release so
`falcon-mcp` is available on `PATH`, then add this to the repo-level
`.mcp.json`:

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

Restart Cursor, open the repo, and confirm the Falcon MCP server lists exactly
these tools:

- `lint_file`
- `lint_diff`
- `review`
- `explain`
- `fix_safe`

Falcon uses stdio transport only. It does not open a local port or require auth.
