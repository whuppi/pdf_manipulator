// Setup script for pdf_manipulator.
//
//   flutter pub run pdf_manipulator:setup                  # web (default)
//   flutter pub run pdf_manipulator:setup <target>        # web|android|ios|macos|linux|windows
//   flutter pub run pdf_manipulator:setup --force <target> # re-resolve (debugging)
//
// Web: resolves WASM + JS into web/pdf_manipulator/. Hash-verified —
//   stale files from a previous version are re-downloaded automatically.
// Native: runs `flutter build <target> --debug` which triggers the build
//   hook and caches the binary in Flutter's shared cache.
// --force: web skips hash check and re-downloads. Native runs
//   `flutter clean` first then rebuilds.
//
// Use `flutter pub run`, NOT `dart run` — native targets
// subprocess `flutter build` which needs flutter on PATH.

import 'dart:io';

import 'package:package_config/package_config.dart';

import '../hook/build.dart' as build;
import 'package:pdf_manipulator/src/hook/build_constants.dart';
import 'package:pdf_manipulator/src/hook/resolver.dart';
import 'package:pdf_manipulator/src/trim/capabilities.dart';
import 'package:pdf_manipulator/src/trim/detector.dart';

const _help = '''
Usage: flutter pub run pdf_manipulator:setup [--force] [target]

Targets:
  (default)        web
  web              Download/compile WASM + JS (auto stale detection)
  android          Build + cache native binary for Android
  ios              Build + cache native binary for iOS
  macos            Build + cache native binary for macOS
  linux            Build + cache native binary for Linux
  windows          Build + cache native binary for Windows

Options:
  --force          Re-resolve target (debugging)
  -h, --help       Show this help
''';

const Map<String, List<String>> _nativeBuildArgs = {
  'android': ['apk', '--debug'],
  'ios': ['ios', '--debug', '--no-codesign'],
  'macos': ['macos', '--debug'],
  'linux': ['linux', '--debug'],
  'windows': ['windows', '--debug'],
};

void main(List<String> args) async {
  if (args.contains('-h') || args.contains('--help')) {
    stdout.writeln(_help);
    return;
  }

  final force = args.contains('--force');
  final trim = args.contains('--trim');
  final targets = args.where((a) => !a.startsWith('-')).toList();

  if (targets.isEmpty) {
    targets.add('web');
  }

  for (final target in targets) {
    if (target == 'web') {
      await _setupWeb(force, trim: trim);
    } else if (_nativeBuildArgs.containsKey(target)) {
      await _setupNative(target, force);
    } else {
      stderr.writeln('Error: unknown target "$target".');
      stderr.writeln('Valid: web, ${_nativeBuildArgs.keys.join(', ')}');
      exit(1);
    }
  }
}

// ── Web ───────────────────────────────────────────────────────────

Future<void> _setupWeb(bool force, {bool trim = false}) async {
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

  // --trim: scan THIS app's source for reachable capabilities, then
  // compile a wasm carrying only those. Fail closed: unresolved files
  // mean the full default build.
  String? featuresOverride;
  if (trim) {
    stdout.writeln('=== Trim: scanning app source ===');
    final result = detectCapabilities(Directory.current.path);
    if (!result.resolved) {
      stdout.writeln(
        'trim: ${result.unresolvedPaths.length} path(s) could not be '
        'read — keeping the FULL binary (fail closed). First: '
        '${result.unresolvedPaths.first}',
      );
    } else {
      final wasmDefaults = BuildConstants.load(packageRoot).wasmFeatures;
      featuresOverride = TrimConfig.keep(
        result.keep,
      ).featuresFor(wasmDefaults, result.keep);
      final kept = result.keep.map((c) => c.wire).toList()..sort();
      stdout.writeln('trim: keeping $kept -> features [$featuresOverride]');
      stdout.writeln('trim: matched ${describeMatches(result)}');
    }
  }

  stdout.writeln('=== Web assets (v$version) ===');
  final destDir = Directory('web/pdf_manipulator');
  final count = await build.resolveWeb(
    wasmFeaturesOverride: featuresOverride,
    packageRoot: packageRoot,
    version: version,
    destDir: destDir,
    force: force,
  );
  stdout.writeln(
    count > 0
        ? '$count file(s) installed to ${destDir.path}/'
        : 'All web assets up to date.',
  );
}

// ── Native ────────────────────────────────────────────────────────

// 'flutter' on PATH is safe here — this script runs via
// `flutter pub run`, so the invoking flutter (bare or FVM)
// adds itself to PATH for child processes.
Future<void> _setupNative(String target, bool force) async {
  final buildArgs = _nativeBuildArgs[target]!;

  stdout.writeln('=== Native ($target) ===');

  if (force) {
    stdout.writeln('  flutter clean');
    final clean = await Process.start('flutter', [
      'clean',
    ], mode: ProcessStartMode.inheritStdio);
    await clean.exitCode;
  }

  stdout.writeln('  flutter build ${buildArgs.join(' ')}');
  final process = await Process.start('flutter', [
    'build',
    ...buildArgs,
  ], mode: ProcessStartMode.inheritStdio);

  final exitCode = await process.exitCode;
  if (exitCode != 0) {
    stderr.writeln('  Build failed (exit $exitCode).');
    exit(exitCode);
  }
  stdout.writeln('  Native binary cached for $target.');
}
