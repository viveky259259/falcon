# dispose-not-called

**Category:** Behavioral
**Default severity:** error
**Status:** resolver-backed

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

The rule uses Falcon's resolver index during full-project analysis, so it can
identify direct and transitive `State` subclasses. The resolver currently uses
simple class names; import-aware duplicate-name handling is a future resolver
slice.
