# Falcon Roadmap

Falcon is a Rust-powered static analysis CLI for Flutter/Dart that aims to match and exceed
the feature set of DCM (Dart Code Metrics). This roadmap outlines the journey from MVP to
full feature parity.

---

## v0.1 -- MVP (Current)

Core infrastructure with essential metrics, lint rules, and unused detection.

### Metrics (6 of 19)
- [x] Cyclomatic complexity
- [x] Lines of code (LOC)
- [x] Source lines of code (SLOC)
- [x] Maintainability index
- [x] Maximum nesting level
- [x] Number of parameters
- [x] Number of methods (class-level)

### Lint Rules (15)

**Dart rules (10):**
- [x] `avoid-long-functions` -- configurable line threshold
- [x] `avoid-long-parameter-list` -- configurable max params
- [x] `avoid-nested-conditionals` -- configurable max depth
- [x] `avoid-dynamic` -- discourage dynamic type usage
- [x] `prefer-trailing-comma` -- multi-line lists
- [x] `avoid-global-state` -- mutable top-level variables
- [x] `avoid-late-keyword` -- prefer nullable/factory patterns
- [x] `no-magic-numbers` -- extract to named constants
- [x] `prefer-match-file-name` -- class name matches file
- [x] `avoid-double-negation` -- simplify expressions

**Flutter rules (5):**
- [x] `avoid-returning-widgets` -- extract to widget classes
- [x] `prefer-extracting-callbacks` -- named methods for callbacks
- [x] `avoid-unnecessary-setstate` -- lifecycle method safety
- [x] `avoid-expanded-as-spacer` -- use Spacer widget
- [x] `prefer-const-constructors` -- performance optimization

### Unused Detection
- [x] Unused Dart files (not imported anywhere)
- [x] Unused top-level declarations (classes, functions, enums, mixins)
- [x] Unused pubspec.yaml dependencies

### CLI Commands
- [x] `falcon analyze <path>` -- full analysis
- [x] `falcon metrics <path>` -- metrics only
- [x] `falcon check-unused-code <path>`
- [x] `falcon check-unused-files <path>`
- [x] `falcon check-dependencies <path>`
- [x] `falcon init` -- generate falcon.yaml

### Output & Config
- [x] Console output (colored)
- [x] JSON output
- [x] YAML configuration (falcon.yaml)
- [x] File/rule suppression comments

---

## v0.2 -- Metrics Expansion

Complete the full set of function and class metrics.

### Function/Method Metrics (+3)
- [ ] Halstead volume
- [ ] Widgets nesting level (Flutter-specific)
- [ ] Number of used widgets (Flutter-specific)

### Class Metrics (+10)
- [ ] Coupling between object classes (CBO)
- [ ] Depth of inheritance tree (DIT)
- [ ] Number of added methods
- [ ] Number of implemented interfaces
- [ ] Number of overridden methods
- [ ] Response for class (RFC)
- [ ] Tight class cohesion (TCC)
- [ ] Weight of class (WOC)
- [ ] Weighted methods per class (WMC)
- [ ] Lack of cohesion of methods (LCOM)

### Reporting
- [ ] HTML report output format with interactive charts
- [ ] Metric threshold levels (noted, warning, alarm)
- [ ] Per-metric severity configuration

---

## v0.3 -- Rules Expansion (50+ Rules)

Grow the rule library significantly with framework-specific rules.

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

### Infrastructure
- [ ] Suppression comments (`// ignore: rule-name`)
- [ ] `// ignore_for_file:` directive for all rules
- [ ] Rule documentation generator

---

## v0.4 -- Advanced Unused Detection

### Detection Features
- [ ] Unused l10n/localization keys detection
- [ ] Cyclic dependency detection (multi-level)
- [ ] Over-promoted dependency detection
- [ ] Under-promoted dependency detection (dev -> regular)
- [ ] Unused method parameters detection
- [ ] Dead code path detection

