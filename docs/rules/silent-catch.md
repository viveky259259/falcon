# silent-catch

**Category:** Behavioral
**Default severity:** error
**Status:** active

Flags empty or comment-only catch blocks. Silent catches hide runtime failures
and commonly appear when generated code is patched to pass tests.

## Bad

```dart
try {
  await save();
} catch (_) {
  // ignore
}
```

## Good

```dart
try {
  await save();
} catch (error, stackTrace) {
  logger.error('save failed', error, stackTrace);
  rethrow;
}
```

## Notes

Intentional ignores should be explicit at the call site or handled by a named
helper/policy. An empty catch body is not enough documentation.
