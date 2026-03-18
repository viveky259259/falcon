# Rule Catalog

Falcon includes 58+ lint rules across 6 categories.

## Common Rules

| Rule | Severity | Description |
|---|---|---|
| `avoid-dynamic` | Error | Avoid using `dynamic` type |
| `avoid-empty-catch` | Error | Don't leave catch blocks empty |
| `avoid-global-state` | Error | Avoid top-level mutable state |
| `avoid-hardcoded-credentials` | Error | No API keys or passwords in source |
| `avoid-late-keyword` | Warning | Avoid `late` keyword where possible |
| `avoid-long-functions` | Warning | Functions should be under 100 lines |
| `avoid-long-parameter-list` | Warning | Max 6 parameters per function |
| `avoid-nested-conditionals` | Warning | Max 3 nesting levels |
| `avoid-non-ascii-symbols` | Warning | Use ASCII identifiers |
| `avoid-print-in-production` | Warning | Use logging framework instead of print() |
| `avoid-throw-in-catch` | Error | Don't throw in catch blocks |
| `avoid-unawaited-futures` | Error | Always await or explicitly ignore Futures |
| `avoid-unnecessary-type-assertions` | Warning | Remove redundant `is` checks |
| `avoid-unnecessary-type-casts` | Warning | Remove redundant `as` casts |
| `avoid-unused-parameters` | Warning | Remove or use all parameters |
| `binary-expression-operand-order` | Info | Literal on the right side |
| `double-literal-format` | Info | Consistent double formatting |
| `newline-before-return` | Info | Blank line before return |
| `no-magic-numbers` | Warning | Extract constants |
| `prefer-correct-identifier-length` | Warning | Meaningful identifier names |
| `prefer-first-last` | Info | Use .first/.last instead of [0] |
| `prefer-match-file-name` | Warning | Class name should match file name |
| `prefer-named-boolean-parameters` | Warning | Named params for booleans |
| `prefer-specific-catch-type` | Warning | Use `on SpecificException` instead of `catch (e)` |
| `prefer-trailing-comma` | Info | Trailing commas in multi-line |
| `avoid-cascade-after-if-null` | Warning | Cascade after ?? is confusing |
| `avoid-collection-methods-unrelated-types` | Warning | Type-safe collection methods |
| `avoid-double-negation` | Warning | Simplify `!!` |
| `avoid-duplicate-exports` | Warning | No duplicate exports |
| `avoid-missing-enum-constant-in-map` | Warning | All enum values in map |
| `avoid-top-level-members-in-tests` | Warning | Keep test helpers scoped |

## Flutter Rules

| Rule | Severity | Description |
|---|---|---|
| `avoid-returning-widgets` | Warning | Return widget class, not method |
| `avoid-unnecessary-setstate` | Error | Don't setState unnecessarily |
| `avoid-expanded-as-spacer` | Info | Use Spacer() instead of Expanded(SizedBox()) |
| `prefer-const-constructors` | Warning | Use const where possible |
| `prefer-extracting-callbacks` | Warning | Extract callbacks from build |
| `ensure-dispose-lifecycle` | Error | Dispose controllers and FocusNodes |
| `ensure-stream-subscription-cancel` | Error | Cancel StreamSubscriptions in dispose |
| `avoid-excessive-widget-nesting` | Warning | Max widget nesting depth |
| `ensure-semantics-label` | Warning | Interactive widgets need Semantics |
| `ensure-image-semantics` | Warning | Images need semanticLabel |
| `ensure-touch-target-size` | Info | Min 48x48dp touch targets (WCAG 2.5.5) |

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

## Presets

Use `--preset` to apply a curated rule set:

```bash
falcon analyze --preset ai-generated    # 20 rules for AI code
falcon analyze --preset strict          # All rules at highest severity
falcon analyze --preset recommended     # Balanced for most projects
falcon analyze --preset flutter         # Flutter-specific rules
falcon analyze --preset performance     # Performance-focused rules
```
