// Everything needed to turn a hook's CodeConfig into a compiled engine:
// the CodeConfig → target mappings and the cargo invocation itself.
// Shared by hook/build.dart (default + scan-trimmed builds) and
// hook/link.dart (RecordUse-trimmed release builds) — ONE compile path,
// two callers. Change compile behavior here, never in a hook.

import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:logging/logging.dart';
import 'package:path/path.dart' as p;

final _log = Logger('pdf_manipulator:engine_compiler');

/// Rust target triple for [code]'s OS + architecture.
String targetTripleFor(CodeConfig code) {
  if (code.targetOS == OS.iOS &&
      code.targetArchitecture == Architecture.arm64 &&
      code.iOS.targetSdk == IOSSdk.iPhoneSimulator) {
    return 'aarch64-apple-ios-sim';
  }
  return switch ((code.targetOS, code.targetArchitecture)) {
    (OS.android, Architecture.arm) => 'armv7-linux-androideabi',
    (OS.android, Architecture.arm64) => 'aarch64-linux-android',
    (OS.android, Architecture.ia32) => 'i686-linux-android',
    (OS.android, Architecture.x64) => 'x86_64-linux-android',
    (OS.iOS, Architecture.arm64) => 'aarch64-apple-ios',
    (OS.iOS, Architecture.x64) => 'x86_64-apple-ios',
    (OS.linux, Architecture.arm64) => 'aarch64-unknown-linux-gnu',
    (OS.linux, Architecture.x64) => 'x86_64-unknown-linux-gnu',
    (OS.macOS, Architecture.arm64) => 'aarch64-apple-darwin',
    (OS.macOS, Architecture.x64) => 'x86_64-apple-darwin',
    (OS.windows, Architecture.arm64) => 'aarch64-pc-windows-msvc',
    (OS.windows, Architecture.x64) => 'x86_64-pc-windows-msvc',
    (_, _) => throw UnsupportedError(
      'Unsupported: ${code.targetOS} ${code.targetArchitecture}',
    ),
  };
}

/// GitHub Release asset key for [code]'s OS + architecture.
String targetKeyFor(CodeConfig code) {
  if (code.targetOS == OS.iOS &&
      code.targetArchitecture == Architecture.arm64 &&
      code.iOS.targetSdk == IOSSdk.iPhoneSimulator) {
    return 'ios-sim-arm64';
  }
  return switch ((code.targetOS, code.targetArchitecture)) {
    (OS.android, Architecture.arm) => 'android-arm',
    (OS.android, Architecture.arm64) => 'android-arm64',
    (OS.android, Architecture.ia32) => 'android-x86',
    (OS.android, Architecture.x64) => 'android-x64',
    (OS.iOS, Architecture.arm64) => 'ios-arm64',
    (OS.iOS, Architecture.x64) => 'ios-sim-x64',
    (OS.linux, Architecture.arm64) => 'linux-arm64',
    (OS.linux, Architecture.x64) => 'linux-x64',
    (OS.macOS, Architecture.arm64) => 'macos-arm64',
    (OS.macOS, Architecture.x64) => 'macos-x64',
    (OS.windows, Architecture.arm64) => 'windows-arm64',
    (OS.windows, Architecture.x64) => 'windows-x64',
    (_, _) => throw UnsupportedError(
      'Unsupported: ${code.targetOS} ${code.targetArchitecture}',
    ),
  };
}

/// The [LinkMode] Flutter asked for.
LinkMode linkModeFor(CodeConfig code) {
  return switch (code.linkModePreference) {
    LinkModePreference.dynamic ||
    LinkModePreference.preferDynamic => DynamicLoadingBundled(),
    LinkModePreference.static ||
    LinkModePreference.preferStatic => StaticLinking(),
    _ => DynamicLoadingBundled(),
  };
}

/// Cargo environment for Android cross-compiles: points cargo's linker at
/// the NDK clang driver Flutter provides. Empty for every other target.
Map<String, String> androidLinkerEnv(CodeConfig code, String targetTriple) {
  if (code.targetOS != OS.android) return const {};
  final cc = code.cCompiler;
  if (cc == null) return const {};

  final compilerDir = p.dirname(p.fromUri(cc.compiler));
  final ndkTriple = targetTriple == 'armv7-linux-androideabi'
      ? 'armv7a-linux-androideabi'
      : targetTriple;
  // The NDK per-API clang driver is a `.cmd` batch wrapper on Windows
  // hosts (e.g. aarch64-linux-android21-clang.cmd); passing the bare name
  // makes cargo fail with "could not exec the linker ... program not
  // found". Append the host executable extension so Android cross-compiles
  // link from a Windows host as well as Linux/macOS. Platform.isWindows
  // here is the BUILD host (which runs cargo), not the Android target.
  final clangExt = Platform.isWindows ? '.cmd' : '';
  final linker = p.join(compilerDir, '${ndkTriple}21-clang$clangExt');
  final ar = p.join(compilerDir, 'llvm-ar');
  final envKey =
      'CARGO_TARGET_${targetTriple.toUpperCase().replaceAll('-', '_')}';
  _log.info('NDK linker: $linker');
  return {'${envKey}_LINKER': linker, '${envKey}_AR': ar};
}

