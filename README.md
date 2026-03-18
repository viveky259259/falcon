# Falcon

**Rust-powered static analysis for Flutter/Dart — designed for AI-generated code.**

![Falcon AI Score](https://img.shields.io/badge/Falcon_AI_Score-72/100-yellow)
![License](https://img.shields.io/badge/license-MIT-blue)
![Rules](https://img.shields.io/badge/rules-58%2B-brightgreen)
![Tests](https://img.shields.io/badge/tests-286-brightgreen)

Falcon is the only Flutter linter specifically designed to catch the bugs AI tools leave behind — missing dispose() calls, empty catch blocks, unawaited futures, hardcoded credentials, and 54 other rules. It's 10-100x faster than alternatives, scores your code 0-100, and integrates with AI tools via MCP for real-time self-correction.

## Quick Start

```bash
# Install
cargo install --git https://github.com/viveky259259/falcon

# Score your project (2 seconds)
falcon ai-score .

# Full analysis
falcon analyze .
```

## AI Code Quality Score

```
$ falcon ai-score .

  AI Code Quality Score: 72/100 (Grade: C)

    Resource Safety      85/100 ████████████████░░░░
    Error Handling       45/100 █████████░░░░░░░░░░░ · 8 empty catches
    Type Safety          90/100 ██████████████████░░
    Security            100/100 ████████████████████
    Convention Match     68/100 █████████████░░░░░░░
    Complexity           55/100 ███████████░░░░░░░░░
```

## Features

| Feature | Command | Description |
|---|---|---|
| **AI Score** | `falcon ai-score` | 0-100 score with 6-dimension breakdown |
| **58+ Rules** | `falcon analyze` | Flutter, BLoC, Riverpod, accessibility rules |
| **MCP Server** | `falcon-mcp` | AI tools call Falcon during code generation |
| **PR Comments** | `falcon pr-comment` | Auto-post analysis on GitHub PRs |
| **Provenance** | `falcon provenance` | Detect AI-generated vs human-written files |
| **Performance** | `falcon check-perf` | DevTools-style rebuild/memory/render analysis |
| **Vulnerability** | `falcon vuln-scan` | Security radar with CWE classification |
| **Drift Detection** | `falcon drift` | Convention drift in new code |
| **Risk Prediction** | `falcon predict` | Predict production issues from patterns |
| **Test Generation** | `falcon test-gen` | Generate test stubs from code analysis |
| **Refactoring Sim** | `falcon refactor-sim` | "What if we migrate to Riverpod?" impact |
| **Upgrade Check** | `falcon upgrade-check` | Deprecated Flutter API detection |
| **Enterprise** | `falcon enterprise` | Policies, audit logs, compliance |
| **Certification** | `falcon certify` | Bronze/Silver/Gold/Platinum badges |
| **HTTP API** | `falcon api` | REST API for integrations |

[See all 74 commands →](docs/cli-reference.md)

## AI Tool Integration (MCP)

Add Falcon to Cursor, Windsurf, or any MCP-compatible AI tool:

```json
{
  "mcpServers": {
    "falcon": { "command": "falcon-mcp" }
  }
}
```

Falcon analyzes Flutter code in real-time during generation — the AI self-corrects before showing you buggy code.

## CI/CD Integration

### GitHub Actions

```yaml
- name: Install Falcon
  run: cargo install --git https://github.com/viveky259259/falcon
- name: Analyze
  run: falcon analyze . --fail-on error
- name: PR Comment
  run: falcon pr-comment . --dry-run
```

### Presets

```bash
falcon analyze --preset ai-generated    # 20 rules for AI code
falcon analyze --preset strict          # All rules, max severity
falcon analyze --preset flutter         # Flutter best practices
```

## Pricing

| Tier | Price | What You Get |
|---|---|---|
| **Core CLI** | **Free forever** | All 74 commands, MCP server, LSP, API |
| **Team** | $9/seat/month | GitHub App, team dashboard, score trends |
| **Enterprise** | $29/seat/month | SSO, audit logs, policies, compliance |

The tool that makes AI code safe is free. The platform that proves it to your boss is paid.

## Performance

Falcon is written in Rust with rayon parallelism and tree-sitter parsing:

| Project Size | Analysis Time |
|---|---|
| 50 files | ~0.5s |
| 200 files | ~1.5s |
| 500 files | ~3s |

Compare: `dart analyze` takes 30-70 seconds on the same projects.

## Documentation

- [Getting Started](docs/getting-started.md)
- [CLI Reference](docs/cli-reference.md) (all 74 commands)
- [Rule Catalog](docs/rule-catalog.md) (58+ rules)
- [State of AI-Generated Flutter Code 2026](content/state-of-ai-flutter-code-2026.md)

## Contributing

Contributions welcome! Falcon is MIT-licensed.

```bash
git clone https://github.com/viveky259259/falcon
cd falcon
cargo test    # 286 tests
cargo build   # Fast build
```

## License

MIT — free for everyone, forever.

---

Built by [Vivek Yadav](https://github.com/viveky259259)
