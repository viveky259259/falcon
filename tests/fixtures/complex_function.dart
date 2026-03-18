void processData(
  String name,
  int age,
  String email,
  String phone,
  String address,
  bool isActive,
) {
  if (name.isNotEmpty) {
    if (age > 18) {
      if (email.contains('@')) {
        if (phone.length > 10) {
          if (address.isNotEmpty) {
            if (isActive) {
              print('All valid');
              for (var i = 0; i < 10; i++) {
                if (i % 2 == 0) {
                  print(i);
                }
              }
            }
          }
        }
      }
    }
  }
}

int calculateScore(int a, int b, int c) {
  var result = 0;
  if (a > 10 && b > 20) {
    result = a + b;
  } else if (a > 5 || c > 30) {
    result = a * c;
  } else {
    result = b + c;
  }

  switch (result) {
    case 0:
      return -1;
    case 42:
      return 100;
    default:
      return result;
  }
}

dynamic fetchData() {
  return null;
}

var globalCounter = 0;
var globalName = 'test';

late String lateValue;

class DataProcessor {
  void methodOne() {}
  void methodTwo() {}
  void methodThree() {}
  void methodFour() {}
  void methodFive() {}
  void methodSix() {}
  void methodSeven() {}
  void methodEight() {}
  void methodNine() {}
  void methodTen() {}
  void methodEleven() {}
  void methodTwelve() {}

  Widget buildHeader() {
    return Container();
  }
}

void compute() {
  var x = 42;
  var y = 3.14;
  var z = x * 100;
  var w = !!true;
}
