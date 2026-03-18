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

## v0.6 — IDE Integration

> **Pain points addressed**:
> - Dart analyzer 70-second lag spikes in IDE (#1)
> - custom_lint hangs VS Code indefinitely (#2)
> - IDE integration is DCM's lock-in mechanism — must match it

The moment Falcon appears in the editor, it becomes the default tool.
Everything before this is CLI — this version makes it invisible infrastructure.

### 6A: VS Code Extension (~5 weeks)

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

## v0.7 — AI-Powered Analysis

> **This is Falcon's differentiator.** No other Dart/Flutter tool does this.
> These features transform Falcon from "a faster DCM" into "an AI teammate
> that understands your codebase."

### 7A: AI PR Review Mode (~4 weeks)

| Deliverable | Effort | Why it matters |
|---|---|---|
| `falcon review --diff HEAD~1` | L | AI-powered review of changes |
| Pattern consistency checking | L | "8 similar handlers use try/catch, yours doesn't" |
| Convention violation detection | L | Naming, architecture, error handling patterns |
| Missing test detection | M | "This function has 4 code paths, tests cover 1" |
| Review as PR comment (GitHub + GitLab) | M | AI review appears alongside human review |
| Review strictness levels (quick / standard / thorough) | S | Teams control depth vs speed |

**Exit criteria**: `falcon review` on a real PR produces 3-5 observations
that a senior engineer would agree with. False alarm rate < 20%.

### 7B: Codebase Intelligence (~3 weeks)

| Deliverable | Effort | Why it matters |
|---|---|---|
| God file decomposition advisor | L | "Here's how to split this 8K-line file into 5 modules" |
| Semantic code clone detection | L | Same logic, different syntax — embeddings-based |
| Architectural drift detection | L | "Repository accumulating UI logic" |
| Codebase health narrative | M | "Complexity rose 31% in checkout/ — here's why" |
| Technical debt scoring + effort estimation | M | Estimations teams can put in sprint planning |

**Exit criteria**: Decomposition advisor produces actionable suggestions
on FlutterFlow's `project.dart` (8,489 lines) and `extensions.dart` (80 extensions).
Health narrative correlates multiple metrics into coherent story.

### 7C: Natural Language Rule Creation (~3 weeks)

| Deliverable | Effort | Why it matters |
|---|---|---|
| `falcon rule create "<description>"` | XL | AI generates AST rule from English description |
| Auto-generate rule test cases | M | NL rule comes with tests |
| Scan codebase for existing violations | M | "Found 7 existing violations of your new rule" |
| Add rule to falcon.yaml with severity | S | Seamless config integration |
| Rule explanation + docs auto-generated | M | Every NL rule ships with documentation |

**Exit criteria**: NL rule creation works for 80%+ of common rule
descriptions. Generated rules have < 10% false positive rate.

### 7D: Advanced Static Analysis (~3 weeks)

| Deliverable | Effort | Why it matters |
|---|---|---|
| Layer dependency enforcement | L | Clean architecture validation |
| Package boundary validation | M | Enforce import restrictions |
| Import restriction rules | M | "Module A cannot import Module B" |
| Circular dependency detection with visualization | M | Detect and visualize import cycles |
| Cognitive complexity metric | M | Beyond cyclomatic — measures human readability |
| Widget rebuild detection | L | Flag unnecessary rebuilds |
| Build method complexity warnings | M | Build methods that are too complex to reason about |
| Async/await anti-patterns | M | Common concurrency mistakes |

**Exit criteria**: Layer enforcement correctly validates clean architecture
on a real project. Cognitive complexity correlates with developer-reported
"hard to understand" code in user study.

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

## v0.8 — Plugin System + Community

> **Pain points addressed**:
> - custom_lint is the only way to write custom rules — and it's 68x slower (#2)
> - DCM offers no extensibility for custom team rules
> - No rule sharing ecosystem exists for Dart/Flutter

### 8A: Plugin System (~4 weeks)

| Deliverable | Effort | Why it matters |
|---|---|---|
| Rust plugin API for custom rules | XL | Native-speed custom rules |
| WASM plugin system | XL | Write rules in Dart, TypeScript, Go — compiles to WASM |
| `falcon plugin create <name>` | M | Scaffold a new plugin project |
| `falcon plugin search` | M | Discover community plugins |
| Plugin performance isolation | L | Plugins can't slow down core analysis |

**Exit criteria**: A developer writes a custom rule in Dart, compiles to
WASM, distributes via registry. Plugin adds < 50ms to analysis time.

### 8B: Community + AI Plugin Intelligence (~3 weeks)

| Deliverable | Effort | Why it matters |
|---|---|---|
| Shareable rule presets (strict, recommended, flutter, riverpod, bloc) | M | Teams adopt curated configs |
| Team configuration sharing and publishing | M | Publish falcon.yaml as a package |
| Rule request and voting system | M | Community-driven rule prioritization |
| Community plugin marketplace | L | Browse, install, rate plugins |
| AI-assisted plugin development | L | "Generate a rule that catches X" using NL → plugin |
| Auto-test generation for plugins | M | Every plugin ships with tests |
| Plugin quality scoring | M | AI-powered: false positive rate, performance impact |

**Exit criteria**: 10+ community-published plugins. Marketplace has search,
install, and rating functionality.

### v0.8 Risks

| Risk | Impact | Mitigation |
|---|---|---|
| WASM plugin perf overhead | Plugins slow down analysis | Sandbox with timeout, performance budget per plugin |
| Low community contribution | Empty marketplace | Seed with official plugins, NL creation lowers barrier |

---

## v0.9 — Dashboard, Analytics + AI Learning

> **Pain points addressed**:
> - const lint wars — no data on whether rules actually help (#5)
> - No tool tracks code quality over time in the Dart ecosystem
> - Teams can't prove ROI of static analysis to management

### 9A: Metrics Dashboard (~4 weeks)

| Deliverable | Effort | Why it matters |
|---|---|---|
| Local web dashboard (localhost, no cloud) | XL | See metrics visually, no account needed |
| Historical trend tracking (per-commit, per-sprint, per-quarter) | L | Quality over time, not just a snapshot |
| Package-level drill-down for monorepos | M | Compare packages side by side |
| Technical debt visualization (treemap, heatmap) | M | Visual hot-spots for refactoring |
| Code health score over time | M | Single number executives understand |
| Team/developer statistics | M | Contribution quality patterns |

**Exit criteria**: Dashboard shows 90-day trend of code health. A tech lead
can show the CTO "our code quality improved 15% this quarter" with a chart.

### 9B: AI Learning + Adaptation (~3 weeks)

| Deliverable | Effort | Why it matters |
|---|---|---|
| Team convention learning | XL | Observe suppress patterns → adapt thresholds |
| Rule impact measurement | L | "This rule prevented 12 bugs in 6 months" |
| Signal-to-noise dashboard | M | Which rules teams value vs ignore |
| Auto-tune recommendations | M | "Consider disabling rule X (98% suppress rate)" |
| Onboarding guide generator | L | AI synthesizes conventions for new devs |
| Complexity growth alerts | M | "checkout/ complexity rising 5% per sprint" |

**Exit criteria**: After 30 days of usage, Falcon recommends rule adjustments
that reduce noise by 40%+ while maintaining bug-catch rate.

### 9C: External Integrations (~2 weeks)

| Deliverable | Effort | Why it matters |
|---|---|---|
| SonarQube bidirectional integration | L | Push metrics to SonarQube, pull config |
| Datadog / Grafana metrics export | M | Feed into existing observability stack |
| Custom webhook notifications | S | Trigger automations on quality events |
| Slack/Teams bot for quality alerts | M | "Checkout module complexity hit alarm threshold" |

**Exit criteria**: Falcon metrics appear in SonarQube dashboard alongside
Java/Kotlin metrics. Slack bot sends actionable alerts.

### v0.9 Success Metrics

| Metric | Target | How to measure |
|---|---|---|
| Dashboard adoption | > 40% of teams | Track dashboard server starts |
| Auto-tune noise reduction | > 40% fewer ignored warnings | Before/after suppress rate |
| Rule impact data accuracy | > 85% | Cross-reference with git blame + bug tracker |

### v0.9 Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Dashboard maintenance burden | Extra surface area | Ship as separate binary, optional install |
| Convention learning is wrong | Auto-tune breaks real rules | Recommendations only, never auto-disable — human confirms |

---

## v1.0 — Production Ready

> **Exit criteria**: A 50-person team using DCM can fully migrate to Falcon in
> one sprint. Zero known crashers. Every feature documented.

### 1.0A: Feature Completion (~3 weeks)

| Deliverable | Effort | Why it matters |
|---|---|---|
| 200+ lint rules | ongoing | Feature parity benchmark with DCM |
| All 19 DCM metrics implemented | S | Complete metric parity |
| Complete CI/CD pipeline support | M | GitHub, GitLab, Bitbucket, Azure |

### 1.0B: Production Hardening (~2 weeks)

| Deliverable | Effort | Why it matters |
|---|---|---|
| Production stability audit | L | Edge cases, error handling, crash recovery |
| Performance: 1M+ LOC < 5 seconds (full) | M | Benchmark on massive monorepos |
| AI features < 2s local, < 5s cloud | M | AI doesn't slow down the core experience |

### 1.0C: DCM Migration + Docs (~2 weeks)

| Deliverable | Effort | Why it matters |
|---|---|---|
| `falcon migrate-from-dcm` | M | Auto-convert DCM config to falcon.yaml |
| Rule name mapping (DCM → Falcon) | M | Familiar names for DCM users |
| Feature gap report | S | "These 5 DCM rules don't have Falcon equivalents yet" |
| Side-by-side comparison mode | M | Run both, diff results — prove equivalence |
| Comprehensive docs site (`falcon.dev`) | L | Every rule, metric, config option documented |

### v1.0 Success Metrics

| Metric | Target | How to measure |
|---|---|---|
| Rule count | 200+ | Registry count |
| GitHub stars | 5,000+ | Community traction |
| Active monthly users | 2,000+ | CLI + IDE telemetry |
| DCM migration completions | 100+ teams | Track `migrate-from-dcm` runs |
| Published plugins | 20+ | Plugin marketplace count |
| Zero known crashers | 0 | Issue tracker |

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

## v1.1 — Community Traction + AI Preset

> **Thesis**: v1.0 earned production-readiness. v1.1 plants the flag for AI-generated
> code quality — the positioning that carries Falcon from "good linter" to category owner.

### AI-Generated Code Preset
- [ ] `--preset` flag infrastructure (named rule configurations)
- [ ] Built-in presets: `recommended`, `strict`, `riverpod`, `bloc`
- [ ] **`--preset=ai-generated`** — the AI-codebase rule pack

### AI-Critical Rules (the rules AI codebases need most)
- [ ] `avoid-empty-catch-blocks` — AI's #1 anti-pattern (`catch (e) {}` or `catch (e) { print(e); }`)
- [ ] `avoid-unawaited-futures` — fire-and-forget async calls, silent production failures
- [ ] `ensure-stream-subscription-cancel` — streams/subscriptions created but never cancelled
- [ ] `ensure-disposable-lifecycle` — controllers, FocusNodes without `dispose()`
- [ ] `avoid-print-in-production` — `print()` left in non-test code
- [ ] `avoid-hardcoded-credentials` — API keys, tokens, passwords in source
- [ ] `prefer-specific-catch-type` — `catch (e)` instead of `on FormatException catch (e)`
- [ ] `avoid-excessive-widget-nesting` — widget trees >N levels deep (configurable)
- [ ] `prefer-named-parameters-for-booleans` — `MyWidget(true, false, true)` is unreadable

### Community & Credibility
- [ ] Public rule benchmarks vs. `dart analyze` + DCM (prove the value gap with data)
- [ ] Open-source showcase: analyze 50 popular Flutter repos, publish results
- [ ] "Falcon Certified" badge for pub.dev packages
- [ ] Blog post: "Why AI-Generated Flutter Code Needs Static Analysis"

---

## v1.2 — Developer Trust

> **Thesis**: Trust is earned through predictability and transparency. Teams won't
> make Falcon a CI gate unless they trust it won't break their workflow.

### Stability & Predictability
- [ ] Stability guarantees (no breaking config changes without migration path)
- [ ] Rule deprecation policy (6-month notice before removal)
- [ ] Performance regression tests (public dashboard)
- [ ] Comprehensive false-positive database with resolution status

### Community Ownership
- [ ] Public roadmap with community voting
- [ ] Rule request and voting system
- [ ] Community-contributed rule showcase

---

## v1.3 — AI Code Quality Narrative

> **Thesis**: Establish Falcon as the authority on AI-generated Flutter code quality.
> The data creates the narrative. The narrative creates demand. Demand creates integrations.

### Thought Leadership
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
