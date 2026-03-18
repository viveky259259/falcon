# CLI Reference

## Core Analysis

| Command | Description |
|---|---|
| `falcon analyze [path]` | Full analysis (metrics + rules + unused detection) |
| `falcon metrics [path]` | Calculate code metrics only |
| `falcon ai-score [path]` | AI Code Quality Score (0-100) with 6-dimension breakdown |
| `falcon ai-report [path]` | Full "State of AI-Generated Flutter Code" report |

### falcon analyze

```bash
falcon analyze . --format console --preset ai-generated --fail-on error
```

| Flag | Description | Default |
|---|---|---|
| `--format` | Output format: console, json, html, sarif, codeclimate, checkstyle, sonar | console |
| `--output` | Output file path | falcon-report.html |
| `--config` | Path to falcon.yaml | auto-detect |
| `--since` | Only analyze files changed since git ref | none |
| `--baseline` | Only report new violations | false |
| `--fail-on` | Minimum severity to fail: error, warning, info | error |
| `--preset` | Rule preset: recommended, strict, flutter, ai-generated | none |

### falcon ai-score

```bash
falcon ai-score . --badge --json
```

| Flag | Description |
|---|---|
| `--badge` | Print shields.io badge markdown |
| `--json` | Output as JSON |

## Code Checks

| Command | Description |
|---|---|
| `falcon check-unused-code` | Unused code declarations |
| `falcon check-unused-files` | Unused Dart files |
| `falcon check-dependencies` | Unused pubspec.yaml dependencies |
| `falcon check-cycles` | Cyclic import dependencies |
| `falcon check-unused-params` | Unused function parameters |
| `falcon check-dead-code` | Unreachable code after return/throw |
| `falcon check-unused-l10n` | Unused localization keys |
| `falcon check-promoted-deps` | Over/under-promoted dependencies |
| `falcon check-layers` | Clean architecture layer enforcement |
| `falcon check-imports` | Import restriction rules |
| `falcon check-widgets` | Widget rebuild issues |
| `falcon check-async` | Async/await anti-patterns |
| `falcon check-platform` | Kotlin/Swift platform channel issues |
| `falcon check-codegen` | Code generation quality (.g.dart, .freezed.dart) |
| `falcon check-perf` | DevTools-style performance analysis |
| `falcon cognitive-complexity` | Function cognitive complexity |

## AI Features

| Command | Description |
|---|---|
| `falcon provenance [path]` | Detect AI-generated vs human-written files |
| `falcon conventions [path]` | Auto-detect team naming/architecture patterns |
| `falcon drift [path]` | Detect convention drift in new code |
| `falcon predict [path]` | Predict production risks from code patterns |
| `falcon discover-rules [path]` | Propose new rules from observed patterns |
| `falcon refactor-sim --scenario <s>` | Simulate refactoring impact |
| `falcon test-gen [path]` | Generate test stubs from code analysis |
| `falcon vuln-scan [path]` | Security vulnerability radar |
| `falcon upgrade-check [path]` | Flutter upgrade compatibility |

## CI/CD

| Command | Description |
|---|---|
| `falcon pr-comment [path]` | Post analysis results as GitHub PR comment |
| `falcon webhook --url <url>` | Send webhook notification |
| `falcon fix [path]` | Auto-fix lint issues |

## Platform

| Command | Description |
|---|---|
| `falcon cloud init --team <name>` | Initialize team cloud config |
| `falcon cloud add-project` | Register a project |
| `falcon cloud dashboard` | Team dashboard |
| `falcon enterprise init` | Initialize enterprise policies |
| `falcon enterprise check` | Check policies |
| `falcon enterprise compliance` | Generate compliance report |
| `falcon marketplace [query]` | Browse rule packs and integrations |
| `falcon certify [path]` | Evaluate for Falcon certification |
| `falcon partners` | View partner integrations |

## Infrastructure

| Command | Description |
|---|---|
| `falcon mcp` | Start MCP server (stdio) |
| `falcon api --port 8090` | Start HTTP API server |
| `falcon init` | Generate default falcon.yaml |
| `falcon explain <rule>` | Explain a rule with examples |
| `falcon validate` | Validate falcon.yaml config |
| `falcon benchmark` | Run performance benchmark |
| `falcon compare` | Compare with dart analyze |
| `falcon self-tune` | Auto-tune rules from usage patterns |
| `falcon score-track` | Track AI score over time |
| `falcon learn [path]` | Record to cross-project learning database |