/// Ensures the Rust target [targetTriple] is installed. Probes rustc's
/// target-libdir first — a target installed by ANY toolchain manager
/// (rustup, distro package, nix) has one — and only falls back to
/// rustup when it exists; a rustup-less toolchain missing the target
/// gets instructions instead of "rustup: command not found".
void ensureRustTarget(String targetTriple) {
  try {
    // target-libdir prints the WOULD-BE path (exit 0) even for
    // uninstalled targets — only the directory existing proves it.
    final probe = Process.runSync('rustc', [
      '--print',
      'target-libdir',
      '--target',
      targetTriple,
    ]);
    final libdir = (probe.stdout as String).trim();
    if (probe.exitCode == 0 &&
        libdir.isNotEmpty &&
        Directory(libdir).existsSync()) {
      return;
    }
  } on ProcessException {
    // rustc missing entirely — the cargo probe above (or below at the
    // call site) already produced the install-Rust instruction.
  }
  final ProcessResult add;
  try {
    _log.info('installing Rust target: $targetTriple');
    add = Process.runSync('rustup', ['target', 'add', targetTriple]);
  } on ProcessException {
    throw StateError(
      'Rust target $targetTriple is not installed and rustup is not '
      'available to add it. Install the target through your Rust '
      "toolchain's own manager, or install rustup: https://rustup.rs",
    );
  }
  if (add.exitCode != 0) {
    throw StateError(
      'rustup target add $targetTriple failed (exit ${add.exitCode}).\n'
      'stderr: ${add.stderr}',
    );
  }
}

/// Strips Xcode Developer PATH injections that break cargo on macOS.
/// Mutates [env]; a no-op on other platforms. Every cargo invocation in
/// the hooks routes its environment through here.
void stripXcodeFromPath(Map<String, String> env) {
  // Prefer a caller-supplied PATH — filtering must not clobber it with
  // the host's.
  final hostPath = env['PATH'] ?? Platform.environment['PATH'];
  if (Platform.isMacOS && hostPath != null) {
    env['PATH'] = hostPath
        .split(':')
        .where((e) => !e.contains('Contents/Developer/'))
        .join(':');
  }
}

/// The engine's pinned Rust version, read from the single source of
/// truth for every version pin: `tool/versions.env`'s `RUST_VERSION`.
/// CI installs exactly this and the gate below enforces it, so one line
/// drives both. Returns null if the file or key is absent (the caller
/// keeps going rather than block on a parse quirk).
String? requiredRustVersion(Uri packageRoot) {
  final env = File.fromUri(packageRoot.resolve('tool/versions.env'));
  if (!env.existsSync()) return null;
  final m = RegExp(
    r'^\s*RUST_VERSION\s*=\s*"?([^"\s]+)"?',
    multiLine: true,
  ).firstMatch(env.readAsStringSync());
  return m?.group(1);
}

