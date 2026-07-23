# riverpod-scope-leak

**Category:** Behavioral
**Default severity:** error
**Status:** conservative lifecycle check

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

The current implementation conservatively flags provider factory bodies that
create leak-prone resources such as stream subscriptions, stream controllers,
timers, or controllers without either using an auto-dispose provider or
registering `ref.onDispose` cleanup.

Full provider-scope checking across generated providers and imports still
requires richer provider symbol resolution.
