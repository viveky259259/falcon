# Falcon Roadmap

Falcon is a Rust-powered static analysis CLI for Flutter/Dart. Open-source, free,
unlimited LOC. Built to be the tool DCM should have been — faster, smarter, and
accessible to every Flutter developer.

> **Positioning**: Not just a linter. A codebase intelligence engine.
>
> **Core thesis**: At scale, developers don't need more warnings — they need signal.
> Falcon combines Rust-speed deterministic analysis with AI-powered intelligence
> to surface what actually matters.

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

## v0.3 — Rules Expansion + Monorepo Support

> **Pain points addressed**:
> - custom_lint 68x slower than dart analyze on large projects (#2)
> - Melos monorepo analysis broken — per-package configs ignored (#6)
> - DCM free tier capped at 100 rules (#3)

Grow the rule library to 50+ while adding first-class monorepo support —
the combination that makes Falcon viable for real teams.

### Common Dart Rules (+15)
- [ ] `avoid-unused-parameters`
- [ ] `prefer-correct-identifier-length`
- [ ] `avoid-cascade-after-if-null`
- [ ] `avoid-collection-methods-with-unrelated-types`
- [ ] `avoid-duplicate-exports`
- [ ] `avoid-missing-enum-constant-in-map`
- [ ] `avoid-non-ascii-symbols`
- [ ] `avoid-throw-in-catch-block`
- [ ] `avoid-top-level-members-in-tests`
- [ ] `avoid-unnecessary-type-assertions`
- [ ] `avoid-unnecessary-type-casts`
- [ ] `binary-expression-operand-order`
- [ ] `double-literal-format`
- [ ] `newline-before-return`
- [ ] `prefer-first-last`

### Provider/Riverpod Rules (+5)
- [ ] `avoid-ref-read-inside-build`
- [ ] `avoid-watch-outside-build`
- [ ] `prefer-async-value-when`
- [ ] `avoid-public-notifier-properties`
- [ ] `prefer-ref-read-for-methods`

### BLoC Rules (+5)
- [ ] `avoid-bloc-public-methods`
- [ ] `avoid-emit-outside-bloc`
- [ ] `prefer-multi-bloc-provider`
- [ ] `avoid-passing-bloc-to-widget`
- [ ] `prefer-bloc-extensions`

### Equatable Rules (+3)
- [ ] `always-override-equals-and-hashcode`
- [ ] `avoid-mutable-equatable`
- [ ] `prefer-equatable`

### Monorepo Support (NEW — addresses Pain Point #6)
- [ ] Auto-detect Melos workspaces (`melos.yaml`)
- [ ] Auto-detect Dart pub workspaces (`pubspec.yaml` workspace field)
- [ ] Per-package `falcon.yaml` configuration with inheritance
- [ ] `falcon analyze --workspace` — analyze all workspace packages
- [ ] Aggregate cross-package reporting (per-package + summary)
- [ ] Respect workspace boundaries (exclude non-workspace dirs)
- [ ] Shared rule configuration with per-package overrides

### Infrastructure
- [ ] Suppression comments (`// ignore: rule-name`)
- [ ] `// ignore_for_file:` directive for all rules
- [ ] Rule documentation generator
- [ ] YAML config validation with clear error messages (not silent failures like dart analyzer)

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
- [ ] GitLab CI template
- [ ] Bitbucket Pipelines pipe
- [ ] Pre-commit hook support
- [x] Exit code configuration (fail on warning vs error only)

### Performance Targets
- [x] Parse 100K+ LOC projects in < 2 seconds
- [x] Incremental re-analysis of changed files in < 500ms
- [x] Memory usage under 100MB for large projects

---

## v0.5 — Advanced Detection + AI Foundation

> **Pain points addressed**:
> - Dead code tools use regex, not AST — unreliable (#7)
> - False positives and flaky lints destroy trust (#4)
> - const lint wars — rules without proven value (#5)
> - No tool distinguishes "definitely unused" from "maybe unused" (#7)

This version has two halves: hardened detection that developers can trust,
and the AI infrastructure that powers everything from v0.6 onward.

### Advanced Unused Detection
- [ ] Unused l10n/localization keys detection
- [ ] Cyclic dependency detection (multi-level) with visualization
- [ ] Over-promoted dependency detection
- [ ] Under-promoted dependency detection (dev → regular)
- [ ] Unused method parameters detection
- [ ] Dead code path detection (unreachable branches)

### AI Foundation — Infrastructure (NEW)
- [ ] `falcon.yaml` AI configuration section (`ai:` block)
- [ ] BYOK (Bring Your Own Key) — OpenAI, Anthropic, Gemini API key support
- [ ] Local model support (Ollama, llama.cpp) for privacy-sensitive codebases
- [ ] Codebase context extraction pipeline (import graph, naming patterns, conventions)
- [ ] AI feature toggle (all AI features off by default, opt-in)
- [ ] `falcon ai setup` — interactive AI configuration wizard

### AI: Confidence Scoring (NEW — addresses Pain Point #7)
- [ ] Dead code confidence levels: "95% unused" vs "72% — check dynamic refs"
- [ ] Mark dynamic/reflection usage patterns that reduce confidence
- [ ] `--min-confidence=90` flag to filter by confidence
- [ ] Confidence explanations ("unused except for 1 dynamic `Type.toString()` reference")

### AI: False Positive Reduction (NEW — addresses Pain Points #4, #5)
- [ ] Context-aware `no-magic-numbers` — skip HTTP status codes, well-known constants
- [ ] Smart `avoid-late-keyword` — skip test files and framework-required patterns
- [ ] Rule impact tracking — "this rule caught 3 real issues / generated 47 ignored warnings"
- [ ] Per-rule signal-to-noise ratio reporting
- [ ] Auto-suggest rule configuration based on team's suppress patterns

### AI: Smart Fix Suggestions (NEW)
- [ ] Context-aware fix generation using LLM (matches codebase naming conventions)
- [ ] "Existing constant available" detection for magic numbers
- [ ] Batch auto-fix with preview (`falcon fix --preview`)
- [ ] `falcon explain <rule>` — AI-generated contextual explanation of any violation

---

## v0.6 — IDE Integration

> **Pain points addressed**:
> - Dart analyzer 70-second lag spikes in IDE (#1)
> - custom_lint hangs VS Code indefinitely (#2)
> - IDE integration is DCM's lock-in mechanism — must match it

The moment Falcon appears in the editor, it becomes the default tool.
Everything before this is CLI — this version makes it invisible infrastructure.

### VS Code Extension
- [ ] Language Server Protocol (LSP) implementation in Rust
- [ ] Real-time analysis in editor (sub-second, not 70-second lag)
- [ ] Inline diagnostics (squiggly underlines)
- [ ] Quick fixes (code actions) — rule-based + AI-generated
- [ ] Auto-fixes on save
- [ ] Configuration UI for falcon.yaml
- [ ] Status bar widget showing issue counts
- [ ] AI explanation tooltips on hover (opt-in)

### IntelliJ/Android Studio Plugin
- [ ] External annotator for inline warnings
- [ ] Quick-fix intentions
- [ ] Tool window for analysis results

### AI: IDE Features (NEW)
- [ ] "Explain this warning" — hover tooltip with AI-generated explanation
- [ ] "Show me similar code" — find semantically related code in the project
- [ ] AI-powered quick fix suggestions alongside rule-based fixes
- [ ] Confidence indicators on unused code warnings (green/yellow/red dot)

---

## v0.7 — AI-Powered Analysis

> **This is Falcon's differentiator.** No other Dart/Flutter tool does this.
> These features transform Falcon from "a faster DCM" into "an AI teammate
> that understands your codebase."

### AI: PR Review Mode (NEW)
- [ ] `falcon review --diff HEAD~1` — AI-powered review of changes
- [ ] Pattern consistency checking ("8 similar handlers use try/catch, yours doesn't")
- [ ] Convention violation detection (naming, architecture, error handling patterns)
- [ ] Missing test detection ("this function has 4 code paths, tests cover 1")
- [ ] Review output as PR comment (GitHub, GitLab)
- [ ] Configurable review strictness (quick / standard / thorough)

### AI: Codebase Intelligence (NEW)
- [ ] God file decomposition advisor ("here's how to safely split this 8K-line file")
- [ ] Semantic code clone detection (same logic, different syntax)
- [ ] Architectural drift detection ("repository accumulating UI logic")
- [ ] Codebase health narrative ("complexity rose 31% in checkout/ this quarter — here's why")
- [ ] Technical debt scoring with effort estimation

### AI: Natural Language Rule Creation (NEW — killer feature)
- [ ] `falcon rule create "warn when BLoC event handler calls another handler directly"`
- [ ] AI generates AST-matching rule logic from English description
- [ ] Auto-generates test cases for the new rule
- [ ] Scans codebase for existing violations
- [ ] Adds rule to falcon.yaml with configurable severity
- [ ] Rule explanation and documentation auto-generated

### Advanced Analysis (from original v0.7)
- [ ] Layer dependency enforcement (clean architecture validation)
- [ ] Package boundary validation
- [ ] Import restriction rules
- [ ] Circular dependency detection with visualization
- [ ] Cognitive complexity metric
- [ ] Widget rebuild detection (unnecessary rebuilds)
- [ ] Build method complexity warnings
- [ ] Async/await anti-patterns

---

## v0.8 — Plugin System + Community

> **Pain points addressed**:
> - custom_lint is the only way to write custom rules — and it's 68x slower (#2)
> - DCM offers no extensibility for custom team rules
> - No rule sharing ecosystem exists for Dart/Flutter

### Plugin System
- [ ] Rust plugin API for custom rules
- [ ] WASM-based plugin system (write rules in any language that compiles to WASM)
- [ ] Rule template generator (`falcon plugin create <name>`)
- [ ] Plugin registry and discovery (`falcon plugin search`)
- [ ] Plugin performance isolation (plugins can't slow down core analysis)

### Community
- [ ] Shareable rule presets (strict, recommended, flutter, riverpod, bloc)
- [ ] Team configuration sharing and publishing
- [ ] Rule request and voting system
- [ ] Community plugin marketplace

### AI: Plugin Intelligence (NEW)
- [ ] AI-assisted plugin development ("generate a rule that catches X")
- [ ] Auto-test generation for custom rules
- [ ] Plugin quality scoring (false positive rate, performance impact)

---

## v0.9 — Dashboard, Analytics + AI Learning

> **Pain points addressed**:
> - const lint wars — no data on whether rules actually help (#5)
> - No tool tracks code quality over time in the Dart ecosystem
> - Teams can't prove ROI of static analysis to management

### Metrics Dashboard
- [ ] Local web dashboard for project metrics
- [ ] Historical trend tracking (per-commit, per-sprint, per-quarter)
- [ ] Team/developer statistics
- [ ] Technical debt visualization (treemap, heatmap)
- [ ] Code health score over time
- [ ] Package-level drill-down for monorepos

### AI: Learning & Adaptation (NEW)
- [ ] Team convention learning — observe suppress patterns, adapt rule thresholds
- [ ] Rule impact measurement — "this rule prevented 12 bugs in 6 months"
- [ ] Signal-to-noise dashboard — which rules teams actually value vs ignore
- [ ] Auto-tune recommendations — "consider disabling rule X (98% suppress rate)"
- [ ] Onboarding guide generator — AI synthesizes codebase conventions for new devs
- [ ] Complexity growth alerts — "checkout/ complexity rising 5% per sprint"

### Integration
- [ ] SonarQube bidirectional integration (push metrics, pull configurations)
- [ ] Datadog / Grafana metrics export
- [ ] Custom webhook notifications
- [ ] Slack/Teams bot for quality alerts

---

## v1.0 — Production Ready

### Goals
- [ ] 200+ lint rules
- [ ] All 19 DCM metrics implemented
- [ ] Full IDE integration (VS Code + IntelliJ)
- [ ] Complete CI/CD pipeline support (GitHub, GitLab, Bitbucket, Azure)
- [ ] Plugin ecosystem with community marketplace
- [ ] AI-powered analysis, review, and rule creation
- [ ] Dashboard with historical trend tracking
- [ ] Production-ready stability and performance
- [ ] Comprehensive documentation site
- [ ] Migration guide from DCM (`falcon migrate-from-dcm`)

### Performance Targets
- [ ] Parse 1M+ LOC monorepos in < 5 seconds (full analysis)
- [ ] Incremental analysis in < 500ms (changed files only)
- [ ] Watch mode with sub-second feedback
- [ ] Memory usage under 100MB for large projects
- [ ] AI features add < 2 seconds for local models, < 5 seconds for cloud

### DCM Migration
- [ ] `falcon migrate-from-dcm` — auto-convert DCM config to falcon.yaml
- [ ] Rule name mapping (DCM rule names → Falcon equivalents)
- [ ] Feature gap report ("these DCM rules don't have Falcon equivalents yet")
- [ ] Side-by-side comparison mode (run both, diff results)

---

## Future Ideas (Post v1.0)

### AI-Native Features
- Refactoring simulation ("what if we migrate checkout/ to Riverpod?")
- State management migration assistant (setState → BLoC → Riverpod)
- AI-powered code review automation (fully autonomous PR gate)
- Flutter upgrade compatibility checker (will my code work on Flutter N+1?)
- Test generation from code analysis (not just stubs — actual meaningful tests)
- Vulnerability / anti-pattern radar (trained on Flutter GitHub issues)

### Platform Expansion
- Multi-language support (Kotlin, Swift for platform channels)
- Code generation quality analysis (build_runner output)
- Accessibility lint rules for Flutter widgets
- Performance profiling integration (DevTools bridge)
- API design consistency rules

### Ecosystem
- Falcon Cloud — hosted dashboard for teams (SaaS offering)
- Enterprise SSO and audit logging
- IDE extension marketplace for AI-powered quick fixes
- Training data from open-source Flutter projects (anonymized)

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