/// Checks the installed cargo against the pinned [requiredRustVersion]:
///   below → throw (a too-old cargo can't even parse the engine's
///     manifests — the failure would otherwise be a cryptic
///     `feature edition2024 is required` from deep in cargo; issue #183);
///   equal → proceed silently (the pinned, CI-tested version);
///   above → proceed with a warning (newer Rust almost always works, but
///     this build isn't the pinned config, so a surprising Rust change
///     has an explanation).
/// No-op if cargo or the pin can't be read — those are handled at the
/// compile call itself.
void ensureCargoVersion(Uri packageRoot) {
  final required = requiredRustVersion(packageRoot);
  if (required == null) return;

  final ProcessResult probe;
  try {
    probe = Process.runSync('cargo', ['--version']);
  } on ProcessException {
    return; // absence is reported by the compile step's own check
  }
  if (probe.exitCode != 0) return;

  // "cargo 1.84.0 (66221abde 2024-11-19)" → 1.84.0
  final token = (probe.stdout as String).trim().split(RegExp(r'\s+'));
  if (token.length < 2) return;
  final have = _semver(token[1]);
  final want = _semver(required);
  if (have == null || want == null) return;

  final cmp = _cmpSemver(have, want);
  if (cmp < 0) {
    // Two clearly-labelled options, each command on its own line so it is
    // obvious what to copy. rustup is one cross-platform tool — the exact
    // same commands run on macOS, Linux, and Windows.
    throw StateError(
      "pdf_manipulator's PDF engine needs Rust $required or newer, but you "
      'have ${token[1]}.\n'
      '\n'
      'Update Rust to the latest, then build again:\n'
      '    rustup update\n'
      '\n'
      'Or install exactly $required:\n'
      '    rustup toolchain install $required && rustup default $required\n'
      '\n'
      '(The same rustup commands work on macOS, Linux, and Windows.)',
    );
  }
  if (cmp > 0) {
    _log.warning(
      'Rust ${token[1]} is newer than $required, the version this engine is '
      'built and tested with — it should work. If a Rust change ever breaks '
      'the build, switch to the tested version and build again:\n'
      '    rustup toolchain install $required && rustup default $required',
    );
  }
}

List<int>? _semver(String v) {
  final parts = v.split('.');
  final out = <int>[];
  for (final p in parts) {
    final n = int.tryParse(RegExp(r'^\d+').firstMatch(p)?.group(0) ?? '');
    if (n == null) return null;
    out.add(n);
  }
  return out.isEmpty ? null : out;
}

int _cmpSemver(List<int> a, List<int> b) {
  for (var i = 0; i < a.length || i < b.length; i++) {
    final x = i < a.length ? a[i] : 0;
    final y = i < b.length ? b[i] : 0;
    if (x != y) return x - y;
  }
  return 0;
}

/// Android cross-compile linker env resolved from `ANDROID_NDK_HOME` (or
/// the newest NDK an Android Studio install left under macOS's default
/// path) — for callers WITHOUT a Flutter `BuildInput` (the standalone CI
/// compile driver). The consumer build hook uses [androidLinkerEnv]
/// instead, which reads the exact NDK path Flutter provides. Both produce
/// the same `CARGO_TARGET_<triple>_LINKER` shape, so they feed the one
/// [compileEngineForTarget] core identically. Returns empty when no NDK
/// is found (the cargo build then fails with a clear linker error).
Map<String, String> androidLinkerEnvFromNdkHome(String targetTriple) {
  final ndkHome =
      Platform.environment['ANDROID_NDK_HOME'] ??
      _latestVersionSubdir(
        '${Platform.environment['HOME']}/Library/Android/sdk/ndk',
      );
  if (ndkHome == null) return const {};

  final prebuilt = p.join(ndkHome, 'toolchains', 'llvm', 'prebuilt');
  String? hostDir;
  for (final h in ['darwin-x86_64', 'linux-x86_64']) {
    if (Directory(p.join(prebuilt, h)).existsSync()) {
      hostDir = p.join(prebuilt, h);
      break;
    }
  }
  if (hostDir == null) return const {};

  final clangPrefix = targetTriple == 'armv7-linux-androideabi'
      ? 'armv7a-linux-androideabi'
      : targetTriple;
  final key =
      'CARGO_TARGET_${targetTriple.toUpperCase().replaceAll('-', '_')}_LINKER';
  return {key: p.join(hostDir, 'bin', '${clangPrefix}21-clang')};
}

/// Newest version-named subdirectory of [base] (e.g. an NDK dir), or null.
/// Sorts by dotted numeric fields so 25.1.2 beats 9.0.0.
String? _latestVersionSubdir(String base) {
  final dir = Directory(base);
  if (!dir.existsSync()) return null;
  final names = dir
      .listSync()
      .whereType<Directory>()
      .map((d) => p.basename(d.path))
      .where((n) => RegExp(r'^\d').hasMatch(n))
      .toList();
  if (names.isEmpty) return null;
  names.sort((a, b) {
    final av = _semver(a) ?? const [0];
    final bv = _semver(b) ?? const [0];
    return _cmpSemver(av, bv);
  });
  return p.join(base, names.last);
}

