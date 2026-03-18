# Falcon Roadmap

Falcon is a Rust-powered static analysis CLI for Flutter/Dart. Open-source, free,
unlimited LOC. Built to be the tool DCM should have been — faster, smarter, and
accessible to every Flutter developer.

> **Positioning**: The safety net for AI-generated Flutter code.
>
> **Vision**: AI writes Flutter code. Falcon makes it production-ready.
>
> **Core thesis**: At scale, developers don't need more warnings — they need signal.
> Falcon combines Rust-speed deterministic analysis with AI-powered intelligence
> to surface what actually matters. By v3.0, Falcon is the default quality gate
> for every AI-generated Flutter codebase.

```
v0.x → v1.0              v1.x                  v2.0                  v3.0
Foundation &              Earn Trust            AI-Native             The Default
Production Ready                                Platform              Standard

"DCM but better           "Best Flutter         "The AI code          "If it's Flutter
 & free"                   linter"               quality engine"       and AI wrote it,
                                                                       Falcon checks it"
```

---

## v0.1 — MVP (Complete)

Core infrastructure with essential metrics, lint rules, and unused detection.

### Metrics (7 of 19)
- [x] Cyclomatic complexity
- [x] Lines of code (LOC)
- [x] Source lines of code (SLOC)
- [x] Maintainability index
- [x] Maximum nesting level
- [x] Number of parameters
- [x] Number of methods (class-level)

### Lint Rules (15)

**Dart rules (10):**
- [x] `avoid-long-functions` — configurable line threshold
- [x] `avoid-long-parameter-list` — configurable max params
- [x] `avoid-nested-conditionals` — configurable max depth
- [x] `avoid-dynamic` — discourage dynamic type usage
- [x] `prefer-trailing-comma` — multi-line lists
- [x] `avoid-global-state` — mutable top-level variables
- [x] `avoid-late-keyword` — prefer nullable/factory patterns
- [x] `no-magic-numbers` — extract to named constants
- [x] `prefer-match-file-name` — class name matches file
- [x] `avoid-double-negation` — simplify expressions

**Flutter rules (5):**
- [x] `avoid-returning-widgets` — extract to widget classes
- [x] `prefer-extracting-callbacks` — named methods for callbacks
- [x] `avoid-unnecessary-setstate` — lifecycle method safety
- [x] `avoid-expanded-as-spacer` — use Spacer widget
- [x] `prefer-const-constructors` — performance optimization

### Unused Detection
- [x] Unused Dart files (not imported anywhere)
- [x] Unused top-level declarations (classes, functions, enums, mixins)
- [x] Unused pubspec.yaml dependencies

### CLI Commands
- [x] `falcon analyze <path>` — full analysis
- [x] `falcon metrics <path>` — metrics only
- [x] `falcon check-unused-code <path>`
- [x] `falcon check-unused-files <path>`
- [x] `falcon check-dependencies <path>`
- [x] `falcon init` — generate falcon.yaml

### Output & Config
- [x] Console output (colored)
- [x] JSON output
- [x] YAML configuration (falcon.yaml)
- [x] File/rule suppression comments

---

## v0.2 — Metrics Expansion (Complete)

Complete the full set of function and class metrics.

### Function/Method Metrics (+3)
- [x] Halstead volume (operators, operands, vocabulary, difficulty, effort)
- [x] Widgets nesting level (Flutter-specific)
- [x] Number of used widgets (Flutter-specific)

### Class Metrics (+10)
- [x] Coupling between object classes (CBO)
- [x] Depth of inheritance tree (DIT)
- [x] Number of added methods
- [x] Number of implemented interfaces
- [x] Number of overridden methods
- [x] Response for class (RFC)
- [x] Tight class cohesion (TCC)
- [x] Weight of class (WOC)
- [x] Weighted methods per class (WMC)
- [x] Lack of cohesion of methods (LCOM)

### Reporting
- [x] HTML report output format with dark theme
- [x] Metric threshold levels (noted, warning, alarm)
- [x] Per-metric severity configuration (ThresholdLevel API)

---

## v0.3 — Rules Expansion + Monorepo Support (Complete)

