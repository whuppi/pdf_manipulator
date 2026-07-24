// compile.dart — compile pdf_oxide for any target (CI + local dev).
//
//   dart tool/compile.dart macos        macOS arm64 + x64
//   dart tool/compile.dart ios          iOS device + simulators
//   dart tool/compile.dart linux        Linux x64 + arm64 (cross-compile)
//   dart tool/compile.dart android      Android arm64 + arm + x64 + x86
//   dart tool/compile.dart windows      Windows x64 (+ arm64 on MSVC)
//   dart tool/compile.dart wasm         WASM + bindgen_runner
//   dart tool/compile.dart native       Auto-detect what this host builds
//   dart tool/compile.dart all          native + wasm
//   dart tool/compile.dart --features [native|wasm]   print feature flags
//
// This drives the SAME compile core the consumer build hook uses —
// `compileEngineForTarget` / `compileWasmEngine` in engine_compiler.dart.
// The hook compiles the one target Flutter asks for; this loops the
// release matrix. One cargo path, two callers (issue #183 follow-up).
//
// Feature sets come from build.json; PDF_FEATURES_NATIVE / _WASM override
// them. Output goes to COMPILE_OUTPUT_DIR (release pipeline) or
// build_output/. Run from the package root.

import 'dart:io';

import 'package:logging/logging.dart';
import 'package:path/path.dart' as p;

import 'package:pdf_manipulator/src/hook/build_constants.dart';
import 'package:pdf_manipulator/src/hook/engine_compiler.dart';

final Uri _pkgRoot = Directory.current.uri;

/// One native compile target: Rust triple, output subdir key, library file.
class _Target {
  const _Target(this.triple, this.key, this.lib);
  final String triple;
  final String key;
  final String lib;
}

const _dylib = 'libpdf_oxide.dylib';
const _staticLib = 'libpdf_oxide.a';
const _so = 'libpdf_oxide.so';
const _dll = 'pdf_oxide.dll';

const _macos = [
  _Target('aarch64-apple-darwin', 'macos-arm64', _dylib),
  _Target('x86_64-apple-darwin', 'macos-x64', _dylib),
];
const _ios = [
  _Target('aarch64-apple-ios', 'ios-arm64', _staticLib),
  _Target('aarch64-apple-ios-sim', 'ios-sim-arm64', _staticLib),
  _Target('x86_64-apple-ios', 'ios-sim-x64', _staticLib),
];
const _linuxX64 = _Target('x86_64-unknown-linux-gnu', 'linux-x64', _so);
const _linuxArm64 = _Target('aarch64-unknown-linux-gnu', 'linux-arm64', _so);
const _android = [
  _Target('aarch64-linux-android', 'android-arm64', _so),
  _Target('armv7-linux-androideabi', 'android-arm', _so),
  _Target('x86_64-linux-android', 'android-x64', _so),
  _Target('i686-linux-android', 'android-x86', _so),
];

Future<void> main(List<String> args) async {
  // --features prints ONLY the feature string on stdout (the Makefile
  // captures it); keep logging off this path.
  if (args.isNotEmpty && args.first == '--features') {
    final c = BuildConstants.load(_pkgRoot);
    switch (args.length > 1 ? args[1] : 'all') {
      case 'native':
        stdout.writeln(_nativeFeatures(c));
      case 'wasm':
        stdout.writeln(_wasmFeatures(c));
      default:
        stdout.writeln('NATIVE=${_nativeFeatures(c)}');
        stdout.writeln('WASM=${_wasmFeatures(c)}');
    }
    return;
  }

  Logger.root.onRecord.listen((r) => stderr.writeln(r.message));

  final mode = args.isEmpty ? 'native' : args.first;
  switch (mode) {
    case 'macos':
      await _compileAll(_macos);
    case 'ios':
      await _compileAll(_ios, strip: true);
    case 'linux':
      await _doLinux();
    case 'android':
      await _doAndroid();
    case 'windows':
      await _doWindows();
    case 'wasm':
      await _doWasm();
    case 'native':
      await _doNative();
    case 'all':
      await _doNative();
      await _doWasm();
    default:
      stderr.writeln(
        'usage: dart tool/compile.dart '
        '{macos|ios|linux|android|windows|wasm|native|all|'
        '--features [native|wasm]}',
      );
      exit(1);
  }
}

