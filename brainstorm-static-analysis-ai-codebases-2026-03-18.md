# Brainstorm: Static Analysis Issues Overlooked by AI-Generated Codebases
**Date**: 2026-03-18
**Type**: exploration / problem-solving

## Central Question
What categories of static analysis issues are systematically overlooked, ignored, or poorly handled when AI models generate code — and why do these blind spots exist?

## Mind Map

```
                              ┌─── What? (Categories of overlooked issues)
                              │
                              ├─── Why? (Root causes in AI code generation)
                              │
[STATIC ANALYSIS BLIND SPOTS] ├─── How? (Manifestation patterns & detection)
                              │
                              ├─── Who? (Stakeholders affected)
                              │
                              ├─── Constraints (Why AI struggles here)
                              │
                              └─── Wild Cards (Non-obvious angles)
```

### Branch 1: What? — Categories of Overlooked Issues

1. **Type Safety & Nullability Gaps**
   - Implicit `any` types in TypeScript/Dart
   - Missing null checks on optional chains
   - Unsafe type casts / `as` assertions sprinkled liberally
   - Generic type parameter mismatches (covariance/contravariance ignored)

2. **Resource Lifecycle & Cleanup**
   - Opened streams/controllers never closed (huge in Flutter/Dart)
   - Database connections leaked
   - Event listeners attached but never removed
   - Missing `dispose()` in stateful widgets
   - File handles left open in error paths

3. **Error Handling Anti-Patterns**
   - Empty catch blocks (`catch (e) {}`) — AI loves to swallow errors
   - Catching too broadly (`catch (Exception)` instead of specific types)
   - Missing `finally` blocks for cleanup
   - Thrown errors with no stack trace preservation
   - Async errors silently lost (unawaited futures)

4. **Dead Code & Unreachable Paths**
   - Unused imports (AI over-imports)
   - Unreachable branches after early returns
   - Variables assigned but never read
   - Duplicate conditions in if-else chains
   - Methods defined but never called

5. **Concurrency & Thread Safety**
   - Race conditions in shared mutable state
   - Missing synchronization primitives
   - Unawaited futures / fire-and-forget async calls
   - Improper isolate/thread communication
   - Deadlock-prone lock ordering

6. **Security-Sensitive Patterns**
   - Hardcoded secrets/API keys in source
   - SQL injection via string interpolation
   - Unsanitized user input in HTML/templates (XSS)
   - Insecure deserialization
   - Overly permissive CORS/permissions

7. **Code Complexity & Maintainability**
   - Cyclomatic complexity explosions (God functions)
   - Deeply nested conditionals (arrow anti-pattern)
   - Excessive parameter counts
   - Tight coupling between modules
   - Copy-paste duplication across generated files

8. **API Contract Violations**
   - Breaking changes to public APIs without versioning
   - Missing `@override` annotations
   - Interface methods with wrong signatures
   - Platform-specific API usage without availability checks

### Branch 2: Why? — Root Causes in AI Code Generation

1. **Training Data Bias**
   - Trained on Stack Overflow snippets (quick answers, not production code)
   - Tutorial-quality code dominates training sets
   - Linter-clean code is underrepresented in training data
   - AI learns "what works" not "what's correct"

2. **Context Window Limitations**
   - AI generates file-by-file, missing cross-file dependencies
   - No awareness of project-wide lint rules or analyzer config
   - Can't see the full dependency graph
   - Loses track of earlier decisions in long sessions

3. **Optimizing for the Wrong Metric**
   - AI optimizes for "compiles and runs" not "passes static analysis"
   - User satisfaction is measured by working code, not clean code
   - RLHF rewards helpfulness over correctness
   - Speed of generation prioritized over thoroughness

4. **Lack of Toolchain Awareness**
   - AI doesn't run `dart analyze` or `eslint` on its output
   - No feedback loop from analyzer to generation
   - Unaware of project-specific lint rules (analysis_options.yaml, .eslintrc)
   - Doesn't know which linter version/ruleset is active

5. **Stateless Generation**
   - Each completion is somewhat independent
   - No memory of previously flagged issues in the session
   - Can't learn from its own mistakes within a project
   - No persistent model of code quality standards for this codebase

### Branch 3: How? — Manifestation Patterns & Detection

