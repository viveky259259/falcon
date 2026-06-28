# Claude Code MCP Install

Falcon runs in Claude Code through `.mcp.json`. Install a Falcon release so
`falcon-mcp` is available on `PATH`, then add this at the root of the Flutter
repo:

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

If the binary is not on `PATH`, set `command` to an absolute path such as
`/usr/local/bin/falcon-mcp` or `target/release/falcon-mcp`.

## Verify

Restart Claude Code in the repo. The Falcon MCP server should expose exactly
these tools:

- `lint_file`
- `lint_diff`
- `review`
- `explain`
- `fix_safe`

Falcon uses stdio transport only. It does not open a local port or require auth.
