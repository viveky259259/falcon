import 'dart:io';

import 'package:falcon_lint/falcon.dart';

Future<void> main(List<String> arguments) async {
  final binary = await FalconBinary.resolve();

  if (binary == null) {
    stderr.writeln('Error: Could not find or download the falcon binary.');
    stderr.writeln('');
    stderr.writeln('Please install falcon manually:');
    stderr.writeln('  cargo install falcon');
    stderr.writeln('');
    stderr.writeln('Or download a prebuilt binary from:');
    stderr.writeln('  https://github.com/falcon-lint/falcon/releases');
    exit(1);
  }

  final result = await Process.run(
    binary,
    arguments,
    runInShell: false,
  );

  stdout.write(result.stdout);
  stderr.write(result.stderr);
  exit(result.exitCode);
}