1. **The "Works But Warns" Pattern**
   - Code compiles and runs correctly
   - But generates 50+ analyzer warnings
   - Developers ignore warnings → technical debt compounds

2. **The "Boilerplate Blast" Pattern**
   - AI generates massive amounts of structurally similar code
   - Duplication detectors light up
   - Maintenance burden multiplies silently

3. **The "Happy Path Only" Pattern**
   - AI writes the success case beautifully
   - Error paths are afterthoughts or absent
   - Null safety analysis reveals missing branches

4. **The "Import Everything" Pattern**
   - Unused imports accumulate
   - Circular dependencies creep in
   - Tree-shaking becomes less effective

5. **Detection Strategies**
   - CI/CD pipeline with strict lint gates
   - Pre-commit hooks running analyzers
   - IDE real-time analysis (which AI-generated code bypasses)
   - Periodic "lint debt" audits
   - Custom lint rules targeting AI-specific anti-patterns

### Branch 4: Who? — Stakeholders Affected

1. **Junior Developers** — Trust AI output, don't question lint warnings
2. **Code Reviewers** — Overwhelmed by volume of AI-generated PRs
3. **Security Teams** — AI-generated vulnerabilities at scale
4. **Platform/Infra Teams** — Resource leaks from AI code hitting production
5. **Future Maintainers** — Inherit complex, lint-noisy codebases

### Branch 5: Constraints — Why AI Fundamentally Struggles

1. **Static analysis requires whole-program reasoning** — AI sees fragments
2. **Lint rules are project-specific** — AI generates generic code
3. **Type inference needs full context** — AI approximates types
4. **Security analysis needs threat modeling** — AI has no threat model
5. **Complexity metrics need architectural awareness** — AI has no architecture sense

### Branch 6: Wild Cards — Non-Obvious Angles

1. **AI-generated code might need NEW lint rules** — patterns humans never wrote
2. **Linters themselves aren't designed for AI patterns** — false positive explosion?
3. **AI could become the linter** — generate AND validate in the same loop
4. **"Lint-driven generation"** — what if the AI prompt included lint rules?
5. **The meta-problem** — AI fixing AI's lint errors creates churn, not quality

## Deep Dives

### Deep Dive 1: The Resource Lifecycle Gap
*AI's biggest blind spot in Flutter/Dart and similar frameworks*

AI routinely generates `StreamController`, `AnimationController`, `TextEditingController`, `FocusNode`, etc. without corresponding `dispose()` calls. This is arguably the #1 most impactful static analysis issue because:

- Memory leaks accumulate silently
- The app works perfectly in development (short sessions)
- Crashes/slowdowns only appear in production (long sessions)
- Static analyzers CAN catch this (e.g., `close_sinks`, `cancel_subscriptions` lint rules)
- But AI doesn't know these rules exist

**Why AI misses this**: Dispose patterns require understanding object lifetime, which spans multiple methods. AI generates `initState` confidently but forgets `dispose` because it's generating one method at a time.

### Deep Dive 2: The Silent Error Swallowing Epidemic

```dart
try {
  await apiCall();
} catch (e) {
  // AI generates this empty catch or just a print statement
  print(e);
}
```

This passes compilation but is a static analysis red flag. AI does this because:
- Training data is full of tutorial code with `print(e)`
- AI wants to show "error handling" without knowing what to DO with the error
- Empty catch blocks prevent crash but hide bugs
- The `avoid_print` and `empty_catches` lint rules exist for exactly this

### Deep Dive 3: Cross-File Type Safety Erosion

AI generates files independently. File A exports a type. File B imports it but uses it slightly wrong. Neither file has an error in isolation, but the composition breaks:

- Implicit interface violations
- Return type mismatches in overrides
- Generic type parameter drift
- Missing `@immutable` annotations on classes used as value types

Static analyzers catch these at the project level, but AI generates at the file level.

## Discussion Log

### Exchange 1: Initial Mapping
- Mapped 8 categories of overlooked issues, 5 root causes, 5 manifestation patterns
- Identified resource lifecycle, error swallowing, and cross-file type safety as top 3

