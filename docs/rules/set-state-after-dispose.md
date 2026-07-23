# set-state-after-dispose

**Category:** Behavioral
**Default severity:** error
**Status:** resolver-backed

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

The project-aware implementation uses Falcon's resolver class index to limit
findings to known `State` subclasses. It conservatively flags `setState(...)`
after an `await` when there is no intervening `mounted`/`!mounted return` guard.

The plain per-file hook still returns no findings because it lacks resolver
context. This first slice does not attempt full Dart control-flow analysis or
symbol aliasing.
