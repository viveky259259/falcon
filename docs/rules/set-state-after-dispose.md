# set-state-after-dispose

**Category:** Behavioral
**Default severity:** error
**Status:** resolver-gated stub

This rule documents the contract that `setState` must not be reachable after a
State object has been disposed. Calling `setState` on an unmounted State throws
at runtime.

## Bad

```dart
Future<void> load() async {
  await api.fetch();
  setState(() {});
}
```

## Good

```dart
Future<void> load() async {
  await api.fetch();
  if (!mounted) return;
  setState(() {});
}
```

## Notes

The current implementation is registered but intentionally returns no findings
until the resolver can identify State subclasses and track async control flow.
The active `fake-mounted-check` rule covers the most common CST-only case.