### Exchange 2: Strategic Pivot — Falcon as Default for AI-Generated Codebases
- User is building **Falcon** — a Rust-powered static analysis CLI for Flutter/Dart
- Falcon already addresses many of these blind spots (see ROADMAP.md v0.1–v1.0)
- Strategic question: **Can Falcon position itself as THE analysis tool for AI-generated Flutter code?**

---

## Deep Dive: Falcon as the Default Analyzer for AI-Generated Flutter Codebases

### The Strategic Thesis

```
AI tools generate Flutter code ──► Code works but has silent issues
                                         │
Falcon catches what AI misses ◄──────────┘
                                         │
Falcon becomes the safety net ◄──────────┘
every AI-generated codebase needs
```

The insight: **Every AI coding tool (Cursor, Copilot, Claude, Gemini, v0, FlutterFlow) produces Flutter code that compiles but accumulates static analysis debt.** None of them run a linter on their output. Falcon can be the universal quality gate.

### Why Falcon Is Uniquely Positioned

| Advantage | Why It Matters for AI Code |
|---|---|
| **Rust speed** | AI generates code fast — the analyzer must keep up. 1M LOC in <5s means real-time feedback loops are possible |
| **Free & open-source** | AI tools won't embed a $19/mo dependency. Free = frictionless integration |
| **CLI-first** | AI pipelines need CLI tools, not IDE plugins. Falcon is already a CLI |
| **Flutter-specific** | The only Rust-powered Flutter analyzer. AI Flutter code is the fastest-growing category |
| **AI features on roadmap** | v0.5–v0.7 already plans confidence scoring, false positive reduction, PR review — these are exactly what AI code needs |

### The AI-Generated Code Problem, Mapped to Falcon's Existing/Planned Rules

| AI Blind Spot | Falcon Rule (existing) | Falcon Rule (planned) | Version |
|---|---|---|---|
| God functions | `avoid-long-functions` | — | v0.1 ✅ |
| Deep nesting | `avoid-nested-conditionals` | — | v0.1 ✅ |
| Magic numbers everywhere | `no-magic-numbers` | AI smart resolution | v0.1 ✅ / v0.5 |
| Implicit dynamic types | `avoid-dynamic` | — | v0.1 ✅ |
| Too many parameters | `avoid-long-parameter-list` | — | v0.1 ✅ |
| Missing dispose/cleanup | — | `close-sinks`, `cancel-subscriptions` | **NEW** |
| Empty catch blocks | — | `avoid-empty-catches` | **NEW** |
| Unawaited futures | — | `avoid-unawaited-futures` | **NEW** |
| Unused imports | — | dead code detection | v0.5 |
| Hardcoded secrets | — | `avoid-hardcoded-secrets` | **NEW** |
| Copy-paste duplication | — | semantic clone detection | v0.7 |
| Missing error handling | — | PR review mode | v0.7 |
| Provider misuse in build | — | `avoid-ref-read-inside-build` | v0.3 |

### NEW: AI-Codebase-Specific Rule Pack

A dedicated rule set designed specifically for patterns AI generates but humans rarely write:

#### Tier 1: Critical (AI generates these constantly)
1. **`avoid-empty-catch-blocks`** — AI's #1 anti-pattern. Catch blocks with no logic or just `print(e)`
2. **`ensure-dispose-lifecycle`** — Controllers, streams, focus nodes created but never disposed
3. **`avoid-unawaited-futures`** — Async calls without `await` or `unawaited()` annotation
4. **`avoid-hardcoded-credentials`** — API keys, tokens, passwords in source code
5. **`avoid-print-in-production`** — `print()` statements left in non-test code

#### Tier 2: High (AI frequently misses these)
6. **`prefer-specific-catch-type`** — `catch (e)` instead of `catch (FormatException e)`
7. **`avoid-unnecessary-string-interpolation`** — `'$variable'` instead of `variable`
8. **`ensure-null-safety-completeness`** — Unnecessary `!` bang operators or missing null checks
9. **`avoid-excessive-widget-nesting`** — Widget trees >6 levels deep without extraction
10. **`prefer-named-parameters-for-booleans`** — `MyWidget(true, false, true)` is unreadable

