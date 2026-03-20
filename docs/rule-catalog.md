# Rule Catalog

Falcon includes 61+ lint rules across 7 categories.

## Common Rules

| Rule | Severity | Description |
|---|---|---|
| `avoid-dynamic` | Warning | Avoid using `dynamic` type — prefer explicit types |
| `avoid-empty-catch` | Warning | Don't leave catch blocks empty — exceptions are silently swallowed |
| `avoid-global-state` | Error | Avoid top-level mutable state |
| `avoid-hardcoded-credentials` | Warning | No API keys, tokens, or passwords in source code |
| `avoid-late-keyword` | Warning | Avoid `late` — prefer nullable types or constructor init |
| `avoid-long-functions` | Warning | Functions over 50 lines should be decomposed |
| `avoid-long-parameter-list` | Warning | Max 4 parameters per function |
| `avoid-nested-conditionals` | Warning | Max nesting depth for readability |
| `avoid-non-ascii-symbols` | Warning | Use ASCII identifiers |
| `avoid-print-in-production` | Warning | Use logging framework instead of print() |
| `avoid-throw-in-catch` | Error | Don't throw in catch blocks |
| `avoid-unawaited-futures` | Warning | Always await or wrap in unawaited() |
| `avoid-unnecessary-type-assertions` | Warning | Remove redundant `is` checks |
| `avoid-unnecessary-type-casts` | Warning | Remove redundant `as` casts |
| `avoid-unused-parameters` | Warning | Remove or use all parameters |
| `binary-expression-operand-order` | Info | Literal on the right side |
| `double-literal-format` | Info | Consistent double formatting |
| `newline-before-return` | Info | Blank line before return statement |
| `no-magic-numbers` | Warning | Extract constants for magic numbers |
| `prefer-correct-identifier-length` | Warning | Identifiers min 3 chars |
| `prefer-first-last` | Info | Use .first/.last instead of [0] |
| `prefer-match-file-name` | Warning | Class name should match file name |
| `prefer-named-boolean-parameters` | Warning | Named params for boolean arguments |
| `prefer-specific-catch-type` | Warning | Use `on SpecificException` not `catch (e)` |
| `prefer-trailing-comma` | Warning | Trailing commas in multi-line constructs |
| `avoid-cascade-after-if-null` | Warning | Cascade after ?? is confusing |
| `avoid-collection-methods-unrelated-types` | Warning | Type-safe collection methods |
| `avoid-double-negation` | Warning | Simplify `!!` |
| `avoid-duplicate-exports` | Warning | No duplicate exports |
| `avoid-missing-enum-constant-in-map` | Warning | All enum values in map |
| `avoid-top-level-members-in-tests` | Warning | Keep test helpers scoped |
| `prefer-const-constructors` | Warning | Use const where possible for performance |
| `prefer-extracting-callbacks` | Warning | Extract callbacks from build methods |
| `prefer-equatable` | Info | Use Equatable for value object classes |

## Flutter Rules

| Rule | Severity | Description |
|---|---|---|
| `avoid-returning-widgets` | Warning | Return widget class, not method |
| `avoid-unnecessary-setstate` | Error | Don't setState unnecessarily |
| `avoid-expanded-as-spacer` | Info | Use Spacer() instead of Expanded(SizedBox()) |
| `ensure-dispose-lifecycle` | Error | Dispose controllers and FocusNodes |
| `ensure-stream-subscription-cancel` | Error | Cancel StreamSubscriptions in dispose |
| `avoid-excessive-widget-nesting` | Warning | Max widget nesting depth |
| `ensure-semantics-label` | Warning | Interactive widgets need Semantics |
| `ensure-image-semantics` | Warning | Images need semanticLabel |
| `ensure-touch-target-size` | Info | Min 48x48dp touch targets (WCAG 2.5.5) |

## Widget & Performance Rules

| Rule | Severity | Description |
|---|---|---|
| `widget-rebuild` | Warning/Error | Detects MediaQuery.of/Theme.of in build, setState in build |
| `build-method-complexity` | Warning | Build methods over 80 lines should be split |
| `async-void` | Warning | Async functions returning void — exceptions unhandled |
| `unawaited-future` | Warning | Future chain without await — errors silently swallowed |
| `await-in-loop` | Info | Await inside loop — consider Future.wait() |

## BLoC Rules

| Rule | Severity | Description |
|---|---|---|
| `avoid-bloc-public-methods` | Warning | BLoC methods should be private |
| `avoid-emit-outside-bloc` | Error | Don't emit outside BLoC class |
| `avoid-passing-bloc-to-widget` | Warning | Use BlocProvider, not constructor |
| `prefer-bloc-extensions` | Info | Use extension methods |
| `prefer-multi-bloc-provider` | Info | Combine nested BlocProviders |

## Provider/Riverpod Rules

| Rule | Severity | Description |
|---|---|---|
| `avoid-public-notifier-properties` | Warning | Keep notifier state private |
| `avoid-ref-read-inside-build` | Error | Use ref.watch in build |
| `avoid-watch-outside-build` | Error | Don't ref.watch outside build |
| `prefer-async-value-when` | Info | Use AsyncValue.when pattern |
| `prefer-ref-read-for-methods` | Info | Use ref.read for method calls |

## Equatable Rules

| Rule | Severity | Description |
|---|---|---|
| `always-override-equals-hashcode` | Warning | Override both or neither |
| `avoid-mutable-equatable` | Warning | Equatable props should be final |
| `prefer-equatable` | Info | Use Equatable for value objects |

## Unused Code Detection

| Check | Description |
|---|---|
| `unused-code` | Declarations not referenced anywhere |
| `unused-file` | Files not imported by any other file |
| `unused-params` | Method/function parameters never used |
| `dead-code` | Unreachable code after return/throw |
| `unused-l10n` | Localization keys in ARB files with no usage |
| `unused-confidence` | Confidence scoring for unused code findings |

## Presets

Use `--preset` to apply a curated rule set:

```bash
falcon analyze --preset ai-generated    # 20 rules for AI-generated code
falcon analyze --preset strict          # 33 rules at max severity
falcon analyze --preset recommended     # 14 balanced rules
falcon analyze --preset flutter         # 9 Flutter-specific rules
falcon analyze --preset riverpod        # 8 Riverpod rules
falcon analyze --preset bloc            # 8 BLoC rules
falcon analyze --preset performance     # 6 performance rules
```