String _nativeFeatures(BuildConstants c) =>
    Platform.environment['PDF_FEATURES_NATIVE'] ?? c.nativeFeatures;
String _wasmFeatures(BuildConstants c) =>
    Platform.environment['PDF_FEATURES_WASM'] ?? c.wasmFeatures;

/// Output base: COMPILE_OUTPUT_DIR (release pipeline) or build_output/.
String _outBase() {
  final env = Platform.environment['COMPILE_OUTPUT_DIR'];
  final base = (env != null && env.isNotEmpty)
      ? (p.isAbsolute(env) ? env : p.join(p.fromUri(_pkgRoot), env))
      : p.join(p.fromUri(_pkgRoot), 'build_output');
  Directory(base).createSync(recursive: true);
  return base;
}

/// Compiles one native target through the shared core and drops the
/// library under the output base as `<key>/<lib>`.
Future<void> _compileOne(
  _Target t, {
  Map<String, String> environment = const {},
}) async {
  final c = BuildConstants.load(_pkgRoot);
  final outFile = File(p.join(_outBase(), t.key, t.lib));
  stderr.writeln('=== Native: ${t.triple} ===');
  await compileEngineForTarget(
    packageRoot: _pkgRoot,
    crateName: c.crate,
    targetTriple: t.triple,
    features: _nativeFeatures(c),
    // One shared build dir so cargo caches across the whole matrix.
    targetDir: p.fromUri(_pkgRoot.resolve('vendor/pdf_oxide/target')),
    libFileName: t.lib,
    outFile: outFile,
    environment: environment,
  );
  // Print the produced size (the old bash driver did `du -h` per target) —
  // handy when eyeballing a release matrix for an unexpectedly large binary.
  stderr.writeln('  → ${outFile.path} (${outFile.lengthSync()} bytes)');
}

Future<void> _compileAll(List<_Target> targets, {bool strip = false}) async {
  for (final t in targets) {
    await _compileOne(t);
    if (strip) {
      // -S drops debug symbols from the static archive (still large, but
      // smaller than the un-stripped .a). A missing or failing strip leaves a
      // valid, just-larger .a — warn, never fail the build over it (e.g. a
      // cross-compile from a host without strip).
      final archive = p.join(_outBase(), t.key, t.lib);
      try {
        final r = Process.runSync('strip', ['-S', archive]);
        if (r.exitCode != 0) {
          stderr.writeln(
            '  ⚠ strip -S failed for $archive (exit ${r.exitCode}) — '
            'shipping an unstripped archive. ${(r.stderr as String).trim()}',
          );
        }
      } on ProcessException catch (e) {
        stderr.writeln(
          '  ⚠ strip unavailable (${e.message}) — '
          'shipping an unstripped archive: $archive',
        );
      }
    }
  }
}

Future<void> _doLinux() async {
  await _compileOne(_linuxX64);
  _ensureCrossGcc();
  await _compileOne(
    _linuxArm64,
    environment: const {
      'CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER': 'aarch64-linux-gnu-gcc',
    },
  );
}

Future<void> _doAndroid() async {
  for (final t in _android) {
    final env = androidLinkerEnvFromNdkHome(t.triple);
    if (env.isEmpty) {
      // On CI a missing NDK must fail loudly — a silent skip ships zero
      // Android binaries while the job exits 0, leaving consumers with
      // download 404s.
      if (Platform.environment['CI'] != null) {
        throw StateError(
          'Android NDK not found (set ANDROID_NDK_HOME). Cannot compile '
          '${t.triple}.',
        );
      }
      stderr.writeln('  ⚠ Skipping ${t.triple} (no Android NDK found)');
      continue;
    }
    await _compileOne(t, environment: env);
  }
}

