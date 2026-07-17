# Falcon CLI Migration

Falcon is moving to a compact top-level CLI before 1.0:

| Before | Now |
|---|---|
| `falcon analyze .` | `falcon check .` |
| `falcon smells .` | `falcon x smells .` |
| `falcon metrics .` | `falcon x metrics .` |
| `falcon ai-score .` | `falcon score .` |
| `falcon pr-comment . --base-ref origin/main` | `falcon review . --base-ref origin/main --format gh` |
| `falcon check-unused-code .` | `falcon x check-unused-code .` |
| `falcon check-unused-files .` | `falcon x check-unused-files .` |
| `falcon check-dependencies .` | `falcon x check-dependencies .` |
| `falcon check-cycles .` | `falcon x check-cycles .` |
| `falcon check-unused-params .` | `falcon x check-unused-params .` |
| `falcon check-dead-code .` | `falcon x check-dead-code .` |
| `falcon check-unused-l10n .` | `falcon x check-unused-l10n .` |
| `falcon check-promoted-deps .` | `falcon x check-promoted-deps .` |
| `falcon upgrade-check .` | `falcon x upgrade-check .` |
| `falcon check-platform .` | `falcon x check-platform .` |
| `falcon check-codegen .` | `falcon x check-codegen .` |
| `falcon check-perf .` | `falcon x check-perf .` |
| `falcon check-unused-confidence .` | `falcon x check-unused-confidence .` |
| `falcon check-layers .` | `falcon x check-layers .` |
| `falcon check-imports .` | `falcon x check-imports .` |
| `falcon cognitive-complexity .` | `falcon x cognitive-complexity .` |
| `falcon check-widgets .` | `falcon x check-widgets .` |
| `falcon check-async .` | `falcon x check-async .` |
| `falcon codebase-intel .` | `falcon x codebase-intel .` |
| `falcon ai-report .` | `falcon x ai-report .` |
| `falcon provenance .` | `falcon x provenance .` |
| `falcon ai-profile .` | `falcon x ai-profile .` |
| `falcon discover-rules .` | `falcon x discover-rules .` |
| `falcon predict .` | `falcon x predict .` |
| `falcon drift .` | `falcon x drift .` |
| `falcon conventions .` | `falcon x conventions .` |
| `falcon compare .` | `falcon x compare .` |
| `falcon history .` | `falcon x history .` |
| `falcon compare-reports . --run1 1 --run2 3` | `falcon x compare-reports . --run1 1 --run2 3` |
| `falcon compare-branches . --base main --branch feature` | `falcon x compare-branches . --base main --branch feature` |
| `falcon baseline create .` | `falcon x baseline create .` |
| `falcon validate .` | `falcon x validate .` |
| `falcon explain <rule>` | `falcon x explain <rule>` |
| `falcon preset list` | `falcon x preset list` |
| `falcon rule-docs --format markdown` | `falcon x rule-docs --format markdown` |
| `falcon stability-contract` | `falcon x stability-contract` |
| `falcon deprecation-status` | `falcon x deprecation-status` |
| `falcon suppress list .` | `falcon x suppress list .` |
| `falcon dashboard snapshot` | `falcon x dashboard snapshot` |
| `falcon dashboard history` | `falcon x dashboard history` |
| `falcon dashboard serve` | `falcon x dashboard serve` |
| `falcon trends .` | `falcon x trends .` |
| `falcon rule-impact .` | `falcon x rule-impact .` |
| `falcon benchmark .` | `falcon x benchmark .` |
| `falcon benchmark-db . --tool cursor` | `falcon x benchmark-db . --tool cursor` |
| `falcon score-track .` | `falcon x score-track .` |
| `falcon perf-track .` | `falcon x perf-track .` |
| `falcon fix-track . --report` | `falcon x fix-track . --report` |
| `falcon self-tune .` | `falcon x self-tune .` |
| `falcon learn .` | `falcon x learn .` |
| `falcon watch .` | `falcon x watch .` |
| `falcon run .` | `falcon x run .` |
| `falcon flutter doctor` | `falcon x flutter doctor` |
| `falcon fvm use stable` | `falcon x fvm use stable` |
| `falcon runtime-check .` | `falcon x runtime-check .` |
| `falcon live .` | `falcon x live .` |
| `falcon devtools memory .` | `falcon x devtools memory .` |
| `falcon manage health .` | `falcon x manage health .` |
| `falcon plugin list` | `falcon x plugin list` |
| `falcon export . --format json` | `falcon x export . --format json` |
| `falcon webhook . --url https://hooks.example.com` | `falcon x webhook . --url https://hooks.example.com` |
| `falcon migrate-from-dcm analysis_options.yaml` | `falcon x migrate-from-dcm analysis_options.yaml` |
| `falcon feature-gap` | `falcon x feature-gap` |
| `falcon showcase .` | `falcon x showcase .` |
| `falcon community contributed` | `falcon x community contributed` |
| `falcon mcp` | `falcon x mcp` |
| `falcon api --port 8090` | `falcon x api --port 8090` |
| `falcon cloud dashboard` | `falcon x cloud dashboard` |
| `falcon enterprise check .` | `falcon x enterprise check .` |
| `falcon marketplace` | `falcon x marketplace` |
| `falcon certify .` | `falcon x certify .` |
| `falcon partners` | `falcon x partners` |
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