#### Tier 3: Medium (AI-specific patterns humans don't write)
11. **`detect-boilerplate-duplication`** — AI generates similar widgets by copy-paste mutation
12. **`avoid-over-importing`** — AI imports packages it doesn't use
13. **`prefer-existing-constants`** — AI writes `Color(0xFF2196F3)` when `Colors.blue` exists
14. **`avoid-redundant-type-annotations`** — AI over-annotates where Dart can infer
15. **`detect-inconsistent-error-handling`** — Some API calls have try/catch, similar ones don't

### Integration Strategy: How Falcon Becomes the Default

#### Path 1: Direct Integration with AI Tools

```
┌──────────────────┐     ┌──────────────┐     ┌─────────────────┐
│ AI Code Generator│────►│ falcon analyze│────►│ Clean, Safe Code│
│ (Cursor, Claude, │     │ --ai-codegen  │     │ (warnings fixed) │
│  Copilot, v0)    │     │               │     │                  │
└──────────────────┘     └──────────────┘     └─────────────────┘
```

- **`falcon analyze --preset=ai-generated`** — preset that activates AI-codebase rules
- AI tools could call `falcon analyze` as a post-generation step
- Output feeds back to the AI for self-correction

#### Path 2: CI/CD Gate for AI-Heavy Teams

```yaml
# .github/workflows/falcon-ai-gate.yml
- name: Falcon AI Code Quality Gate
  run: |
    falcon analyze lib/ \
      --preset=ai-generated \
      --fail-on=warning \
      --format=github
```

Teams using AI to generate code run Falcon in CI as the safety net.

#### Path 3: The "AI Code Score"

A new Falcon metric specifically for AI-generated codebases:

```
$ falcon ai-score lib/

AI Code Quality Score: 72/100

  Resource Safety:     85/100  (2 undisposed controllers found)
  Error Handling:      45/100  (14 empty catch blocks, 8 unawaited futures)
  Type Safety:         90/100  (3 unnecessary bang operators)
  Security:           100/100  (no hardcoded credentials)
  Convention Match:    68/100  (naming inconsistencies in 6 files)
  Duplication:         55/100  (4 semantic clones detected)

Highest Priority Fixes:
  1. lib/services/api_service.dart:45 — empty catch swallows NetworkException
  2. lib/screens/home_screen.dart:23 — StreamController never closed
  3. lib/utils/auth.dart:12 — API key hardcoded
```

This gives teams a single number to track: "How safe is our AI-generated code?"

#### Path 4: MCP Server / Tool Integration

Falcon as an MCP tool that AI coding assistants can call:

```json
{
  "tool": "falcon_analyze",
  "description": "Analyze Flutter/Dart code for quality issues, especially effective on AI-generated code",
  "parameters": {
    "path": "lib/",
    "preset": "ai-generated",
    "format": "json"
  }
}
```

AI assistants that have access to Falcon via MCP can self-check their output.

### Positioning & Messaging

#### Current positioning:
> "A faster, free DCM alternative"

#### New positioning:
> "The safety net for AI-generated Flutter code"

or more specifically:

> "AI writes Flutter code. Falcon makes it production-ready."

This shifts Falcon from **competing with DCM** (a shrinking market of manual linting) to **owning a new category** (AI code quality) that's growing exponentially.

#### Target Audiences (in priority order)

1. **Teams using Cursor/Copilot for Flutter** — fastest growing segment, highest need
2. **FlutterFlow users exporting code** — generated code needs cleanup
3. **Agencies shipping AI-assisted projects** — need a quality gate for client deliverables
4. **Solo devs using AI to ship faster** — want confidence their AI code is production-safe
5. **Enterprise teams with compliance** — need to prove AI-generated code meets standards

### Competitive Moat

| Competitor | Can they do this? |
|---|---|
| `dart analyze` | No — no AI-specific rules, too slow at scale, no confidence scoring |
| DCM | No — paid, no AI awareness, no MCP integration path |
| custom_lint | No — 68x slower, can't handle large AI-generated codebases |
| SonarQube | No — Dart support is a hack, no Flutter-specific intelligence |
| **Falcon** | **Yes — Rust speed + AI-specific rules + MCP integration + free** |

Nobody is in this space. The first mover wins.

### What This Means for the Roadmap

The AI-codebase story accelerates certain features:

