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
// There are no config flags. Engine config (keep, build) lives in the
// app's pubspec under `hooks: user_defines: pdf_manipulator:` — the SAME
// block the native build hook reads. Web reads it here too, so the command
// never changes and there is one place to configure both.
//
// Use `flutter pub run`, NOT `dart run` — native targets
// subprocess `flutter build` which needs flutter on PATH.

import 'dart:io';
import 'dart:isolate';

import 'package:package_config/package_config.dart';

import '../hook/build.dart' as build;
import 'package:pdf_manipulator/src/hook/app_root.dart';
import 'package:pdf_manipulator/src/hook/build_constants.dart';
import 'package:pdf_manipulator/src/hook/keep_plan.dart';
import 'package:pdf_manipulator/src/hook/pdf_config.dart';
import 'package:pdf_manipulator/src/hook/resolver.dart';
import 'package:pdf_manipulator/src/hook/user_defines.dart';
import 'package:pdf_manipulator/src/keep/capabilities.dart' show PdfConfigError;

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
  --force          Re-resolve target (ignore cache / clean rebuild)
  -h, --help       Show this help

Engine config is read from your pubspec.yaml — the same keys for web and
native, so this command never needs flags:

  hooks:
    user_defines:
      pdf_manipulator:
        keep: [render]   # or `keep: auto` to scan your app's source;
                         # absent = every capability
        build: speed     # speed (default) | size | debug

Native targets pick this block up automatically during `flutter build`.
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
  final targets = args.where((a) => !a.startsWith('-')).toList();

  if (targets.isEmpty) {
    targets.add('web');
  }

  for (final target in targets) {
    if (target == 'web') {
      await _setupWeb(force);
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

Future<void> _setupWeb(bool force) async {
  final config = await findPackageConfig(Directory.current);
  if (config == null) {
    stderr.writeln('Error: not inside a Dart/Flutter project.');
    exit(1);
  }

  // NOT Directory.current: this command is user-invoked, so the working
  // directory is wherever they were standing — run it from a subfolder and
  // the app's pubspec is not there, which silently reads as "no config at
  // all" and quietly ships the full engine. The VM knows whose package
  // resolution is running us, and that answer sits inside the app's own
  // `.dart_tool/` — the same anchor the build hook derives from, so web and
  // native cannot disagree about which directory the app is.
  final packageConfigUri = await Isolate.packageConfig;
  final appRoot = packageConfigUri == null
      ? null
      : appRootFromDartTool(packageConfigUri);
  if (appRoot == null) {
    stderr.writeln(
      'Error: could not locate your app from this command. Run it from '
      'inside your project (flutter pub run pdf_manipulator:setup).',
    );
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
  final constants = BuildConstants.load(packageRoot);

  // Read + validate the app's `hooks: user_defines: pdf_manipulator:` block
  // through the SAME parser the native hook uses — keep/detector/build are
  // honored identically for web and native. Web reads the pubspec directly,
  // so it also rejects unknown keys (a typo, a stranded option).
  final PdfManipulatorConfig cfg;
  try {
    cfg = PdfManipulatorConfig.parse(readPdfManipulatorUserDefines(appRoot));
  } on PdfConfigError catch (e) {
    stderr.writeln('Error: ${e.message}');
    exit(1);
  }

  // `keep: [...]` (explicit) or `keep: auto` (scan this app's source) —
  // resolveKeepPlan runs the SAME logic the native hook does, failing closed
  // to the full binary when the source can't be resolved.
  final plan = await resolveKeepPlan(
    keep: cfg.keep,
    detector: cfg.detector,
    defaultFeatures: constants.wasmFeatures,
    appRootCandidate: appRoot,
    scanDirs: cfg.scanDirs,
  );

  stdout.writeln('=== Web assets (v$version) ===');
  if (plan.deferToLink) {
    // record-use trims in the native link hook, which web has no equivalent
    // of — so web stays on the full binary. Explicit `keep: [...]` works on web.
    stdout.writeln(
      'detector record-use is native-only; web keeps the FULL binary. '
      'Use `keep: [...]` to shrink the web engine.',
    );
  }
  if (!cfg.build.isDefault) {
    stdout.writeln('build: ${cfg.build.wire} (compiles from source)');
  }
  if (plan.isCustom) {
    stdout.writeln('keep: features [${plan.features}]');
  }
  final destDir = Directory('web/pdf_manipulator');
  final count = await build.resolveWeb(
    wasmFeaturesOverride: plan.isCustom ? plan.features : null,
    packageRoot: packageRoot,
    version: version,
    destDir: destDir,
    force: force,
    engineBuild: cfg.build,
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
