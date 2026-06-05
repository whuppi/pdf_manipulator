// Thin CLI wrapper over hook/build.dart's resolveNative() and resolveWeb().
//
// Usage:
//   flutter pub run pdf_manipulator:setup           # web (default)
//   flutter pub run pdf_manipulator:setup --native  # native for current OS
//   flutter pub run pdf_manipulator:setup --all     # both
//   flutter pub run pdf_manipulator:setup --force   # re-resolve everything
//
// Use `flutter pub run`, NOT `dart run`.

import 'dart:io';

import 'package:package_config/package_config.dart';

import '../hook/build.dart' as build;
import 'package:pdf_manipulator/src/hook/resolver.dart';

const _help = '''
Usage: flutter pub run pdf_manipulator:setup [options]

Resolves pdf_manipulator assets (download, compile, or cache).

Targets:
  (default)    Web assets only (WASM + JS glue)
  --native     Native binary for current platform only
  --all        Both web and native

Options:
  --force      Re-resolve even if files exist and hashes match
  -h, --help   Show this help
''';

void main(List<String> args) async {
  if (args.contains('-h') || args.contains('--help')) {
    stdout.writeln(_help);
    return;
  }

  final force = args.contains('--force');
  final doNative = args.contains('--native') || args.contains('--all');
  final doWeb = args.contains('--all') || !args.contains('--native');

  final config = await findPackageConfig(Directory.current);
  if (config == null) {
    stderr.writeln('Error: not inside a Dart/Flutter project.');
    exit(1);
  }

  final pkg = config.packages
      .where((p) => p.name == 'pdf_manipulator')
      .firstOrNull;
  if (pkg == null) {
    stderr.writeln('Error: pdf_manipulator not in pubspec.yaml.');
    exit(1);
  }

  final packageRoot = pkg.root;
  final version = readVersion(packageRoot);

  if (doWeb) {
    stdout.writeln('=== Web assets (v$version) ===');
    final destDir = Directory('web/pdf_manipulator');
    final count = await build.resolveWeb(
      packageRoot: packageRoot,
      version: version,
      destDir: destDir,
      force: force,
    );
    stdout.writeln(count > 0
        ? '$count file(s) installed to ${destDir.path}/'
        : 'All web assets up to date.');
  }

  if (doNative) {
    stdout.writeln('=== Native binary (v$version) ===');
    final platform = build.currentPlatformKey();
    final libFileName = build.currentLibFileName();
    final destDir = Directory('.dart_tool/pdf_manipulator');
    if (!destDir.existsSync()) destDir.createSync(recursive: true);

    await build.resolveNative(
      packageRoot: packageRoot,
      version: version,
      platform: platform,
      libFileName: libFileName,
      dest: File('${destDir.path}/$libFileName'),
      force: force,
    );
    stdout.writeln('  $libFileName — resolved');
  }
}
