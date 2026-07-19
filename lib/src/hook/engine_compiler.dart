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
