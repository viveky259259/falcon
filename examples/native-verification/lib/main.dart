import 'package:flutter/material.dart';

void main() => runApp(const NativeProbeApp());

class NativeProbeApp extends StatefulWidget {
  const NativeProbeApp({super.key});

  @override
  State<NativeProbeApp> createState() => _NativeProbeState();
}

class _NativeProbeState extends State<NativeProbeApp> {
  int count = 0;

  @override
  Widget build(BuildContext context) => MaterialApp(
    home: Scaffold(
      appBar: AppBar(title: const Text('Falcon native probe')),
      body: Center(child: Text('Count: $count')),
      floatingActionButton: FloatingActionButton(
        tooltip: 'Increment',
        onPressed: () => setState(() => count++),
        child: const Icon(Icons.add),
      ),
    ),
  );
}
