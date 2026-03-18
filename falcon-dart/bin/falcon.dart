import 'dart:io';

/// Dart wrapper for the Falcon CLI.
/// Downloads and runs the Rust-compiled Falcon binary.
void main(List<String> args) async {
  final binary = await _ensureBinary();
  final result = await Process.run(binary, args, runInShell: false);
  stdout.write(result.stdout);
  stderr.write(result.stderr);
  exit(result.exitCode);
}

Future<String> _ensureBinary() async {
  // Check if falcon is already on PATH
  final which = await Process.run('which', ['falcon']);
  if (which.exitCode == 0) {
    return which.stdout.toString().trim();
  }

  // Check cargo install location
  final home = Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'] ?? '.';
  final cargoPath = '$home/.cargo/bin/falcon';
  if (await File(cargoPath).exists()) {
    return cargoPath;
  }

  // Prompt to install
  stderr.writeln('Falcon CLI not found. Install with:');
  stderr.writeln('  cargo install --git https://github.com/viveky259259/falcon');
  stderr.writeln('');
  stderr.writeln('Or download from: https://github.com/viveky259259/falcon/releases');
  exit(1);
}