Future<void> _doWindows() async {
  if (Platform.isWindows) {
    await _compileOne(
      const _Target('x86_64-pc-windows-msvc', 'windows-x64', _dll),
    );
    await _compileOne(
      const _Target('aarch64-pc-windows-msvc', 'windows-arm64', _dll),
    );
  } else if (_has('x86_64-w64-mingw32-gcc')) {
    await _compileOne(
      const _Target('x86_64-pc-windows-gnu', 'windows-x64', _dll),
    );
  } else {
    stderr.writeln('⚠ Windows cross-compile not available on this host');
  }
}

/// Auto-detect what this host can build. Local-dev convenience; CI calls
/// the explicit target commands.
Future<void> _doNative() async {
  if (Platform.isMacOS) {
    await _compileAll(_macos);
    await _compileAll(_ios, strip: true);
  }
  if (Platform.isLinux) {
    await _compileOne(_linuxX64);
    if (_has('aarch64-linux-gnu-gcc')) {
      await _compileOne(
        _linuxArm64,
        environment: const {
          'CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER':
              'aarch64-linux-gnu-gcc',
        },
      );
    }
  }
  if (_has('x86_64-w64-mingw32-gcc')) {
    await _compileOne(
      const _Target('x86_64-pc-windows-gnu', 'windows-x64', _dll),
    );
  } else if (Platform.isWindows) {
    await _doWindows();
  }
  if (androidLinkerEnvFromNdkHome(_android.first.triple).isNotEmpty) {
    await _doAndroid();
  }
}

Future<void> _doWasm() async {
  final c = BuildConstants.load(_pkgRoot);
  final webAssets = Directory.fromUri(_pkgRoot.resolve('web_assets'));
  stderr.writeln('=== WASM: compile + bindgen ===');
  await compileWasmEngine(
    packageRoot: _pkgRoot,
    features: _wasmFeatures(c),
    outDir: webAssets,
  );
  final bg = File(p.join(webAssets.path, 'pdf_oxide_bg.wasm'));
  stderr.writeln('Binary: ${bg.lengthSync()} bytes');

  // Release pipeline mirrors the outputs into COMPILE_OUTPUT_DIR/wasm.
  final rel = Platform.environment['COMPILE_OUTPUT_DIR'];
  if (rel != null && rel.isNotEmpty) {
    final base = p.isAbsolute(rel) ? rel : p.join(p.fromUri(_pkgRoot), rel);
    final wasmOut = Directory(p.join(base, 'wasm'))
      ..createSync(recursive: true);
    for (final f in const ['pdf_oxide.js', 'pdf_oxide_bg.wasm']) {
      File(p.join(webAssets.path, f)).copySync(p.join(wasmOut.path, f));
    }
    stderr.writeln('Copied to ${wasmOut.path}');
  }
}

bool _has(String cmd) {
  try {
    final w = Platform.isWindows ? 'where' : 'which';
    return Process.runSync(w, [cmd]).exitCode == 0;
  } on ProcessException {
    return false;
  }
}

/// Ensures the aarch64 Linux cross-linker is present. Auto-installs on CI
/// (the runner is disposable); locally the dev is told the exact command.
void _ensureCrossGcc() {
  if (_has('aarch64-linux-gnu-gcc')) return;
  if (Platform.environment['CI'] != null) {
    stderr.writeln('installing gcc-aarch64-linux-gnu');
    Process.runSync('sudo', ['apt-get', 'update', '-qq']);
    final r = Process.runSync('sudo', [
      'apt-get',
      'install',
      '-y',
      '-qq',
      'gcc-aarch64-linux-gnu',
    ]);
    if (r.exitCode != 0) {
      throw StateError(
        'apt-get install gcc-aarch64-linux-gnu failed:\n'
        '${r.stderr}',
      );
    }
    return;
  }
  throw StateError(
    'Linux arm64 cross-compile needs gcc-aarch64-linux-gnu.\n'
    '  sudo apt-get install gcc-aarch64-linux-gnu',
  );
}
