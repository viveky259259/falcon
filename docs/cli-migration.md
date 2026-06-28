# Falcon CLI Migration

Falcon is moving to a compact top-level CLI before 1.0:

| Before | Now |
|---|---|
| `falcon analyze .` | `falcon check .` |
| `falcon ai-score .` | `falcon score .` |
| `falcon pr-comment . --base-ref origin/main` | `falcon review . --base-ref origin/main --format gh` |
| `falcon asset-audit .` | `falcon x asset-audit .` |
| `falcon theme-audit .` | `falcon x theme-audit .` |
| `falcon l10n-coverage .` | `falcon x l10n-coverage .` |
| `falcon deeplink-validate .` | `falcon x deeplink-validate .` |
| `falcon animation-audit .` | `falcon x animation-audit .` |
| `falcon golden-gen .` | `falcon x golden-gen .` |
| `falcon dep-graph .` | `falcon x dep-graph .` |
| `falcon workspace .` | `falcon x workspace .` |
| `falcon docs docs/` | `falcon x docs docs/` |
| `falcon vuln-scan .` | `falcon x vuln-scan .` |
| `falcon refactor-sim . --scenario migrate-to-riverpod` | `falcon x refactor-sim . --scenario migrate-to-riverpod` |
| `falcon test-gen .` | `falcon x test-gen .` |

The default `falcon --help` now shows the stable surface:

```bash
falcon review
falcon check
falcon fix
falcon score
falcon x
```

For one release, the full historical command list remains available:

```bash
falcon --legacy-help
```

Legacy command aliases continue to run and print a one-line redirect before
they are removed in v1.0.