### Workflow Features
- [ ] `--exclude-public-api` flag
- [ ] Baseline support (only report new violations)
- [ ] Baseline file management (`falcon baseline`)
- [ ] Diff-only mode for CI (`--since=HEAD~1`)

---

## v0.5 -- IDE Integration

### VS Code Extension
- [ ] Language Server Protocol (LSP) implementation
- [ ] Real-time analysis in editor
- [ ] Inline diagnostics (squiggly underlines)
- [ ] Quick fixes (code actions)
- [ ] Auto-fixes on save
- [ ] Configuration UI
- [ ] Status bar widget showing issue counts

### IntelliJ/Android Studio Plugin
- [ ] External annotator for inline warnings
- [ ] Quick-fix intentions
- [ ] Tool window for analysis results

---

## v0.6 -- CI/CD and Reporting

### Output Formats
- [ ] CodeClimate output format
- [ ] GitLab code quality format
- [ ] Checkstyle XML format
- [ ] SARIF format (GitHub code scanning)
- [ ] Sonar format

### CI Integration
- [ ] GitHub Actions action (`falcon-lint/action`)
- [ ] GitLab CI template
- [ ] Bitbucket Pipelines pipe
- [ ] Azure DevOps task
- [ ] Pre-commit hook support

### Configuration
- [ ] Exit code configuration (fail on warning vs error only)
- [ ] PR comment bot (post analysis results as PR comments)
- [ ] Custom severity mapping per CI context

---

## v0.7 -- Advanced Analysis

### Code Quality
- [ ] Duplicate widget detection
- [ ] Code similarity analysis (clone detection)
- [ ] Refactoring suggestions
- [ ] Technical debt scoring and estimation
- [ ] Cognitive complexity metric

### Architecture
- [ ] Layer dependency enforcement (clean architecture validation)
- [ ] Package boundary validation
- [ ] Import restriction rules
- [ ] Circular dependency detection with visualization

### Performance Analysis
- [ ] Widget rebuild detection (unnecessary rebuilds)
- [ ] Build method complexity warnings
- [ ] Async/await anti-patterns

---

## v0.8 -- Plugin System

### Custom Rules
- [ ] Rust plugin API for custom rules
- [ ] WASM-based plugin system
- [ ] Rule template generator
- [ ] Plugin registry and discovery

### Community
- [ ] Shareable rule presets (strict, recommended, flutter, etc.)
- [ ] Team configuration sharing
- [ ] Rule request and voting system

---

## v0.9 -- Dashboard & Analytics

### Metrics Dashboard
- [ ] Local web dashboard for project metrics
- [ ] Historical trend tracking
- [ ] Team/developer statistics
- [ ] Technical debt visualization
- [ ] Code health score over time

### Integration
- [ ] SonarQube integration
- [ ] Datadog metrics export
- [ ] Custom webhook notifications

---

## v1.0 -- Feature Parity & Beyond

### Goals
- [ ] 200+ lint rules
- [ ] All 19 DCM metrics implemented
- [ ] Full IDE integration (VS Code + IntelliJ)
- [ ] Complete CI/CD pipeline support
- [ ] Plugin ecosystem
- [ ] Dashboard with trend tracking
- [ ] Production-ready stability
- [ ] Comprehensive documentation site
- [ ] Migration guide from DCM

### Performance Targets
- [ ] Parse 100K+ LOC projects in < 2 seconds
- [ ] Incremental analysis (only re-analyze changed files)
- [ ] Watch mode for continuous analysis
- [ ] Memory usage under 100MB for large projects

---

## Future Ideas (Post v1.0)

- AI-powered refactoring suggestions
- Code review automation (PR gate)
- Multi-language support (Kotlin, Swift for platform channels)
- Code generation quality analysis (build_runner output)
- Accessibility lint rules for Flutter widgets
- Performance profiling integration
- Test coverage quality analysis (not just quantity)
- API design consistency rules
- State management migration assistant
- Flutter upgrade compatibility checker
