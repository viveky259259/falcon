import 'dart:io';

/// Dart wrapper for the Falcon CLI: finds the Rust-built `falcon` binary and
/// runs it with the same arguments, terminal and exit code.
///
/// It does not download Falcon. Install the binary first (see the README).
Future<void> main(List<String> args) async {
  final binary = await findFalconBinary();
  if (binary == null) {
    stderr
      ..writeln('Falcon CLI not found. Install it with:')
      ..writeln(
          '  curl -fsSL https://raw.githubusercontent.com/viveky259259/falcon/master/scripts/install.sh | sh')
      ..writeln('or, with Rust installed:')
      ..writeln('  cargo install falcon-flutter')
      ..writeln('')
      ..writeln('Set FALCON_BIN to use a binary somewhere else.');
    exit(1);
  }
  final process =
      await Process.start(binary, args, mode: ProcessStartMode.inheritStdio);
  exit(await process.exitCode);
}

/// The Falcon binary to run: `$FALCON_BIN`, then the install script's and
/// cargo's locations, then `PATH`. This package's own `falcon` shim (in the
/// pub cache) is skipped, or the wrapper would start itself.
Future<String?> findFalconBinary() async {
  final env = Platform.environment;
  final override = env['FALCON_BIN'];
  if (override != null && override.isNotEmpty) {
    return override;
  }
  final exe = Platform.isWindows ? 'falcon.exe' : 'falcon';
  final sep = Platform.pathSeparator;
  final home = env['HOME'] ?? env['USERPROFILE'];
  final candidates = <String>[
    if (env['FALCON_INSTALL_DIR'] != null)
      '${env['FALCON_INSTALL_DIR']}$sep$exe',
    if (home != null) '$home$sep.local${sep}bin$sep$exe',
    if (home != null) '$home$sep.cargo${sep}bin$sep$exe',
    for (final dir in (env['PATH'] ?? '').split(Platform.isWindows ? ';' : ':'))
      if (dir.isNotEmpty) '$dir$sep$exe',
  ];
  for (final path in candidates) {
    if (isPubShim(path)) continue;
    if (await File(path).exists()) return path;
  }
  return null;
}

/// Whether `path` is inside the pub cache, where `dart pub global activate`
/// puts this package's own `falcon` command.
bool isPubShim(String path) {
  final normalized = path.replaceAll('\\', '/');
  final pubCache = Platform.environment['PUB_CACHE']?.replaceAll('\\', '/');
  return normalized.contains('/.pub-cache/') ||
      normalized.contains('/Pub/Cache/') ||
      (pubCache != null &&
          pubCache.isNotEmpty &&
          normalized.startsWith(pubCache));
}
