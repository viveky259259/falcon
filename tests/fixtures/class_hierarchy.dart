import 'package:flutter/material.dart';

abstract class Animal {
  String name;
  int age;

  Animal(this.name, this.age);

  void speak();
  void eat() {
    print('$name is eating');
  }
  void sleep() {
    print('$name is sleeping');
  }
}

class Dog extends Animal implements Comparable<Dog> {
  String breed;

  Dog(String name, int age, this.breed) : super(name, age);

  @override
  void speak() {
    print('$name says Woof!');
  }

  @override
  int compareTo(Dog other) {
    return name.compareTo(other.name);
  }

  void fetch() {
    print('$name fetches the ball');
  }

  void rollOver() {
    print('$name rolls over');
  }

  void _privateMethod() {
    print('private');
  }
}

class Cat extends Animal {
  bool isIndoor;

  Cat(String name, int age, this.isIndoor) : super(name, age);

  @override
  void speak() {
    print('$name says Meow!');
  }

  void purr() {
    print('$name purrs');
  }
}

class ServiceLocator {
  final DatabaseService db;
  final ApiClient api;
  final Logger logger;
  final CacheManager cache;

  ServiceLocator(this.db, this.api, this.logger, this.cache);

  void initialize() {
    db.connect();
    api.setup();
    logger.init();
    cache.warmUp();
  }

  void dispose() {
    db.disconnect();
    api.teardown();
  }
}

class DatabaseService {
  void connect() {}
  void disconnect() {}
  void query(String sql) {}
}

class ApiClient {
  void setup() {}
  void teardown() {}
  void get(String url) {}
  void post(String url, Map data) {}
}

class Logger {
  void init() {}
  void log(String message) {}
}

class CacheManager {
  void warmUp() {}
  void invalidate() {}
}

class CounterWidget extends StatefulWidget {
  @override
  State<CounterWidget> createState() => _CounterWidgetState();
}

class _CounterWidgetState extends State<CounterWidget> {
  int _count = 0;

  void _increment() {
    setState(() {
      _count++;
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text('Counter'),
      ),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Text('Count: $_count'),
            SizedBox(height: 16),
            ElevatedButton(
              onPressed: _increment,
              child: Icon(Icons.add),
            ),
          ],
        ),
      ),
    );
  }
}
