# riverpod-scope-leak

**Category:** Behavioral
**Default severity:** error
**Status:** resolver-gated stub

This rule targets Riverpod providers, subscriptions, or containers that escape
their intended `ProviderScope` or lifecycle. These leaks can retain stale state,
listeners, or rebuild paths after the owning widget is gone.

## Bad

```dart
final subscriptionProvider = Provider((ref) {
  return stream.listen((event) {});
});
```

## Good

```dart
final subscriptionProvider = AutoDisposeProvider((ref) {
  final sub = stream.listen((event) {});
  ref.onDispose(sub.cancel);
  return sub;
});
```

## Notes

The current implementation is registered but intentionally returns no findings
until provider symbol resolution lands.
