// Fixture for v0.3 rule tests

// avoid-throw-in-catch-block
void throwInCatch() {
  try {
    riskyOperation();
  } catch (e) {
    throw Exception('Failed: $e'); // bad: should use rethrow
  }
}

// avoid-duplicate-exports (won't trigger here, needs export statements)

// prefer-first-last
void firstLastUsage() {
  final list = [1, 2, 3];
  final first = list[0]; // bad: use .first
  final last = list[list.length - 1]; // bad: use .last
}

// double-literal-format
void doubleLiterals() {
  final a = .5; // bad: use 0.5
  final b = 1.0;
  final c = 3.14;
}

// newline-before-return
int noNewlineBeforeReturn(int x) {
  final y = x * 2;
  return y; // bad: should have blank line before
}

// binary-expression-operand-order
void yodaCondition(int x) {
  if (0 == x) { // bad: literal on left
    print('zero');
  }
}

// avoid-unnecessary-type-assertions
void typeChecks(Object obj) {
  if (obj is Object) { // bad: always true
    print('always');
  }
}

// Equatable test
class MyValue {
  final int x;
  MyValue(this.x);

  @override
  bool operator ==(Object other) => other is MyValue && other.x == x;

  @override
  int get hashCode => x.hashCode;
}

// avoid-unused-parameters
void unusedParam(int used, int notUsed) {
  print(used);
}

// BLoC test
class MyBloc extends Bloc<MyEvent, MyState> {
  void doSomething() { // bad: public method on Bloc
    emit(MyState());
  }
}

// Equatable mutable field
class BadEquatable extends Equatable {
  String name; // bad: should be final in Equatable

  BadEquatable(this.name);

  @override
  List<Object> get props => [name];
}
