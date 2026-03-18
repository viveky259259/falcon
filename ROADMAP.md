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

### Advanced Unused Detection (Complete)
- [x] Unused l10n/localization keys detection
- [x] Cyclic dependency detection (multi-level) with visualization
- [x] Over-promoted dependency detection
- [x] Under-promoted dependency detection (dev → regular)
- [x] Unused method parameters detection
- [x] Dead code path detection (unreachable branches)

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
