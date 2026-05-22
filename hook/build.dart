// Build hook for pdf_manipulator.
//
// Two paths, selected automatically by what's on disk:
//
//   CONTRIBUTOR (cloned repo with vendor/pdf_oxide/ submodule):
//     → Compiles from source via cargo build
//     → Always fresh, matches their code changes
//     → Requires: Rust toolchain (https://rustup.rs)
//
//   CONSUMER (installed from pub.dev, no vendor/ directory):
//     → Downloads pre-built binary from GitHub Releases
//     → Zero toolchain required
//
// Link mode, target triple, library filename, and Android NDK linker
// are all read from the CodeConfig input — never hardcoded.
// Pattern learned from native_toolchain_rust.

import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:hooks/hooks.dart';
import 'package:logging/logging.dart';
import 'package:path/path.dart' as p;

final _log = Logger('pdf_manipulator:build');

const _assetId = 'src/ffi/native_bindings.g.dart';
const _crateName = 'pdf_oxide';
const _releaseRepo = 'https://github.com/whuppi/pdf_manipulator/releases/download';
const _features = 'icc,legacy-crypto,rendering,signatures';

void main(List<String> args) async {
  await build(args, (BuildInput input, BuildOutputBuilder output) async {
    if (!input.config.buildCodeAssets) return;

    final codeConfig = input.config.code;
    final targetTriple = _targetTriple(codeConfig);
    final linkMode = _linkMode(codeConfig);
    final libFileName = codeConfig.targetOS
        .libraryFileName(_crateName, linkMode);
    final outFile = File.fromUri(input.outputDirectory.resolve(libFileName));

    final cargoToml = File.fromUri(
      input.packageRoot.resolve('vendor/pdf_oxide/Cargo.toml'),
    );

    if (cargoToml.existsSync()) {
      await _compileFromSource(input, targetTriple, linkMode, outFile);
    } else if (!outFile.existsSync()) {
      await _downloadPrebuilt(input, codeConfig, libFileName, outFile);
    }

    output.assets.code.add(
      CodeAsset(
        package: input.packageName,
        name: _assetId,
        linkMode: linkMode,
        file: outFile.uri,
      ),
      routing: input.config.linkingEnabled
          ? ToLinkHook(input.packageName)
          : const ToAppBundle(),
    );
    output.dependencies.add(input.packageRoot.resolve('hook/build.dart'));
    output.dependencies.add(input.packageRoot.resolve('pubspec.yaml'));
    output.dependencies.add(input.packageRoot.resolve('vendor/pdf_oxide/Cargo.toml'));
    // List key source files so the hooks_runner invalidates on changes.
    // Directories aren't tracked reliably — individual files are.
    for (final path in [
      'vendor/pdf_oxide/src/ffi.rs',
      'vendor/pdf_oxide/src/wasm.rs',
      'vendor/pdf_oxide/src/document.rs',
      'vendor/pdf_oxide/src/bridge/mod.rs',
      'vendor/pdf_oxide/src/bridge/ffi_api.rs',
      'vendor/pdf_oxide/src/bridge/thread_pool.rs',
      'vendor/pdf_oxide/src/bridge/callback_reader.rs',
      'vendor/pdf_oxide/src/bridge/callback_writer.rs',
      'vendor/pdf_oxide/src/bridge/shared_buffer.rs',
      'vendor/pdf_oxide/src/bridge/arena.rs',
    ]) {
      output.dependencies.add(input.packageRoot.resolve(path));
    }
  });
}

// ── Path 1: Compile from source ────────────────────────────────────────

