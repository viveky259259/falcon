import 'package:flutter/material.dart';

class BadPatterns extends StatefulWidget {
  @override
  State<BadPatterns> createState() => _BadPatternsState();
}

class _BadPatternsState extends State<BadPatterns> {
  late String value;

  @override
  void initState() {
    super.initState();
    setState(() {
      value = 'initialized';
    });
  }

  Widget _buildButton() {
    return Container();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Expanded(
          child: Container(),
        ),
        ElevatedButton(
          onPressed: () {
            setState(() {
              value = 'updated';
            });
            if (value.isNotEmpty) {
              print('value: $value');
              if (value.length > 5) {
                print('long value');
              }
            }
          },
          child: Text(value),
        ),
        SizedBox(height: 16),
        Icon(Icons.home),
        Text('Hello World'),
      ],
    );
  }
}
