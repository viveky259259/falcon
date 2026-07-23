# unawaited-future-in-build

**Category:** Behavioral
**Default severity:** error
**Status:** active

Flags fire-and-forget Future work started inside a Flutter `build()` method.
Build is synchronous and may run repeatedly, so async work there can duplicate
network calls, mutations, and error paths.

## Bad

```dart
Widget build(BuildContext context) {
  Future.delayed(Duration.zero).then((_) => load());
  return const Text('loading');
}
```

## Good

```dart
late final Future<void> _loadFuture;

@override
void initState() {
  super.initState();
  _loadFuture = load();
}

Widget build(BuildContext context) {
  return FutureBuilder<void>(
    future: _loadFuture,
    builder: buildBody,
  );
}
```

## Notes

Use `unawaited(...)` only for deliberate fire-and-forget work outside build,
with a clear error-handling policy.