Future<void> _compileFromSource(
  BuildInput input,
  String targetTriple,
  LinkMode linkMode,
  File outFile,
) async {
  final manifestPath = p.fromUri(
    input.packageRoot.resolve('vendor/pdf_oxide/Cargo.toml'),
  );
  final targetDir = p.join(p.fromUri(input.outputDirectory), 'cargo_target');

  _log.info('compiling from source for $targetTriple');

  final env = <String, String>{};

  // Android: use the C compiler Flutter provides via CodeConfig
  final codeConfig = input.config.code;
  if (codeConfig.targetOS == OS.android) {
    final cc = codeConfig.cCompiler;
    if (cc != null) {
      final compilerDir = p.dirname(p.fromUri(cc.compiler));
      final ndkTriple = targetTriple == 'armv7-linux-androideabi'
          ? 'armv7a-linux-androideabi'
          : targetTriple;
      final linker = p.join(compilerDir, '${ndkTriple}35-clang');
      final ar = p.join(compilerDir, 'llvm-ar');
      final envKey = 'CARGO_TARGET_${targetTriple.toUpperCase().replaceAll('-', '_')}';
      env['${envKey}_LINKER'] = linker;
      env['${envKey}_AR'] = ar;
      _log.info('NDK linker: $linker');
    }
  }

  // macOS: strip Xcode injections from PATH that break host builds
  if (Platform.isMacOS && codeConfig.targetOS != OS.macOS) {
    env['PATH'] = Platform.environment['PATH']!
        .split(':')
        .where((e) => !e.contains('Contents/Developer/'))
        .join(':');
  }

  final result = await Process.run(
    'cargo',
    [
      'build',
      '--manifest-path', manifestPath,
      '--lib',
      '--release',
      '--target', targetTriple,
      '--target-dir', targetDir,
      '--features', _features,
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

  // Find the compiled library using the same filename the output expects
  final compiled = p.join(
    targetDir,
    targetTriple,
    'release',
    codeConfig.targetOS
        .libraryFileName(_crateName, linkMode)
        .replaceAll('-', '_'),
  );

  if (!File(compiled).existsSync()) {
    throw StateError(
      'Compiled library not found at $compiled.\n'
      'Expected: ${codeConfig.targetOS.libraryFileName(_crateName, linkMode)}',
    );
  }

  outFile.parent.createSync(recursive: true);
  File(compiled).copySync(outFile.path);
  _log.info('compiled → ${outFile.path}');
}

// ── Path 2: Download from GitHub Releases ──────────────────────────────

Future<void> _downloadPrebuilt(
  BuildInput input,
  CodeConfig codeConfig,
  String libFileName,
  File outFile,
) async {
  final version = _readVersion(input.packageRoot);
  final platform = _platformKey(codeConfig);
  final url = '$_releaseRepo/v$version/$platform-$libFileName';

  _log.info('downloading $url');

  final client = HttpClient();
  try {
    var uri = Uri.parse(url);
    HttpClientResponse response;
    var redirects = 0;
    do {
      final req = await client.getUrl(uri);
      req.followRedirects = false;
      response = await req.close();
      if (response.isRedirect && response.headers.value('location') != null) {
        await response.drain<void>();
        uri = Uri.parse(response.headers.value('location')!);
        redirects++;
      } else {
        break;
      }
    } while (redirects < 5);

    if (response.statusCode != 200) {
      await response.drain<void>();
      throw StateError(
        'Failed to download pre-built binary.\n'
        'URL: $url\n'
        'Status: ${response.statusCode}\n\n'
        'Options:\n'
        '  1. Install Rust (https://rustup.rs) and clone with --recursive\n'
        '  2. Wait for a release that includes $platform binaries\n',
      );
    }

    outFile.parent.createSync(recursive: true);
    final sink = outFile.openWrite();
    await response.pipe(sink);
    final mb = (outFile.lengthSync() / 1024 / 1024).toStringAsFixed(1);
    _log.info('downloaded ${outFile.path} ($mb MB)');
  } finally {
    client.close();
  }
}

// ── Config mapping (from native_toolchain_rust pattern) ────────────────

LinkMode _linkMode(CodeConfig code) {
  return switch (code.linkModePreference) {
    LinkModePreference.dynamic ||
    LinkModePreference.preferDynamic => DynamicLoadingBundled(),
    LinkModePreference.static ||
    LinkModePreference.preferStatic => StaticLinking(),
    _ => DynamicLoadingBundled(),
  };
}

String _targetTriple(CodeConfig code) {
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
        'Unsupported: ${code.targetOS} ${code.targetArchitecture}'),
  };
}

// ── Helpers ────────────────────────────────────────────────────────────

String _readVersion(Uri packageRoot) {
  final lines = File.fromUri(packageRoot.resolve('pubspec.yaml'))
      .readAsLinesSync();
  for (final line in lines) {
    if (line.startsWith('version:')) {
      return line.substring('version:'.length).trim();
    }
  }
  throw StateError('No version in pubspec.yaml');
}

String _platformKey(CodeConfig code) {
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
        'Unsupported: ${code.targetOS} ${code.targetArchitecture}'),
  };
}
