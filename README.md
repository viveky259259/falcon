# Falcon

**Rust-powered static analysis for Flutter/Dart**

Falcon is a blazing-fast static analysis CLI that calculates code metrics, enforces lint rules, and detects unused code in your Flutter and Dart projects. Built in Rust with [tree-sitter](https://tree-sitter.github.io/) for parsing and [Rayon](https://docs.rs/rayon/) for parallel analysis.

## Features

- **6 Code Metrics** -- cyclomatic complexity, LOC/SLOC, maintainability index, max nesting, parameter count, method count
- **15 Lint Rules** -- 10 Dart + 5 Flutter-specific rules covering common anti-patterns
- **Unused Detection** -- find unused files, unused declarations, and unused dependencies
- **Fast** -- Rust + tree-sitter + parallel analysis = near-instant results
- **Configurable** -- YAML config with per-rule severity and thresholds
- **Multiple Output Formats** -- colored console and JSON

## Installation

### From source (requires Rust toolchain)

```bash
cargo install --path .
```

### Build from source

```bash
git clone https://github.com/falcon-lint/falcon.git
cd falcon
cargo build --release
# Binary at target/release/falcon
```

## Usage

### Full analysis

```bash
falcon analyze lib/
```

### Metrics only

```bash
falcon metrics lib/
```

### Unused detection

```bash
falcon check-unused-code lib/
falcon check-unused-files lib/
falcon check-dependencies .
```

### JSON output

```bash
falcon analyze lib/ --format json
```

### Generate config

```bash
falcon init
```

## Configuration

Create a `falcon.yaml` in your project root (or run `falcon init`):

```yaml
metrics:
  cyclomatic_complexity: 20
  lines_of_code: 100
  number_of_parameters: 4
  maximum_nesting_level: 5
  number_of_methods: 10

rules:
  - avoid-long-functions:
      severity: warning
      max-lines: 50
  - avoid-returning-widgets: error
  - prefer-const-constructors: info
  - avoid-dynamic
  - avoid-global-state

unused:
  exclude:
    - "**/*.g.dart"
    - "**/*.freezed.dart"

exclude:
  - "build/**"
  - ".dart_tool/**"
  - "**/*.g.dart"
```

## Suppression

Suppress rules per-line or per-file:

```dart
// ignore: avoid-dynamic
dynamic value = getData();

// ignore_for_file: no-magic-numbers
```

## Rules

### Dart Rules

| Rule | Description | Default Severity |
|------|-------------|-----------------|
| `avoid-long-functions` | Functions exceeding max lines | Warning |
| `avoid-long-parameter-list` | Too many parameters | Warning |
| `avoid-nested-conditionals` | Deep nesting depth | Warning |
| `avoid-dynamic` | Using `dynamic` type | Warning |
| `prefer-trailing-comma` | Multi-line trailing commas | Info |
| `avoid-global-state` | Mutable top-level variables | Warning |
| `avoid-late-keyword` | Using `late` keyword | Info |
| `no-magic-numbers` | Unnamed numeric literals | Info |
| `prefer-match-file-name` | Class name matches file | Info |
| `avoid-double-negation` | `!!` expressions | Warning |

### Flutter Rules

| Rule | Description | Default Severity |
|------|-------------|-----------------|
| `avoid-returning-widgets` | Returning widgets from methods | Warning |
| `prefer-extracting-callbacks` | Long inline callbacks | Info |
| `avoid-unnecessary-setstate` | setState in lifecycle methods | Warning |
| `avoid-expanded-as-spacer` | Expanded with empty child | Info |
| `prefer-const-constructors` | Missing const constructors | Info |

## Development

```bash
# Run tests
cargo test

# Build release binary
cargo build --release

# Run on a project
cargo run -- analyze /path/to/flutter/project
```

## License

MIT