/// Compiles the engine to wasm32 and post-processes it (wasm-bindgen +
/// wasm-opt) via the workspace's `bindgen_runner` crate, writing
/// `pdf_oxide.js` + `pdf_oxide_bg.wasm` into [outDir]. Shared by the
/// consumer web setup (`hook/build.dart`) and the CI compile driver
/// (`tool/compile.dart`) — one wasm compile path, two callers.
Future<void> compileWasmEngine({
  required Uri packageRoot,
  required String features,
  required Directory outDir,
}) async {
  ensureCargoVersion(packageRoot);
  final manifest = p.fromUri(
    packageRoot.resolve('vendor/pdf_oxide/Cargo.toml'),
  );

  try {
    Process.runSync('cargo', ['--version']);
  } on ProcessException {
    throw StateError(
      'This build needs to compile the PDF engine WASM from source, but '
      'Rust is not installed.\n'
      'Install it from https://rustup.rs (macOS, Linux, and Windows), '
      'then rerun setup.',
    );
  }
  ensureRustTarget('wasm32-unknown-unknown');
  outDir.createSync(recursive: true);

  final env = <String, String>{...Platform.environment};
  stripXcodeFromPath(env);

  _log.info('compiling WASM from source (features: $features)');
  final build = await Process.run('cargo', [
    'build',
    '--manifest-path',
    manifest,
    '--lib',
    '--release',
    '--target',
    'wasm32-unknown-unknown',
    '--no-default-features',
    '--features',
    features,
  ], environment: env);
  if (build.exitCode != 0) {
    throw StateError(
      'WASM engine compile failed (exit ${build.exitCode}).\n'
      'stderr: ${build.stderr}',
    );
  }

  // First run also compiles the runner itself (binaryen builds from
  // source — a few minutes once; cargo caches it after).
  final rawWasm = p.fromUri(
    packageRoot.resolve(
      'vendor/pdf_oxide/target/wasm32-unknown-unknown/release/pdf_oxide.wasm',
    ),
  );
  final bindgen = await Process.run('cargo', [
    'run',
    '--manifest-path',
    manifest,
    '--release',
    '-p',
    'bindgen_runner',
    '--',
    rawWasm,
    outDir.path,
  ], environment: env);
  if (bindgen.exitCode != 0) {
    throw StateError(
      'WASM post-processing (wasm-bindgen + wasm-opt) failed '
      '(exit ${bindgen.exitCode}).\n'
      'stderr: ${bindgen.stderr}',
    );
  }
}

/// Compiles the engine crate for [targetTriple] with [features] and copies
/// the produced library to [outFile]. Installs the Rust target when it's
/// missing (fresh CI or formatted laptop).
Future<void> compileEngineForTarget({
  required Uri packageRoot,
  required String crateName,
  required String targetTriple,
  required String features,
  required String targetDir,
  required String libFileName,
  required File outFile,
  Map<String, String> environment = const {},
}) async {
  final manifestPath = p.fromUri(
    packageRoot.resolve('vendor/pdf_oxide/Cargo.toml'),
  );

  _log.info('compiling from source for $targetTriple (features: $features)');

  // A missing toolchain must be a clear instruction, not a raw
  // ProcessException from the first rustup/cargo call.
  try {
    Process.runSync('cargo', ['--version']);
  } on ProcessException {
    throw StateError(
      'This build needs to compile the PDF engine from source (a trimmed '
      'feature set has no prebuilt binary), but Rust is not installed.\n'
      'Install it from https://rustup.rs (macOS, Linux, and Windows), '
      'then build again.',
    );
  }

  // A too-old cargo can't parse the engine's manifests — fail with an
  // actionable line, not cargo's cryptic edition error (issue #183).
  ensureCargoVersion(packageRoot);

  ensureRustTarget(targetTriple);

  final env = <String, String>{...environment};
  stripXcodeFromPath(env);

  final result = await Process.run(
    'cargo',
    [
      'build',
      '--manifest-path',
      manifestPath,
      '--lib',
      '--release',
      '--target',
      targetTriple,
      '--target-dir',
      targetDir,
      '--features',
      features,
    ],
    environment: {...Platform.environment, ...env},
  );

  if (result.exitCode != 0) {
    throw StateError(
      'cargo build failed (exit ${result.exitCode}).\n'
      'stderr: ${result.stderr}\n\n'
      'Ensure Rust is installed: https://rustup.rs\n'
      'Then: rustup target add $targetTriple',
    );
  }

  final compiled = p.join(
    targetDir,
    targetTriple,
    'release',
    libFileName.replaceAll('-', '_'),
  );

  if (!File(compiled).existsSync()) {
    throw StateError('Compiled library not found at $compiled.');
  }

  outFile.parent.createSync(recursive: true);
  File(compiled).copySync(outFile.path);
  _log.info('compiled → ${outFile.path}');
}
