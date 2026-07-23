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
// There are no config flags. Engine config (profile, trim) lives in the
// app's pubspec under `hooks: user_defines: pdf_manipulator:` — the SAME
// block the native build hook reads. Web reads it here too, so the command
// never changes and there is one place to configure both.
//
// Use `flutter pub run`, NOT `dart run` — native targets
// subprocess `flutter build` which needs flutter on PATH.

import 'dart:io';

import 'package:package_config/package_config.dart';

import '../hook/build.dart' as build;
import 'package:pdf_manipulator/src/hook/build_constants.dart';
import 'package:pdf_manipulator/src/hook/build_profile.dart';
import 'package:pdf_manipulator/src/hook/resolver.dart';
import 'package:pdf_manipulator/src/hook/trim_plan.dart';
import 'package:pdf_manipulator/src/hook/user_defines.dart';

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
        profile: release        # release (default) | small | debug
        trim: {keep: [render]}  # or `trim: auto` to scan your app's source

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

  // Read the app's `hooks: user_defines: pdf_manipulator:` block — the same
  // config the native build hook receives. `profile` and `trim` are honored
  // identically for web and native; the parsers are shared code.
  final defines = readPdfManipulatorUserDefines(Directory.current.path);
  final EngineProfile profile;
  try {
    profile = EngineProfile.parse(defines['profile']);
  } on ArgumentError catch (e) {
    stderr.writeln('Error: ${e.message}');
    exit(1);
  }

  // `trim: {keep: [...]}` (explicit) or `trim: auto` (scan this app's
  // source) — resolveTrimPlan runs the SAME logic the native hook does,
  // failing closed to the full binary when the source can't be resolved.
  final plan = await resolveTrimPlan(
    trimDefine: defines['trim'],
    detectorDefine: defines['trim-detector'],
    defaultFeatures: constants.wasmFeatures,
    appRootCandidate: Directory.current.path,
  );

  stdout.writeln('=== Web assets (v$version) ===');
  if (plan.deferToLink) {
    // record-use trims in the native link hook, which web has no equivalent
    // of — so web stays on the full binary. Explicit trim keeps work on web.
    stdout.writeln(
      'trim-detector record-use is native-only; web keeps the FULL binary. '
      'Use `trim: {keep: [...]}` to trim the web engine.',
    );
  }
  if (!profile.isDefault) {
    stdout.writeln('profile: ${profile.wire} (compiles from source)');
  }
  if (plan.isCustom) {
    stdout.writeln('trim: features [${plan.features}]');
  }
  final destDir = Directory('web/pdf_manipulator');
  final count = await build.resolveWeb(
    wasmFeaturesOverride: plan.isCustom ? plan.features : null,
    packageRoot: packageRoot,
    version: version,
    destDir: destDir,
    force: force,
    profile: profile,
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
