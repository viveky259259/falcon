# fake-mounted-check

**Category:** Behavioral
**Default severity:** error
**Status:** active

Flags `if (mounted)` blocks that contain an `await` without checking `mounted`
again afterward. The first check can become stale while the Future is pending.

## Bad

```dart
if (mounted) {
  await repository.load();
  setState(() {});
}
```

## Good

```dart
await repository.load();
if (!mounted) return;
setState(() {});
```

## Notes

This is a CST-only rule and intentionally focuses on the common AI-generated
shape. Broader state-after-dispose flow analysis belongs to
`set-state-after-dispose`.