| Feature | Original Version | Accelerate To | Reason |
|---|---|---|---|
| `--preset=ai-generated` rule pack | Not planned | **v0.3** | Immediate differentiation |
| `avoid-empty-catches` + lifecycle rules | Not planned | **v0.3** | Core AI-code issues |
| AI Code Score metric | Not planned | **v0.4** | Compelling marketing number |
| MCP server integration | Not planned | **v0.4** | Direct AI tool integration |
| Confidence scoring | v0.5 | v0.5 | Keep as planned |
| False positive reduction | v0.5 | v0.5 | Keep as planned |
| PR review mode | v0.7 | v0.6 | High demand for AI PRs |

### Risks & Counterarguments

| Risk | Mitigation |
|---|---|
| AI tools improve and stop generating bad code | They haven't in 3+ years. Even GPT-5/Claude-4 produce lint issues. The gap is structural |
| Positioning as "AI cleanup" implies AI code is bad | Frame as "making great code production-ready" not "fixing bad code" |
| AI-specific rules may not generalize | They DO — empty catches and missing dispose hurt human code too. The preset just prioritizes them |
| Market too niche | AI-assisted Flutter development is growing. The niche IS the market |

## Synthesis

### Key Insights
1. **AI's blind spots map exactly to the gap between "compiles" and "production-ready"** — Falcon lives in that gap
2. **Resource lifecycle is the single highest-impact category** — especially in Flutter (dispose/close patterns)
3. **The root cause is structural**: AI generates locally-correct code without global awareness — Falcon does global analysis
4. **Nobody owns "AI code quality for Flutter"** — first-mover advantage is massive
5. **Falcon is already 80% positioned for this** — just needs an AI-specific rule preset and the messaging shift
6. **The feedback loop is broken** — AI never sees lint output. Falcon-as-MCP-tool closes this loop

### Decision Points
1. **Should the AI-codebase preset ship in v0.3?** — adds ~10 new rules but massive positioning value
2. **Should MCP integration be prioritized?** — enables direct AI tool integration
3. **Does the "AI Code Score" metric justify its own command?** — great for marketing, extra engineering
4. **How aggressive to be with messaging?** — "AI safety net" vs "universal Flutter analyzer that's also great for AI code"
5. **Should Falcon target FlutterFlow export specifically?** — large captive audience with generated codebases

### Next Steps
1. Execute v1.0 roadmap — build the foundation
2. Plan v2.0 as the AI-native pivot
3. Target v3.0 as category ownership — "the default for AI-generated Flutter"
4. See full PM phased breakdown below

---

## PM Breakdown: The Road to v3.0 — Falcon as the Default for AI-Generated Flutter

### The v3.0 Vision

> **v3.0 is not a feature list. It's a market position.**
>
> Falcon v3.0 = the moment when "AI-generated Flutter code" and "Falcon-analyzed"
> become synonymous. Like how "JavaScript project" implies "has ESLint."
> Every AI tool that outputs Flutter code either integrates Falcon directly
> or its users install Falcon within the first week.

---

### The Journey: v1.0 → v3.0

```
v1.0                  v2.0                  v3.0
Production            AI-Native             The Default
Linter                Platform              Standard
                                            
"DCM but              "The AI code          "If it's Flutter
 better & free"        quality engine"        and AI wrote it,
                                              Falcon checks it"
                                            
┌─────────────┐      ┌──────────────┐      ┌──────────────────┐
│ 200+ rules  │      │ AI is the    │      │ Embedded in the  │
│ IDE plugins  │      │ core, not a  │      │ AI generation    │
│ CI/CD        │─────►│ feature      │─────►│ pipeline itself  │
│ Plugin system│      │ Self-healing │      │ Industry standard│
│ Dashboard    │      │ MCP/SDK      │      │ Ecosystem moat   │
└─────────────┘      └──────────────┘      └──────────────────┘

Adoption:             Adoption:              Adoption:
Thousands             Tens of thousands      Default install
(early adopters)      (mainstream)           (category standard)
```

---

## Phase 1: v1.0 → v1.x — Earn the Right (Months 0–12)

> **Thesis**: You can't be the default for anything if you're not production-ready
> for everything. v1.x is about earning trust, building community, and proving
> that Falcon is reliable enough to gate production deployments.

### v1.0 — Production Ready (current roadmap, already planned)
- 200+ rules, full IDE integration, CI/CD, plugin system, dashboard
- DCM migration path
- Performance: 1M+ LOC in <5s

