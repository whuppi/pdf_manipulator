// Everything needed to turn a hook's CodeConfig into a compiled engine:
// the CodeConfig → target mappings and the cargo invocation itself.
// Shared by hook/build.dart (default + analyzer-trimmed builds) and
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

  final targetCheck = Process.runSync('rustup', [
    'target',
    'list',
    '--installed',
  ]);
  if (targetCheck.exitCode == 0 &&
      !(targetCheck.stdout as String).contains(targetTriple)) {
    _log.info('installing Rust target: $targetTriple');
    Process.runSync('rustup', ['target', 'add', targetTriple]);
  }

  final env = <String, String>{...environment};

  // macOS: strip Xcode Developer PATH injections that break cargo
  if (Platform.isMacOS) {
    env['PATH'] = Platform.environment['PATH']!
        .split(':')
        .where((e) => !e.contains('Contents/Developer/'))
        .join(':');
  }

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
