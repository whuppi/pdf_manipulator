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
import 'package:crypto/crypto.dart';
import 'package:hooks/hooks.dart';
import 'package:logging/logging.dart';
import 'package:path/path.dart' as p;

import 'package:pdf_manipulator/src/hook/asset_hashes.dart';

final _log = Logger('pdf_manipulator:build');

const _assetId = 'src/ffi/native_bindings.g.dart';
const _crateName = 'pdf_oxide';
const _releaseRepo = 'https://github.com/whuppi/pdf_manipulator/releases/download';
const _featuresFallback = 'icc,legacy-crypto,rendering,signatures,native-bridge';

/// Read features from compile_rust.sh (single source of truth).
/// Falls back to hardcoded value for pub.dev consumers who don't have the script.
String _resolveFeatures(Uri packageRoot) {
  final script = File.fromUri(packageRoot.resolve('tool/compile_rust.sh'));
  if (script.existsSync()) {
    final result = Process.runSync('bash', [script.path, '--features', 'native']);
    if (result.exitCode == 0) {
      final features = (result.stdout as String).trim();
      if (features.isNotEmpty) return features;
    }
  }
  return _featuresFallback;
}

void main(List<String> args) async {
  await build(args, (BuildInput input, BuildOutputBuilder output) async {
    if (!input.config.buildCodeAssets) return;

    final codeConfig = input.config.code;
    final targetTriple = _targetTriple(codeConfig);
    final linkMode = _linkMode(codeConfig);
    final libFileName = codeConfig.targetOS
        .libraryFileName(_crateName, linkMode);
    final outFile = File.fromUri(input.outputDirectory.resolve(libFileName));

    // Resolution order:
    // 1. Try pre-built binary (fast, no toolchain needed)
    // 2. Compile from source if vendor/pdf_oxide exists
    // 3. Init submodules if .gitmodules exists (git dep with ref: dev)
    // 4. Clear error with options

    final cargoToml = File.fromUri(
      input.packageRoot.resolve('vendor/pdf_oxide/Cargo.toml'),
    );
    final version = _readVersion(input.packageRoot);
    final isDevVersion = version == '0.0.0';

    // Step 1: Try pre-built binary (skip for 0.0.0 — no release exists)
    if (!isDevVersion) {
      final downloaded = await _tryDownloadPrebuilt(
        input, codeConfig, libFileName, outFile,
      );
      if (downloaded) {
        // Success — skip to asset registration below
      } else if (cargoToml.existsSync()) {
        // Step 2: Binary unavailable, but source exists — compile
        _log.warning(
          'Pre-built binary unavailable. Compiling from source '
          '(requires Rust toolchain).',
        );
        await _compileFromSource(input, targetTriple, linkMode, outFile);
      } else {
        throw StateError(
          'Pre-built binary unavailable and no Rust source found.\n\n'
          'Options:\n'
          '  1. Use a published version: pdf_manipulator: ^X.Y.Z\n'
          '  2. Use a git tag with binaries: ref: vX.Y.Z\n'
          '  3. Clone with --recursive and use a path dependency\n',
        );
      }
    } else if (cargoToml.existsSync()) {
      // Step 2: Dev version (0.0.0) with source — compile directly
      await _compileFromSource(input, targetTriple, linkMode, outFile);
    } else {
      // Step 3: Dev version, no source — try init submodules (git dep user)
      final gitmodules = File.fromUri(
        input.packageRoot.resolve('.gitmodules'),
      );
      if (gitmodules.existsSync()) {
        _log.warning(
          'No pre-built binary for dev version. Initializing submodules '
          'and compiling from source (requires Rust toolchain).',
        );
        final initResult = await Process.run(
          'git',
          ['submodule', 'update', '--init', '--recursive'],
          workingDirectory: p.fromUri(input.packageRoot),
        );
        if (initResult.exitCode != 0) {
          throw StateError(
            'Failed to initialize submodules.\n'
            'stderr: ${initResult.stderr}\n\n'
            'Clone manually with --recursive and use a path dependency.',
          );
        }
        if (!cargoToml.existsSync()) {
          throw StateError(
            'Submodules initialized but vendor/pdf_oxide/Cargo.toml '
            'still missing. The submodule may be misconfigured.',
          );
        }
        await _compileFromSource(input, targetTriple, linkMode, outFile);
      } else {
        // Step 4: No binary, no source, no submodules — nothing we can do
        throw StateError(
          'No pre-built binary and no Rust source available.\n\n'
          'Options:\n'
          '  1. Use a published version: pdf_manipulator: ^X.Y.Z\n'
          '  2. Use a git tag with binaries: ref: vX.Y.Z\n'
          '  3. Clone with --recursive and use a path dependency\n',
        );
      }
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

    // Register every source file Cargo used via its dep-info (.d) file.
    // Cargo writes this automatically next to each artifact, listing every
    // .rs, .toml, font, and asset that contributed to the build.
    // The hooks_runner MD5-hashes each registered file — any content change
    // triggers a recompile. No hand-written file lists to maintain.
    // Pattern from native_toolchain_rust (irondash).
    if (cargoToml.existsSync()) {
      final targetDir = p.join(p.fromUri(input.outputDirectory), 'cargo_target');
      final depInfoPath = p.join(
        targetDir, targetTriple, 'release', 'deps', 'pdf_oxide.d',
      );
      _registerCargoDeps(output, depInfoPath, input.packageRoot);
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
      // API 21 = Android 5.0 — matches compile_rust.sh's release builds.
      final linker = p.join(compilerDir, '${ndkTriple}21-clang');
      final ar = p.join(compilerDir, 'llvm-ar');
      final envKey = 'CARGO_TARGET_${targetTriple.toUpperCase().replaceAll('-', '_')}';
      env['${envKey}_LINKER'] = linker;
      env['${envKey}_AR'] = ar;
      _log.info('NDK linker: $linker');
    }
  }

  // macOS: strip Xcode Developer injections from PATH unconditionally.
  // Flutter's build system injects Xcode paths that break Cargo's
  // build-script host compilation (proc-macro2, quote, libc).
  // Same fix as native_toolchain_rust:
  // https://github.com/irondash/native_toolchain_rust/issues/17
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
      '--manifest-path', manifestPath,
      '--lib',
      '--release',
      '--target', targetTriple,
      '--target-dir', targetDir,
      '--features', _resolveFeatures(input.packageRoot),
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

// ── Path 1: Try download from GitHub Releases (returns false on 404) ────

Future<bool> _tryDownloadPrebuilt(
  BuildInput input,
  CodeConfig codeConfig,
  String libFileName,
  File outFile,
) async {
  final version = _readVersion(input.packageRoot);
  final platform = _platformKey(codeConfig);
  final assetKey = '$platform-$libFileName';
  final cachedFile = File.fromUri(
    input.outputDirectoryShared.resolve(libFileName),
  );

  // Check cache first
  if (cachedFile.existsSync()) {
    final expectedHash = assetHashesSha256[assetKey];
    if (expectedHash != null) {
      final actualHash = sha256.convert(await cachedFile.readAsBytes()).toString();
      if (actualHash == expectedHash) {
        _log.info('using cached ${cachedFile.path} (hash verified)');
        outFile.parent.createSync(recursive: true);
        cachedFile.copySync(outFile.path);
        return true;
      }
      _log.info('cached file hash mismatch — re-downloading');
    } else {
      _log.info('using cached ${cachedFile.path} (no hash to verify)');
      outFile.parent.createSync(recursive: true);
      cachedFile.copySync(outFile.path);
      return true;
    }
  }

  // Download
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
      _log.info('binary not available (HTTP ${response.statusCode})');
      return false;
    }

    cachedFile.parent.createSync(recursive: true);
    final sink = cachedFile.openWrite();
    await response.pipe(sink);
    final mb = (cachedFile.lengthSync() / 1024 / 1024).toStringAsFixed(1);
    _log.info('downloaded ${cachedFile.path} ($mb MB)');

    outFile.parent.createSync(recursive: true);
    cachedFile.copySync(outFile.path);
    return true;
  } catch (e) {
    _log.warning('download failed: $e');
    return false;
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

// ── Cargo dep-info parsing ─────────────────────────────────────────────
//
// Cargo writes a .d file (Makefile dep-info format) next to each compiled
// artifact listing every source file that contributed to the build. We
// parse it and register each file as a build dependency so the hooks_runner
// invalidates the cache on any content change.
// Pattern from native_toolchain_rust (irondash).

void _registerCargoDeps(
  BuildOutputBuilder output,
  String depInfoPath,
  Uri packageRoot,
) {
  final depFile = File(depInfoPath);
  if (!depFile.existsSync()) {
    _log.warning('dep-info not found at $depInfoPath');
    return;
  }

  final content = depFile.readAsStringSync();
  final crateRoot = p.fromUri(packageRoot.resolve('vendor/pdf_oxide/'));
  final packageDir = p.fromUri(packageRoot);
  var registered = 0;

  for (final line in content.split('\n')) {
    final trimmed = line.trim();
    if (trimmed.isEmpty || trimmed.startsWith('#')) continue;

    final colonIdx = trimmed.indexOf(':');
    if (colonIdx < 0) continue;
    final deps = trimmed.substring(colonIdx + 1).trim();

    if (deps.isEmpty) {
      // Standalone "src/foo.rs:" line — the path itself is the dep.
      final candidate = trimmed.substring(0, colonIdx).trim();
      if (candidate.isNotEmpty) {
        _addDep(output, packageDir, crateRoot, candidate);
        registered++;
      }
      continue;
    }

    for (final dep in deps.split(' ')) {
      final d = dep.trim();
      if (d.isNotEmpty) {
        _addDep(output, packageDir, crateRoot, d);
        registered++;
      }
    }
  }

  _log.info('registered $registered source deps from Cargo dep-info');
}

void _addDep(
  BuildOutputBuilder output,
  String packageDir,
  String crateRoot,
  String depPath,
) {
  final absolute = p.isAbsolute(depPath)
      ? depPath
      : p.normalize(p.join(crateRoot, depPath));

  if (!File(absolute).existsSync()) return;
  if (!p.isWithin(packageDir, absolute)) return;

  output.dependencies.add(Uri.file(absolute));
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