### v1.1 — Community Traction
| Item | Purpose |
|---|---|
| `--preset=ai-generated` rule pack (ship early, don't wait for v1.0) | Plant the AI-codebase flag now |
| Curated presets: `recommended`, `strict`, `riverpod`, `bloc`, `ai-generated` | One-line adoption |
| Public rule benchmarks vs. `dart analyze` + DCM | Prove the value gap with data |
| Open-source showcase: analyze 50 popular Flutter repos, publish results | Build credibility through transparency |
| "Falcon Certified" badge for pub.dev packages | Start the ecosystem flywheel |

### v1.2 — Developer Trust
| Item | Purpose |
|---|---|
| Stability guarantees (no breaking config changes without migration) | Enterprise trust |
| Rule deprecation policy (6-month notice) | Predictability |
| Performance regression tests (public dashboard) | Accountability |
| Comprehensive false-positive database with resolution status | Show you take accuracy seriously |
| Public roadmap with community voting | Shared ownership |

### v1.3 — AI Code Quality Narrative
| Item | Purpose |
|---|---|
| "State of AI-Generated Flutter Code" annual report | Establish thought leadership |
| Dataset: analysis of 10K+ AI-generated Flutter files across tools | Become the authority on AI code quality |
| Blog series: "What Cursor/Copilot/Claude get wrong in Flutter" | SEO + awareness |
| Conference talks: "Why your AI-generated Flutter app will crash in production" | Community presence |
| Partnership outreach to Cursor, Windsurf, Copilot teams | Seed the integration conversation |

#### Phase 1 Success Metrics
| Metric | Target |
|---|---|
| GitHub stars | 5,000+ |
| Monthly active projects | 10,000+ |
| Rules | 250+ |
| Community-contributed rules | 20+ |
| "Falcon" mentions in Flutter forums/Discord per month | 100+ |
| CI pipeline integrations | 2,000+ |

#### Phase 1 Exit Criteria
> Falcon is a **respected, trusted tool** that teams choose voluntarily.
> Not yet the default — but clearly the best option.

---

## Phase 2: v2.0 — AI-Native Platform (Months 12–24)

> **Thesis**: Flip the architecture. In v1.x, AI is a feature bolted onto a linter.
> In v2.0, AI is the core intelligence layer and deterministic rules are one
> input into a smarter system. Falcon stops being "a linter with AI" and becomes
> "an AI that understands Flutter codebases."

### v2.0 — The AI Core

**Architecture Shift**
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

**Core Capabilities**
| Capability | What It Does | Why It Matters for v3.0 |
|---|---|---|
| **Codebase Model** | Falcon builds a semantic model of the entire codebase — not just AST, but intent, patterns, conventions | AI code breaks conventions it doesn't know exist. Falcon knows them |
| **Convention Engine** | Automatically discovers team patterns (naming, error handling, architecture) without manual config | Zero-config analysis that's smarter than 200 hand-written rules |
| **Drift Detector** | Detects when new code (especially AI-generated) drifts from established patterns | The core value prop: "AI wrote something inconsistent, Falcon caught it" |
| **Self-Tuning Rules** | Rules adjust thresholds based on team behavior (suppress patterns, fix acceptance) | Eliminates false-positive fatigue — the #1 reason teams abandon linters |
| **Provenance Tagging** | Optionally tag code as AI-generated vs. human-written, analyze differently | Different standards for different origins |

### v2.1 — The Integration SDK

**Falcon becomes embeddable** — not just a CLI you run after, but an SDK that AI tools call during generation.

| Item | What It Does |
|---|---|
| **Falcon MCP Server** | AI coding assistants (Cursor, Windsurf, etc.) call Falcon as a tool. Self-correction loop closes |
| **Falcon SDK (Rust library)** | Embeddable analysis engine. AI tools compile Falcon into their pipeline |
| **Falcon API (HTTP)** | Cloud-hosted analysis endpoint. `POST /analyze` with code, get issues back |
| **Falcon LSP Protocol Extensions** | Custom LSP messages for AI-specific diagnostics (confidence, provenance, convention match) |
| **Webhook Callbacks** | `on_ai_code_generated → falcon.analyze → feedback_to_ai` pipeline |

```
AI Tool Integration Flow (v2.1):

  Developer types prompt
       │
       ▼
  AI generates Flutter code
       │
       ▼
  Falcon SDK analyzes in-flight    ◄── NEW: happens DURING generation
       │
       ├── Critical issues? ──► AI self-corrects before showing to user
       │
       ├── Warnings? ──► Shown inline with AI output
       │
       └── Clean? ──► Code delivered to user
```

### v2.2 — The AI Code Score

**Falcon defines what "production-ready" means for AI-generated Flutter code.**

| Item | What It Does |
|---|---|
| **AI Code Score (0-100)** | Single number: "How production-ready is this AI-generated code?" |
| **Score Breakdown** | Resource Safety, Error Handling, Type Safety, Security, Convention Match, Duplication |
| **Score API** | Embeddable score badge for READMEs, PR comments, dashboards |
| **Benchmark Database** | "Average Cursor-generated Flutter app scores 64. Average human-written scores 78." |
| **Score Trends** | Track score over time. "Your AI code quality improved 12 points this quarter" |
| **Certification** | "Falcon Certified: Production Ready" badge for repos that maintain 85+ |

### v2.3 — The Learning Engine

| Item | What It Does |
|---|---|
| **Cross-Project Learning** | Anonymized patterns from thousands of projects improve Falcon's convention detection |
| **AI-Tool Profiling** | "Cursor tends to miss dispose() in 34% of StatefulWidgets. Claude misses it in 12%." |
| **Auto-Rule Generation** | Falcon observes patterns across projects and proposes new rules automatically |
| **Fix Effectiveness Tracking** | Which auto-fixes do teams accept? Which do they reject? Feed back into fix quality |
| **Regression Prediction** | "Based on similar codebases, this pattern will cause a production issue within 3 months" |

#### Phase 2 Success Metrics
| Metric | Target |
|---|---|
| GitHub stars | 20,000+ |
| Monthly active projects | 50,000+ |
| AI tool integrations (MCP/SDK) | 5+ major tools |
| API calls per month | 1M+ |
| Repos with "Falcon Certified" badge | 500+ |
| AI Code Score adopted as industry metric | Referenced in 3+ conference talks by others |

#### Phase 2 Exit Criteria
> Falcon is the **recognized authority** on AI-generated Flutter code quality.
> Multiple AI tools have integrated it. The "AI Code Score" is becoming a standard.

---

## Phase 3: v3.0 — The Default Standard (Months 24–36)

> **Thesis**: "Default" isn't a feature — it's a network effect. v3.0 is the moment
> where NOT using Falcon on an AI-generated Flutter codebase feels like shipping
> a Node.js project without a `package.json`. It's just what you do.

### What "Default" Actually Means

```
Levels of adoption:

Level 1: "I've heard of it"                    ← v1.0 (awareness)
Level 2: "My team uses it"                     ← v1.x (adoption)  
Level 3: "Of course we use it, doesn't everyone?" ← v2.x (mainstream)
Level 4: "Wait, you DON'T use it?"             ← v3.0 (default)    ◄── HERE
Level 5: "It's just part of Flutter"           ← v4.0+ (standard)
```

### v3.0 — The Category Standard

#### 3.0.1: Ecosystem Lock-In (the good kind)

| Item | What It Does | Network Effect |
|---|---|---|
| **Flutter `create` template integration** | `flutter create` optionally includes `falcon.yaml` | Every new project starts with Falcon |
| **pub.dev quality scoring** | Falcon Score shown on pub.dev package pages | Package authors adopt Falcon to improve visibility |
| **FlutterFlow native integration** | FlutterFlow export includes Falcon analysis | Every exported project is Falcon-analyzed |
| **AI tool default configs** | Cursor, Windsurf, Copilot ship with Falcon MCP enabled for Flutter | AI tools adopt Falcon as their quality layer |
| **`dart analyze` bridge** | Falcon rules surface in `dart analyze` output | Developers don't even know they're using Falcon |

#### 3.0.2: The Quality Graph

| Item | What It Does | Why It Creates Lock-In |
|---|---|---|
| **Cross-Project Intelligence** | Falcon knows how 50K+ Flutter projects handle auth, state, errors | No competitor can replicate this dataset |
| **AI Tool Report Card** | Monthly report: "Which AI tools produce the cleanest Flutter code?" | AI tools compete on Falcon scores |
| **Flutter Health Index** | Industry metric: "Average Flutter project quality is 73/100 this quarter" | Falcon defines the measurement |
| **Convention Marketplace** | Teams share their convention configs. "Download Stripe's Flutter conventions" | Network effects from shared standards |
| **Compliance Engine** | "This AI-generated code meets SOC2/HIPAA/PCI requirements for static analysis" | Enterprise can't NOT use Falcon |

#### 3.0.3: The Platform

| Item | What It Does |
|---|---|
| **Falcon Cloud** | Hosted analysis with team dashboards, trend tracking, alerts |
| **Falcon for Enterprise** | SSO, audit logs, custom policies, compliance reporting |
| **Falcon Marketplace** | Third-party rule packs, convention configs, integrations |
| **Falcon Certification Program** | "Certified Falcon Analyst" for consultants/agencies |
| **Falcon Partner Program** | AI tools, IDEs, CI/CD platforms certified to integrate |

### v3.0 Revenue Model

| Tier | Price | For Whom |
|---|---|---|
| **Core CLI** | Free forever | Individual devs, open source |
| **Team** | $9/seat/month | Team dashboards, shared conventions, trend tracking |
| **Enterprise** | $29/seat/month | SSO, compliance, audit logs, custom policies |
| **API** | Usage-based | AI tools embedding Falcon, CI/CD platforms |
| **Certification** | One-time fee | Agencies, consultants |

**Key pricing principle**: The tool that makes AI code safe must be free. The platform that proves it to your boss is paid.

#### Phase 3 Success Metrics
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

Once this flywheel spins, it's self-reinforcing. Competitors can't catch up because:
1. They don't have the convention data (requires scale)
2. They don't have the AI tool integrations (requires relationships)
3. They don't have the community trust (requires time)

---

## Risk Matrix: v1.0 → v3.0

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| **AI tools improve enough that code quality gap closes** | Fatal | Low | The gap is structural (context-free generation). Even if quality improves 2x, conventions still need checking |
| **Google ships a native AI-code linter in `dart analyze`** | High | Medium | Move faster. By the time Google ships, Falcon should have 2 years of convention data. Also: Google moves slow on tooling |
| **DCM pivots to AI-code analysis** | Medium | Medium | DCM is paid + closed. Can't build community flywheel. Price advantage is permanent |
| **Flutter itself declines** | Fatal | Low | Hedge: Falcon's architecture (Rust AST engine) could support Kotlin/Swift. But don't split focus pre-v2.0 |
| **Can't monetize (everyone expects free)** | High | Medium | Free CLI + paid cloud/enterprise. Same model as ESLint → ESLint Cloud, Prettier → Prettier Cloud |
| **AI coding tools build their own analysis** | High | Medium | Building a full Flutter analyzer is a massive investment. Easier to integrate Falcon. Be the standard, not the competitor |

---

## Timeline Summary

```
2026                      2027                      2028
├───── v1.0-1.x ─────────┼───── v2.0-2.x ─────────┼───── v3.0 ──────►
│                         │                         │
│ Earn Trust              │ AI-Native Platform      │ The Default
│ • Production ready      │ • Codebase model        │ • Ecosystem lock-in
│ • Community traction    │ • MCP/SDK integration   │ • Quality graph
│ • Thought leadership    │ • AI Code Score         │ • Falcon Cloud
│ • AI preset (plant flag)│ • Learning engine       │ • Revenue engine
│                         │ • Tool partnerships     │ • Industry standard
│                         │                         │
│ 10K projects            │ 50K projects            │ 200K+ projects
│ "Best Flutter linter"   │ "AI code quality engine"│ "Just part of Flutter"
```

---

## The One Decision That Matters Now

All of v3.0 depends on one thing executing well in v1.x:

**Ship the `--preset=ai-generated` rule pack early, run it against every AI-generated
Flutter project you can find, and publish the results loudly.**

The data — "Cursor-generated Flutter code has 3.2x more empty catch blocks than
human-written code" — is what creates the narrative. The narrative creates demand.
Demand creates integrations. Integrations create the default.
