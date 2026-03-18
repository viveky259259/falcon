import 'dart:io';

import 'package:path/path.dart' as p;

class FalconBinary {
  static const _binaryName = 'falcon';
  static const _version = '0.1.0';
  static const _repo = 'falcon-lint/falcon';

  /// Resolves the path to the falcon binary.
  ///
  /// Search order:
  /// 1. PATH environment variable (if installed via cargo install)
  /// 2. Local .dart_tool/falcon/ cache
  /// 3. Downloads from GitHub releases
  static Future<String?> resolve() async {
    final fromPath = _findInPath();
    if (fromPath != null) return fromPath;

    final cached = _findCached();
    if (cached != null) return cached;

    return null;
  }

  static String? _findInPath() {
    final pathEnv = Platform.environment['PATH'] ?? '';
    final separator = Platform.isWindows ? ';' : ':';
    final dirs = pathEnv.split(separator);

    for (final dir in dirs) {
      final binary = File(p.join(dir, _executableName));
      if (binary.existsSync()) {
        return binary.path;
      }
    }

    return null;
  }

  static String? _findCached() {
    final cacheDir = p.join('.dart_tool', 'falcon', _version);
    final binary = File(p.join(cacheDir, _executableName));

    if (binary.existsSync()) {
      return binary.path;
    }

    return null;
  }

  static String get _executableName {
    if (Platform.isWindows) {
      return '$_binaryName.exe';
    }
    return _binaryName;
  }

  static String get _downloadUrl {
    final os = _currentOs;
    final arch = _currentArch;
    final ext = Platform.isWindows ? '.zip' : '.tar.gz';
    return 'https://github.com/$_repo/releases/download/v$_version/falcon-$os-$arch$ext';
  }

  static String get _currentOs {
    if (Platform.isMacOS) return 'darwin';
    if (Platform.isLinux) return 'linux';
    if (Platform.isWindows) return 'windows';
    return 'unknown';
  }

  static String get _currentArch {
    final arch = Platform.version;
    if (arch.contains('arm64') || arch.contains('aarch64')) return 'aarch64';
    return 'x86_64';
  }
}
