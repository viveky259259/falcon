# dispose-not-called

**Category:** Behavioral
**Default severity:** error
**Status:** resolver-gated stub

This rule targets State classes and other disposable owners that create
controllers, subscriptions, focus nodes, or similar resources but do not dispose
them or do not call `super.dispose()`.

## Bad

```dart
class _ScreenState extends State<Screen> {
  final controller = TextEditingController();
}
```

## Good

```dart
class _ScreenState extends State<Screen> {
  final controller = TextEditingController();

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }
}
```

## Notes

The current implementation is registered but intentionally returns no findings
until class-hierarchy resolution lands. The existing `ensure-dispose-lifecycle`
rule covers the narrower Flutter controller case today.