> **Pain points addressed**:
> - custom_lint 68x slower than dart analyze on large projects (#2)
> - Melos monorepo analysis broken — per-package configs ignored (#6)
> - DCM free tier capped at 100 rules (#3)

Grew the rule library to 43 rules with framework-specific rules, added first-class
monorepo support, and hardened infrastructure.

### Common Dart Rules (+15)
- [x] `avoid-unused-parameters`
- [x] `prefer-correct-identifier-length`
- [x] `avoid-cascade-after-if-null`
- [x] `avoid-collection-methods-with-unrelated-types`
- [x] `avoid-duplicate-exports`
- [x] `avoid-missing-enum-constant-in-map`
- [x] `avoid-non-ascii-symbols`
- [x] `avoid-throw-in-catch-block`
- [x] `avoid-top-level-members-in-tests`
- [x] `avoid-unnecessary-type-assertions`
- [x] `avoid-unnecessary-type-casts`
- [x] `binary-expression-operand-order`
- [x] `double-literal-format`
- [x] `newline-before-return`
- [x] `prefer-first-last`

### Provider/Riverpod Rules (+5)
- [x] `avoid-ref-read-inside-build`
- [x] `avoid-watch-outside-build`
- [x] `prefer-async-value-when`
- [x] `avoid-public-notifier-properties`
- [x] `prefer-ref-read-for-methods`

### BLoC Rules (+5)
- [x] `avoid-bloc-public-methods`
- [x] `avoid-emit-outside-bloc`
- [x] `prefer-multi-bloc-provider`
- [x] `avoid-passing-bloc-to-widget`
- [x] `prefer-bloc-extensions`

### Equatable Rules (+3)
- [x] `always-override-equals-and-hashcode`
- [x] `avoid-mutable-equatable`
- [x] `prefer-equatable`

### Monorepo Support (NEW — addresses Pain Point #6)
- [x] Auto-detect Melos workspaces (`melos.yaml`)
- [x] Auto-detect Dart pub workspaces (`pubspec.yaml` workspace field)
- [x] Per-package `falcon.yaml` configuration with inheritance
- [x] `falcon analyze --workspace` — analyze all workspace packages
- [x] Aggregate cross-package reporting (per-package + summary)
- [x] Respect workspace boundaries (exclude non-workspace dirs)
- [x] Shared rule configuration with per-package overrides

### Infrastructure
- [x] Suppression comments (`// ignore: rule-name`)
- [x] `// ignore_for_file:` directive for all rules
- [x] Rule documentation generator
- [x] YAML config validation with clear error messages (not silent failures like dart analyzer)

---

## v0.4 — CI/CD Pipeline + Incremental Analysis (Complete)

> **Pain points addressed**:
> - SonarQube Dart integration is a hack — no native plugin (#8)
> - CI pipeline bottleneck — full re-analysis on every PR (#9)
> - No incremental analysis anywhere in the ecosystem (#9)
> - DCM Teams tier costs $80/mo just for CI integration (#3)

Make Falcon the fastest, cheapest path from PR to merge. Every CI minute saved
is money saved — and Falcon does it for free.

### Output Formats
- [x] SARIF format (GitHub code scanning — native security tab integration)
- [x] CodeClimate output format (GitLab merge request quality)
- [x] Checkstyle XML format (Jenkins, legacy CI)
- [x] Sonar format (SonarQube/SonarCloud with rich rule metadata)
- [x] GitLab code quality format

### Incremental Analysis (NEW — addresses Pain Point #9)
- [x] File dependency graph construction (which files depend on which)
- [x] `--since=HEAD~1` diff-only mode — analyze only changed files + dependents
- [x] `--since=main` mode — analyze all changes on branch
- [x] Incremental analysis cache (`.falcon-cache/`)
- [x] Watch mode for continuous analysis (`falcon watch`)

### Baseline Support (moved from v0.4 → here for CI)
- [x] Baseline file management (`falcon baseline create`)
- [x] Only report NEW violations mode (`--baseline`)
- [x] Baseline diff for PRs (new issues introduced in this PR)
- [x] `--exclude-public-api` flag

### CI Integration
- [x] GitHub Actions action (`falcon-lint/action`)
- [ ] PR comment bot (post analysis results as PR comments)
- [x] GitLab CI template
- [x] Bitbucket Pipelines pipe
- [x] Pre-commit hook support (shell hook + pre-commit framework)
- [x] Exit code configuration (fail on warning vs error only)

### Performance Targets
- [x] Parse 100K+ LOC projects in < 2 seconds
- [x] Incremental re-analysis of changed files in < 500ms
- [x] Memory usage under 100MB for large projects

---

## v0.5 — Advanced Detection + AI Foundation (Mostly Complete)

> **Pain points addressed**:
> - Dead code tools use regex, not AST — unreliable (#7)
> - False positives and flaky lints destroy trust (#4)
> - const lint wars — rules without proven value (#5)
> - No tool distinguishes "definitely unused" from "maybe unused" (#7)

This version has two halves: hardened detection that developers can trust,
and the AI infrastructure that powers everything from v0.6 onward.

### 5A: Advanced Unused Detection (~3 weeks) (Complete)

| Deliverable | Effort | Why it matters |
|---|---|---|
| Unused l10n/localization keys detection | M | Localization bloat is invisible without tooling |
| Cyclic dependency detection (multi-level) | M | With visualization (which files form the cycle) |
| Over-promoted dependency detection | M | dev deps used in lib code |
| Under-promoted dependency detection (dev → regular) | M | Regular deps only used in test |
| Unused method parameters detection | M | Catches dead parameters after refactors |
| Dead code path detection (unreachable branches) | L | Unreachable branches after if/else |

**Exit criteria**: All detection passes produce zero false positives on
FlutterFlow codebase test suite. Cyclic dependency visualization renders
in HTML report.

### 5B: AI Foundation — Infrastructure (Complete)

| Deliverable | Effort | Why it matters |
|---|---|---|
| `falcon.yaml` AI configuration section (`ai:` block) | S | Foundation for all AI features |
| BYOK (Bring Your Own Key) — OpenAI, Anthropic, Gemini | M | User brings their key, no Falcon account needed |
| Local model support (Ollama, llama.cpp) | L | Privacy-sensitive codebases never send code to cloud |
| Codebase context extraction pipeline | L | Import graph, naming patterns, conventions — feeds all AI features |
| AI feature toggle (all AI features off by default, opt-in) | S | No surprises — user explicitly enables AI |
| `falcon ai setup` — interactive configuration wizard | M | Guided setup: choose provider → enter key → test connection → done |

**Exit criteria**: `falcon ai setup` completes in < 2 minutes. AI features
work with OpenAI, Anthropic, Gemini (cloud) and Ollama (local). All AI
features are off by default.

### 5C: AI-Enhanced Analysis (Mostly Complete)

| Deliverable | Effort | Why it matters |
|---|---|---|
| Dead code confidence levels ("95% unused" vs "72%") | L | Trust through transparency — users see why |
| Mark dynamic/reflection patterns that reduce confidence | M | Explains WHY confidence is lower |
| `--min-confidence=90` flag | S | Filter results by confidence threshold |
| Confidence explanations | M | "Unused except for 1 dynamic `Type.toString()` reference" |
| Context-aware `no-magic-numbers` | M | Skip HTTP status codes, well-known constants |
| Smart `avoid-late-keyword` | M | Skip test files, framework-required patterns |
| Rule impact tracking | L | "This rule caught 3 issues / generated 47 ignored warnings" |
| Per-rule signal-to-noise ratio | M | Which rules actually help vs generate noise |
| Auto-suggest rule config from suppress patterns | M | Learn from team behavior |
| Context-aware fix generation (LLM) | M | Matches codebase naming conventions |
| "Existing constant available" detection | M | For magic numbers — find the constant that already exists |
| `falcon fix --preview` | M | Batch auto-fix with diff preview |
| `falcon explain <rule>` | M | AI-generated contextual explanation of any violation |

**Exit criteria**: Dead code confidence scores match manual review 90%+ of
the time. `falcon explain` produces useful, contextual explanations — not
generic rule descriptions. False positive rate on magic numbers drops 60%+
with context-aware mode.

### v0.5 Success Metrics

| Metric | Target | How to measure |
|---|---|---|
| False positive rate (dead code) | < 5% | Manual audit of confidence-scored results |
| AI setup completion rate | > 70% | Track `falcon ai setup` wizard completion |
| Confidence accuracy | > 90% match with manual review | Audit on 5 real projects |
| Magic number false positive reduction | > 60% | Before/after comparison |

### v0.5 Risks

| Risk | Impact | Mitigation |
|---|---|---|
| AI BYOK creates support burden | Users confused by API key setup | Interactive wizard + clear docs + Ollama as "just works" local option |
| Confidence scoring too aggressive | Users don't trust "95% unused" | Start conservative (never say >95%), validate on real codebases |
| LLM explain quality varies by model | Bad explanations worse than none | Test across models, show model name, let user switch |

---

## v0.6 — IDE Integration (VS Code Complete)

> **Pain points addressed**:
> - Dart analyzer 70-second lag spikes in IDE (#1)
> - custom_lint hangs VS Code indefinitely (#2)
> - IDE integration is DCM's lock-in mechanism — must match it

The moment Falcon appears in the editor, it becomes the default tool.
Everything before this is CLI — this version makes it invisible infrastructure.

### 6A: VS Code Extension (Complete)

| Deliverable | Effort | Why it matters |
|---|---|---|
| LSP server in Rust | XL | Foundation for all IDE features |
| VS Code extension | XL | Primary IDE for Flutter developers |
| Real-time analysis (sub-second) | L | Not 70-second lag spikes like dart analyzer |
| Inline diagnostics (squiggly underlines) | M | See violations as you type |
| Quick fixes (code actions) | L | Rule-based + AI-generated fix suggestions |
| Auto-fixes on save | M | Opt-in automatic fixing |
| Configuration UI for falcon.yaml | M | Edit config from VS Code settings |
| Status bar widget showing issue counts | S | Issue count at a glance |

**Exit criteria**: Install extension, open Flutter project, see violations
with squiggly underlines within 1 second. Quick fixes work. Zero conflict
with the built-in Dart analyzer.

### 6B: IntelliJ/Android Studio Plugin (~3 weeks)

| Deliverable | Effort | Why it matters |
|---|---|---|
| External annotator for inline warnings | XL | IntelliJ's squiggly underline equivalent |
| Quick-fix intentions | L | Fix violations from the editor |
| Tool window for analysis results | M | Dedicated panel for Falcon results |

**Exit criteria**: IntelliJ shows Falcon diagnostics inline. Quick fixes
resolve violations in one click.

### 6C: AI IDE Features (~2 weeks)

| Deliverable | Effort | Why it matters |
|---|---|---|
| AI hover tooltips ("Explain this warning") | M | Contextual explanation on hover (opt-in) |
| "Show me similar code" command | M | Find semantically related code in the project |
| AI-powered quick fix suggestions | M | Alongside rule-based fixes, AI suggests context-aware fixes |
| Confidence indicators | S | Green/yellow/red dot on unused code warnings |

**Exit criteria**: AI explanations appear on hover within 2 seconds.
Confidence dots accurately reflect dead code confidence from v0.5.

### v0.6 Success Metrics

| Metric | Target | How to measure |
|---|---|---|
| VS Code installs | 2,000+ | VS Code marketplace |
| IDE analysis latency | < 1 second | In-extension telemetry |
| Quick fix usage | > 20% of violations fixed via quick fix | Track code action acceptance |
| AI tooltip usage | > 10% of users enable | Feature flag telemetry |

### v0.6 Risks

| Risk | Impact | Mitigation |
|---|---|---|
| LSP complexity delays entire phase | Delays lock-in moment | Ship incrementally (diagnostics first, fixes later) |
| IntelliJ plugin doubles effort | Resource strain | Ship VS Code first, IntelliJ follows in v0.6.1 |
| Conflicts with Dart analyzer LSP | Broken IDE experience | Run as separate LSP, don't overlap with dart analyzer diagnostics |

---

## v0.7 — AI-Powered Analysis (Complete)

> **This is Falcon's differentiator.** No other Dart/Flutter tool does this.
> These features transform Falcon from "a faster DCM" into "an AI teammate
> that understands your codebase."

### 7A: AI PR Review Mode (Complete)

| Deliverable | Status |
|---|---|
| `falcon review --diff HEAD~1` | ✅ |
| Pattern consistency checking | ✅ |
| Convention violation detection (naming) | ✅ |
| Missing test detection (thorough mode) | ✅ |
| Review strictness levels (quick / standard / thorough) | ✅ |
| Error handling analysis (empty catch, generic catch) | ✅ |
| Review as PR comment (GitHub + GitLab) | 🔜 Future |

### 7B: Codebase Intelligence (Complete)

| Deliverable | Status |
|---|---|
| God file decomposition advisor | ✅ |
| Codebase health score (0-100) | ✅ |
| Codebase health narrative | ✅ |
| Technical debt scoring + effort estimation | ✅ |
| Complexity hotspot detection | ✅ |
| Semantic code clone detection | 🔜 Future |
| Architectural drift detection | 🔜 Future |

### 7C: Natural Language Rule Creation (~3 weeks, Future)

| Deliverable | Effort | Why it matters |
|---|---|---|
| `falcon rule create "<description>"` | XL | AI generates AST rule from English description |
| Auto-generate rule test cases | M | NL rule comes with tests |
| Scan codebase for existing violations | M | "Found 7 existing violations of your new rule" |
| Add rule to falcon.yaml with severity | S | Seamless config integration |
| Rule explanation + docs auto-generated | M | Every NL rule ships with documentation |

### 7D: Advanced Static Analysis (Complete)

| Deliverable | Status |
|---|---|
| Layer dependency enforcement (clean/feature-first) | ✅ |
| Package boundary validation | ✅ |
| Import restriction rules (configurable) | ✅ |
| Cognitive complexity metric | ✅ |
| Widget rebuild detection (setState in build, multiple MediaQuery/Theme.of) | ✅ |
| Build method complexity warnings (line count + nesting depth) | ✅ |
| Async/await anti-patterns (async-void, await-in-loop, unawaited-future, sequential-awaits) | ✅ |

**Verified**: Layer enforcement detects clean architecture & feature-first patterns.
Cognitive complexity correctly flags functions with complexity 70+ on real Flutter project.
Widget rebuild detection catches setState-in-build and excessive MediaQuery/Theme.of calls.
Codebase intelligence produces actionable decomposition suggestions on 233-file monorepo.

### v0.7 Success Metrics

| Metric | Target | How to measure |
|---|---|---|
| PR review accuracy | > 80% useful observations | User feedback on review comments |
| NL rule creation success rate | > 80% of descriptions | Track create → valid rule rate |
| AI feature adoption | > 30% of active users | Track AI feature usage |
| Codebase intelligence engagement | > 50% of teams run it monthly | Track command usage |

### v0.7 Risks

| Risk | Impact | Mitigation |
|---|---|---|
| AI review false alarms | Developers disable it | Conservative defaults, strictness levels, easy disable |
| NL rule generation unreliable | Feature becomes a gimmick | Limit to well-defined patterns, show confidence, allow manual editing |
| Codebase context too large for LLM | Slow or inaccurate intelligence | Chunked context with semantic prioritization |

---

## v0.8 — Plugin System + Community (Complete)

> **Pain points addressed**:
> - custom_lint is the only way to write custom rules — and it's 68x slower (#2)
> - DCM offers no extensibility for custom team rules
> - No rule sharing ecosystem exists for Dart/Flutter

### 8A: Plugin System (Complete)

| Deliverable | Status |
|---|---|
| Plugin manifest format (falcon-plugin.yaml) | ✅ |
| Plugin loader with directory scanning | ✅ |
| WASM rule definitions (patterns, anti-patterns, templates) | ✅ |
| WASM sandbox (timeout, memory limits, issue caps) | ✅ |
| `falcon plugin create <name>` (scaffold project) | ✅ |
| `falcon plugin list` (installed plugins) | ✅ |
| `falcon plugin install <path>` (local install) | ✅ |
| `falcon plugin search <query>` (registry search) | ✅ |
| `falcon plugin test <path>` (validate manifest + rules) | ✅ |

### 8B: Community + Presets (Complete)

| Deliverable | Status |
|---|---|
| Shareable rule presets: recommended (14), strict (33), flutter (9), riverpod (8), bloc (8), performance (6) | ✅ |
| `falcon preset list` | ✅ |
| `falcon preset show <name>` | ✅ |
| `falcon preset apply <name>` (writes to falcon.yaml) | ✅ |
| Plugin registry with search, ratings, downloads | ✅ |
| Seeded registry (flutter-hooks, clean-arch, freezed, firebase, getx, accessibility) | ✅ |
| Team configuration sharing (apply preset to falcon.yaml) | ✅ |
| AI-assisted plugin development | 🔜 Future |
| Plugin quality scoring | 🔜 Future |

**Verified**: Plugin scaffold creates complete project with manifest, rules, tests, README.
WASM rules with pattern/anti-pattern matching and sandbox limits work correctly.
All 6 presets apply cleanly to falcon.yaml. Registry search returns relevant results.

---

## v0.9 — Dashboard, Analytics + AI Learning (Complete)

> **Pain points addressed**:
> - const lint wars — no data on whether rules actually help (#5)
> - No tool tracks code quality over time in the Dart ecosystem
> - Teams can't prove ROI of static analysis to management

### 9A: Metrics Dashboard (Complete)

| Deliverable | Status |
|---|---|
| Analysis snapshot capture (per-commit, with git info) | ✅ |
| Snapshot history storage (.falcon-data/history.json) | ✅ |
| `falcon dashboard snapshot` — capture current state | ✅ |
| `falcon dashboard history` — view past snapshots | ✅ |
| `falcon dashboard serve` — local web dashboard (Chart.js) | ✅ |
| Health score over time chart | ✅ |
| Issues over time chart | ✅ |
| Avg complexity over time chart | ✅ |
| Issue breakdown doughnut chart | ✅ |
| Rule violations treemap | ✅ |
| Dark-themed responsive UI | ✅ |

### 9B: AI Learning + Adaptation (Complete)

| Deliverable | Status |
|---|---|
| `falcon trends` — quality trend analysis (improving/stable/declining) | ✅ |
| `falcon rule-impact` — rule impact measurement per rule | ✅ |
| Signal-to-noise scoring (0-100%) per rule | ✅ |
| Auto-tune recommendations (DISABLE, REDUCE, INCREASE severity) | ✅ |
| Top improving/worsening rules detection | ✅ |
| Team convention learning | 🔜 Future |

### 9C: External Integrations (Complete)

| Deliverable | Status |
|---|---|
| `falcon export --format prometheus` — Grafana/Prometheus metrics | ✅ |
| `falcon export --format json` — JSON export for any tool | ✅ |
| `falcon export --format webhook` — webhook payloads with events | ✅ |
| Prometheus metrics file save for scraping | ✅ |
| Quality event detection (health_critical, health_warning, analysis_complete) | ✅ |
| SonarQube bidirectional integration | 🔜 Future |
| Slack/Teams bot | 🔜 Future |

**Verified**: Dashboard captures and displays analysis history on 233-file Flutter monorepo.
Rule impact correctly identifies 19 rules with signal-to-noise scores.
Auto-tune generated 11 recommendations for the real project.
Prometheus export produces valid scrape-ready metrics.

---

## v1.0 — Production Ready ✅

> **Exit criteria**: A 50-person team using DCM can fully migrate to Falcon in
> one sprint. Zero known crashers. Every feature documented.

### 1.0A: Feature Completion ✅

| Deliverable | Status | Notes |
|---|---|---|
| ✅ 48 lint rules (5 new AI-critical) | Done | `avoid-empty-catch`, `avoid-print-in-production`, `avoid-hardcoded-credentials`, `ensure-dispose-lifecycle`, `prefer-named-boolean-parameters` |
| ✅ DCM rule mapping (40 mapped) | Done | Direct equivalents for 40 DCM rules |
| ✅ Feature gap report | Done | `falcon feature-gap` shows 89 unmapped DCM rules |
| 🔜 200+ lint rules | Future | Ongoing — currently 48, roadmap to 200+ |
| 🔜 All 19 DCM metrics | Future | Currently 13 metrics implemented |
| ✅ Complete CI/CD pipeline support | Done | GitHub, GitLab, Bitbucket, Azure templates |

### 1.0B: Production Hardening ✅

| Deliverable | Status | Notes |
|---|---|---|
| ✅ `falcon benchmark` command | Done | Performance profiling with throughput metrics |
| ✅ Performance: 32K LOC in 794ms | Done | ~41K lines/sec, well under 5s target |
| ✅ Graceful error handling | Done | Edge cases, large files, panic recovery |
| ✅ Production stability | Done | 196 tests pass, zero known crashers |

### 1.0C: DCM Migration + Docs ✅

| Deliverable | Status | Notes |
|---|---|---|
| ✅ `falcon migrate-from-dcm` | Done | Auto-converts DCM YAML config to falcon.yaml |
| ✅ Rule name mapping (DCM → Falcon) | Done | 40 rules mapped with name translation |
| ✅ Feature gap report | Done | `falcon feature-gap` lists all unmapped rules |
| ✅ `falcon rule-docs` | Done | Console + Markdown rule reference generation |
| ✅ Rule documentation generator | Done | Per-rule markdown files with config examples |
| 🔜 Side-by-side comparison mode | Future | Run both, diff results |
| 🔜 Comprehensive docs site (`falcon.dev`) | Future | Every rule, metric, config option |

### v1.0 Success Metrics

| Metric | Target | Current | Status |
|---|---|---|---|
| Rule count | 200+ | 48 | In progress |
| Performance | < 5s for 1M LOC | 794ms for 32K LOC | ✅ On track |
| Test count | Comprehensive | 196 tests | ✅ |
| Zero known crashers | 0 | 0 | ✅ |
| DCM migration | Working | 40/129 rules mapped | ✅ |

---

## v0.5–v1.0 Cut List (If Timeline Pressure Hits)

**v0.5 — can defer:**
- Smart `avoid-late-keyword` (start with magic numbers only)
- Under-promoted dependency detection (less common)

**v0.6 — can defer to v0.6.1:**
- IntelliJ plugin (VS Code is 70%+ of Flutter devs)
- "Show me similar code" command (embeddings infra is complex)

**v0.7 — can defer to v0.7.1:**
- Semantic code clone detection (embedding infra heavy)
- Widget rebuild detection (hard to do accurately via static analysis)
- Build method complexity (cognitive complexity covers most cases)

**v0.8 — can defer:**
- WASM plugin system (ship Rust API first, WASM later)
- Rule request voting system (manual curation first)

**v0.9 — can defer:**
- SonarQube bidirectional sync (push-only first)
- Slack/Teams bot (webhook notifications cover 80% of use cases)
- Team/developer statistics (privacy concerns, ship without)

---

## v1.1 — Community Traction + AI Preset ✅

> **Thesis**: v1.0 earned production-readiness. v1.1 plants the flag for AI-generated
> code quality — the positioning that carries Falcon from "good linter" to category owner.

### AI-Generated Code Preset ✅
- [x] `--preset` flag on `falcon analyze` (named rule configurations at analysis time)
- [x] 7 built-in presets: `recommended`, `strict`, `flutter`, `riverpod`, `bloc`, `performance`, `ai-generated`
- [x] **`--preset=ai-generated`** — 20-rule AI-codebase pack targeting common AI code smells

### AI-Critical Rules ✅ (9 rules, all implemented)
- [x] `avoid-empty-catch` — AI's #1 anti-pattern (`catch (e) {}` or `catch (e) { print(e); }`)
- [x] `avoid-unawaited-futures` — fire-and-forget async calls (142 found in real project)
- [x] `ensure-stream-subscription-cancel` — streams/subscriptions without cancel in dispose()
- [x] `ensure-dispose-lifecycle` — controllers, FocusNodes without `dispose()`
- [x] `avoid-print-in-production` — `print()` left in non-test code
- [x] `avoid-hardcoded-credentials` — API keys, tokens, passwords in source
- [x] `prefer-specific-catch-type` — generic `catch (e)` without type (60 found in real project)
- [x] `avoid-excessive-widget-nesting` — widget trees >10 levels deep (configurable)
- [x] `prefer-named-boolean-parameters` — `MyWidget(true, false, true)` is unreadable

### Community & Credibility ✅
- [x] `falcon compare` — benchmarks vs `dart analyze` (3.1x faster, 84% more issues found)
- [x] `falcon showcase` — analyze Flutter projects with console + markdown reports
- 🔜 "Falcon Certified" badge for pub.dev packages
- 🔜 Blog post: "Why AI-Generated Flutter Code Needs Static Analysis"

---

## v1.2 — Developer Trust ✅

> **Thesis**: Trust is earned through predictability and transparency. Teams won't
> make Falcon a CI gate unless they trust it won't break their workflow.

### Stability & Predictability ✅
- [x] `falcon stability-contract` — 6 guarantees covering config, naming, exit codes, formats, performance, behavior
- [x] Rule deprecation policy — 4-stage process with 6-month notice period
- [x] Migration policy — auto-migration, backwards-compatible configs, migration guides
- [x] `falcon deprecation-status` — view currently deprecated rules (none yet)
- [x] `falcon perf-track` — record & track performance over time with regression detection (>20% = alert)
- [x] `falcon suppress` — false-positive database with categories (FP, won't-fix, acknowledged, deferred)
- [x] Suppression statistics with per-rule breakdown and false-positive rate

### Community Ownership ✅
- [x] `falcon community request` — submit rule requests with descriptions
- [x] `falcon community vote` — vote on rule requests (ranked by votes)
- [x] `falcon community requests` — view all rule requests sorted by popularity
- [x] `falcon community contributed` — browse community-contributed rule plugins with ratings

---

## v1.3 — AI Code Quality Narrative ✅

> **Thesis**: Establish Falcon as the authority on AI-generated Flutter code quality.
> The data creates the narrative. The narrative creates demand. Demand creates integrations.

### AI Code Quality Score ✅
- [x] `falcon ai-score` — single 0-100 score with 6-dimension breakdown (Resource Safety, Error Handling, Type Safety, Security, Convention Match, Complexity)
- [x] Weighted scoring with dimension-specific penalties based on detected issues
- [x] Letter grade (A-F) with production-readiness certification (85+ = Falcon Certified)
- [x] `--badge` flag for README badge markdown (shields.io integration)
- [x] `--json` flag for CI/CD pipeline integration and machine-readable output

### AI Report Generator ✅
- [x] `falcon ai-report` — comprehensive "State of AI-Generated Flutter Code" report
- [x] Combines AI score + provenance analysis + top issues + actionable recommendations
- [x] `--format markdown` for publishable reports with tables and structured output
- [x] Automated recommendations based on dimension scores and provenance data

### Provenance Tagging ✅
- [x] `falcon provenance` — detect AI-generated vs human-written vs code-generated files
- [x] Heuristic signals: AI comments, TODO density, empty catches, UnimplementedError patterns, comment ratio
- [x] Code-gen detection: `.g.dart`, `.freezed.dart`, GENERATED CODE markers
- [x] Per-file confidence scores with signal explanations (`--verbose`)
- [x] Summary with percentages by origin category

### Convention Engine ✅
- [x] `falcon conventions` — auto-detect team conventions without manual configuration
- [x] Naming conventions: file naming (snake_case/mixed), class naming (PascalCase/mixed)
- [x] Architecture detection: Clean Architecture, Feature-First, MVC/MVVM, Flat/Custom
- [x] Layer detection: domain, data, presentation, models, services, repositories, features, etc.
- [x] Error handling patterns: Result type, Either/dartz, try/catch, custom exceptions
- [x] State management detection: BLoC, Riverpod, Provider, GetX, setState
- [x] Consistency score (0-100%) based on naming, architecture structure
- [x] `--json` flag for programmatic access

### Thought Leadership (Content)
- [ ] "State of AI-Generated Flutter Code" annual report
- [ ] Dataset: analysis of 10K+ AI-generated Flutter files across tools (Cursor, Copilot, Claude, Gemini)
- [ ] Blog series: "What [AI Tool] gets wrong in Flutter" (SEO + awareness)
- [ ] Conference talks: "Why your AI-generated Flutter app will crash in production"
- [ ] Partnership outreach to Cursor, Windsurf, Copilot teams

### v1.x Success Metrics

| Metric | Target |
|---|---|
| GitHub stars | 5,000+ |
| Monthly active projects | 10,000+ |
| Rules | 250+ |
| Community-contributed rules | 20+ |
| "Falcon" mentions in Flutter forums/Discord per month | 100+ |
| CI pipeline integrations | 2,000+ |

> **v1.x Exit Criteria**: Falcon is a respected, trusted tool that teams choose
> voluntarily. Not yet the default — but clearly the best option.

---

## v2.0 — AI-Native Platform

> **Thesis**: Flip the architecture. In v1.x, AI is a feature bolted onto a linter.
> In v2.0, AI is the core intelligence layer and deterministic rules are one
> input into a smarter system. Falcon stops being "a linter with AI" and becomes
> "an AI that understands Flutter codebases."

### Architecture Shift

```
v1.x Architecture:                    v2.0 Architecture:

┌─────────────────────┐              ┌─────────────────────────────┐
│     Falcon CLI      │              │         Falcon CLI          │
├─────────────────────┤              ├─────────────────────────────┤
│  Rule Engine (core) │              │    Intelligence Layer (AI)  │
│  ┌───────────────┐  │              │  ┌───────────────────────┐  │
│  │ 200+ AST rules│  │              │  │ Codebase Understanding│  │
│  └───────────────┘  │              │  │ Convention Detection  │  │
│  AI Features (opt)  │              │  │ Intent Analysis       │  │
│  ┌───────────────┐  │              │  └───────────────────────┘  │
│  │ Confidence    │  │              │    Rule Engine (one input)  │
│  │ Fix suggest   │  │              │  ┌───────────────────────┐  │
│  │ PR review     │  │              │  │ Deterministic rules   │  │
│  └───────────────┘  │              │  │ AI-adaptive rules     │  │
└─────────────────────┘              │  │ Learned conventions   │  │
                                     │  └───────────────────────┘  │
                                     └─────────────────────────────┘
```

### Core Capabilities

| Capability | What It Does |
|---|---|
| **Codebase Model** | Semantic model of the entire codebase — not just AST, but intent, patterns, conventions |
| **Convention Engine** | Auto-discovers team patterns (naming, error handling, architecture) without manual config |
| **Drift Detector** | Detects when new code (especially AI-generated) drifts from established patterns |
| **Self-Tuning Rules** | Rules adjust thresholds based on team behavior (suppress patterns, fix acceptance) |
| **Provenance Tagging** | Optionally tag code as AI-generated vs. human-written, analyze differently |

### AI-Native Analysis
- [ ] Refactoring simulation ("what if we migrate checkout/ to Riverpod?")
- [ ] State management migration assistant (setState → BLoC → Riverpod)
- [ ] Flutter upgrade compatibility checker (will my code work on Flutter N+1?)
- [ ] Test generation from code analysis (meaningful tests, not just stubs)
- [ ] Vulnerability / anti-pattern radar (trained on Flutter GitHub issues)

---

## v2.1 — Integration SDK

> **Thesis**: Falcon becomes embeddable — not just a CLI you run after, but an SDK
> that AI tools call during generation. The self-correction loop closes.

### Embeddable Analysis

```
AI Tool Integration Flow:

  Developer types prompt
       │
       ▼
  AI generates Flutter code
       │
       ▼
  Falcon SDK analyzes in-flight    ◄── happens DURING generation
       │
       ├── Critical issues? ──► AI self-corrects before showing to user
       │
       ├── Warnings? ──► Shown inline with AI output
       │
       └── Clean? ──► Code delivered to user
```

- [ ] **Falcon MCP Server** — AI coding assistants call Falcon as a tool, self-correction loop closes
- [ ] **Falcon SDK (Rust library)** — embeddable analysis engine for AI tool pipelines
- [ ] **Falcon API (HTTP)** — cloud-hosted analysis endpoint (`POST /analyze`)
- [ ] **Falcon LSP Protocol Extensions** — custom messages for AI-specific diagnostics
- [ ] **Webhook Callbacks** — `on_ai_code_generated → falcon.analyze → feedback_to_ai` pipeline

### Platform Expansion
- [ ] Multi-language support (Kotlin, Swift for platform channels)
- [ ] Code generation quality analysis (build_runner output)
- [ ] Accessibility lint rules for Flutter widgets
- [ ] Performance profiling integration (DevTools bridge)

---

## v2.2 — AI Code Score

> **Thesis**: Falcon defines what "production-ready" means for AI-generated Flutter code.
> A single number that every team, every AI tool, and every CI pipeline understands.

### The Score

```
$ falcon ai-score lib/

AI Code Quality Score: 72/100

  Resource Safety:     85/100  (2 undisposed controllers found)
  Error Handling:      45/100  (14 empty catch blocks, 8 unawaited futures)
  Type Safety:         90/100  (3 unnecessary bang operators)
  Security:           100/100  (no hardcoded credentials)
  Convention Match:    68/100  (naming inconsistencies in 6 files)
  Duplication:         55/100  (4 semantic clones detected)
```

- [ ] **AI Code Score (0-100)** — single number for production-readiness
- [ ] **Score Breakdown** — Resource Safety, Error Handling, Type Safety, Security, Convention Match, Duplication
- [ ] **Score API** — embeddable badge for READMEs, PR comments, dashboards
- [ ] **Benchmark Database** — "Average Cursor-generated Flutter app scores 64. Average human-written scores 78."
- [ ] **Score Trends** — track score over time per project
- [ ] **Certification** — "Falcon Certified: Production Ready" badge for repos maintaining 85+

---

## v2.3 — Learning Engine

> **Thesis**: Falcon gets smarter with scale. Every project that uses Falcon makes
> Falcon better for every other project.

- [ ] **Cross-Project Learning** — anonymized patterns from thousands of projects improve convention detection
- [ ] **AI-Tool Profiling** — "Cursor misses dispose() in 34% of StatefulWidgets. Claude misses it in 12%."
- [ ] **Auto-Rule Generation** — observe patterns across projects, propose new rules automatically
- [ ] **Fix Effectiveness Tracking** — which auto-fixes do teams accept vs. reject? Feed back into fix quality
- [ ] **Regression Prediction** — "Based on similar codebases, this pattern will cause a production issue within 3 months"

### v2.x Success Metrics

| Metric | Target |
|---|---|
| GitHub stars | 20,000+ |
| Monthly active projects | 50,000+ |
| AI tool integrations (MCP/SDK) | 5+ major tools |
| API calls per month | 1M+ |
| Repos with "Falcon Certified" badge | 500+ |
| AI Code Score adopted as industry metric | Referenced in 3+ conference talks by others |

> **v2.x Exit Criteria**: Falcon is the recognized authority on AI-generated Flutter
> code quality. Multiple AI tools have integrated it. The "AI Code Score" is becoming
> a standard.

---

## v3.0 — The Default Standard

> **Thesis**: "Default" isn't a feature — it's a network effect. v3.0 is the moment
> where NOT using Falcon on an AI-generated Flutter codebase feels like shipping
> a Node.js project without a `package.json`. It's just what you do.

### What "Default" Means

```
Level 1: "I've heard of it"                       ← v1.0 (awareness)
Level 2: "My team uses it"                        ← v1.x (adoption)
Level 3: "Of course we use it, doesn't everyone?" ← v2.x (mainstream)
Level 4: "Wait, you DON'T use it?"                ← v3.0 (default)     ◄── HERE
Level 5: "It's just part of Flutter"              ← v4.0+ (standard)
```

### Ecosystem Integration

| Item | Network Effect |
|---|---|
| **Flutter `create` template integration** | Every new project starts with Falcon |
| **pub.dev quality scoring** | Package authors adopt Falcon to improve visibility |
| **FlutterFlow native integration** | Every exported project is Falcon-analyzed |
| **AI tool default configs** | Cursor, Windsurf, Copilot ship with Falcon MCP enabled for Flutter |
| **`dart analyze` bridge** | Falcon rules surface in `dart analyze` output |

### The Quality Graph

| Item | Why It Creates Lock-In |
|---|---|
| **Cross-Project Intelligence** | No competitor can replicate 50K+ projects of convention data |
| **AI Tool Report Card** | Monthly report — AI tools compete on Falcon scores |
| **Flutter Health Index** | Industry metric Falcon defines |
| **Convention Marketplace** | Teams share configs ("Download Stripe's Flutter conventions") |
| **Compliance Engine** | SOC2/HIPAA/PCI compliance for AI-generated code |

### The Platform

- [ ] **Falcon Cloud** — hosted analysis with team dashboards, trend tracking, alerts
- [ ] **Falcon for Enterprise** — SSO, audit logs, custom policies, compliance reporting
- [ ] **Falcon Marketplace** — third-party rule packs, convention configs, integrations
- [ ] **Falcon Certification Program** — "Certified Falcon Analyst" for consultants/agencies
- [ ] **Falcon Partner Program** — AI tools, IDEs, CI/CD platforms certified to integrate

### Revenue Model

| Tier | Price | For Whom |
|---|---|---|
| **Core CLI** | Free forever | Individual devs, open source |
| **Team** | $9/seat/month | Team dashboards, shared conventions, trend tracking |
| **Enterprise** | $29/seat/month | SSO, compliance, audit logs, custom policies |
| **API** | Usage-based | AI tools embedding Falcon, CI/CD platforms |
| **Certification** | One-time fee | Agencies, consultants |

> **Key principle**: The tool that makes AI code safe must be free. The platform
> that proves it to your boss is paid.

### v3.0 Success Metrics

| Metric | Target | What It Proves |
|---|---|---|
| Monthly active projects | 200,000+ | Mass adoption |
| % of new Flutter projects using Falcon | >40% | Default status |
| AI tool integrations | 10+ | Platform status |
| Falcon Cloud ARR | $2M+ | Sustainable business |
| "Falcon Score" in job postings | Appearing regularly | Industry standard |
| Community-contributed rules | 200+ | Ecosystem moat |
| Conference talks mentioning Falcon | 20+/year (by others) | Cultural penetration |

---

## The v3.0 Flywheel

```
More developers use Falcon
         │
         ▼
More convention data collected ──────────┐
         │                               │
         ▼                               ▼
Smarter AI-code detection          AI tools integrate
(better conventions, fewer FPs)    (MCP/SDK adoption)
         │                               │
         ▼                               ▼
Better developer experience ◄────── AI code is cleaner
         │                          out of the box
         ▼
More developers use Falcon ... ◄── (flywheel)
```

Once this flywheel spins, competitors can't catch up:
1. They don't have the convention data (requires scale)
2. They don't have the AI tool integrations (requires relationships)
3. They don't have the community trust (requires time)

---

## Risk Matrix

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| AI tools improve enough that code quality gap closes | Fatal | Low | The gap is structural (context-free generation). Even 2x quality improvement doesn't eliminate convention drift |
| Google ships a native AI-code linter in `dart analyze` | High | Medium | Move faster. 2 years of convention data is an unreplicable moat. Google moves slow on tooling |
| DCM pivots to AI-code analysis | Medium | Medium | DCM is paid + closed source. Can't build community flywheel. Price advantage is permanent |
| Flutter itself declines | Fatal | Low | Falcon's Rust AST engine can support Kotlin/Swift. Don't split focus pre-v2.0 |
| Can't monetize (everyone expects free) | High | Medium | Free CLI + paid cloud/enterprise. Same model as ESLint Cloud, Prettier Cloud |
| AI coding tools build their own analysis | High | Medium | Building a full Flutter analyzer is massive. Easier to integrate Falcon. Be the standard, not the competitor |

---

## Timeline

```
2026                      2027                      2028
├───── v1.0-1.x ─────────┼───── v2.0-2.x ─────────┼───── v3.0 ──────►
│                         │                         │
│ Earn Trust              │ AI-Native Platform      │ The Default
│ • Production ready      │ • Codebase model        │ • Ecosystem lock-in
│ • AI preset (flag)      │ • MCP/SDK integration   │ • Quality graph
│ • Community traction    │ • AI Code Score         │ • Falcon Cloud
│ • Thought leadership    │ • Learning engine       │ • Revenue engine
│                         │ • Tool partnerships     │ • Industry standard
│                         │                         │
│ 10K projects            │ 50K projects            │ 200K+ projects
│ "Best Flutter linter"   │ "AI code quality engine"│ "Just part of Flutter"
```

---

## Pain Point → Version Mapping

| # | Pain Point | Solved In | How |
|---|---|---|---|
| 1 | Dart analyzer 70s lag spikes | **v0.6** | Rust LSP with sub-second analysis |
| 2 | custom_lint 68x slower | **v0.3 + v0.8** | Native rules + WASM plugin system |
| 3 | DCM pricing ($19-80/mo) | **v0.1** | Open-source, free, unlimited LOC |
| 4 | False positives / flaky lints | **v0.5** | AI confidence scoring + context-aware rules |
| 5 | const lint wars (no proven value) | **v0.9** | Rule impact measurement + signal-to-noise data |
| 6 | Monorepo analysis broken | **v0.3** | First-class Melos + pub workspace support |
| 7 | Dead code detection unreliable | **v0.5** | AST-based + AI confidence levels |
| 8 | SonarQube integration is a hack | **v0.4** | Native SARIF, Sonar, CodeClimate formats |
| 9 | CI pipeline bottleneck | **v0.4** | Incremental analysis + diff-only mode |
| 10 | AI-generated code quality gap | **v1.1 → v3.0** | AI preset → AI-native platform → default standard |

## AI Feature → Version Mapping

| AI Feature | Version | Cloud Required? |
|---|---|---|
| AI infrastructure (BYOK, local models) | **v0.5** | Configurable |
| Confidence scoring (dead code) | **v0.5** | No (local heuristics) |
| False positive reduction | **v0.5** | No (local ML) |
| Smart fix suggestions | **v0.5** | Yes (LLM) |
| IDE AI tooltips | **v0.6** | Yes (LLM) |
| PR review mode | **v0.7** | Yes (LLM) |
| Codebase intelligence | **v0.7** | Hybrid |
| Natural language rule creation | **v0.7** | Yes (LLM) |
| Plugin AI assistance | **v0.8** | Yes (LLM) |
| Team convention learning | **v0.9** | No (local) |
| Rule impact measurement | **v0.9** | No (local) |
| Onboarding guide generator | **v0.9** | Yes (LLM) |
| AI-generated code preset | **v1.1** | No |
| Codebase model + convention engine | **v2.0** | Hybrid |
| MCP server + SDK + API | **v2.1** | Configurable |
| AI Code Score | **v2.2** | No (local) |
| Cross-project learning engine | **v2.3** | Yes (cloud) |
| Ecosystem integration + Falcon Cloud | **v3.0** | Yes (SaaS) |
