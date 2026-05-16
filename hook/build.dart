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
// The fork is automatic: vendor/pdf_oxide/Cargo.toml exists → compile.
// Doesn't exist → download. Version read from pubspec.yaml — no
// hardcoded constants to keep in sync.

import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:hooks/hooks.dart';
import 'package:logging/logging.dart';

final _log = Logger('pdf_manipulator:build');

const _assetId = 'src/ffi/native_bindings.g.dart';
const _releaseRepo = 'https://github.com/whuppi/pdf_manipulator/releases/download';
const _features = 'icc,legacy-crypto,rendering,signatures';

void main(List<String> args) async {
  await build(args, (BuildInput input, BuildOutputBuilder output) async {
    if (!input.config.buildCodeAssets) return;

    final libName = input.config.code.targetOS.dylibFileName('pdf_oxide');
    final outFile = File.fromUri(input.outputDirectory.resolve(libName));

    if (!outFile.existsSync()) {
      final cargoToml = File.fromUri(
        input.packageRoot.resolve('vendor/pdf_oxide/Cargo.toml'),
      );

      if (cargoToml.existsSync()) {
        await _compileFromSource(input, outFile);
      } else {
        await _downloadPrebuilt(input, outFile);
      }
    }

    output.assets.code.add(
      CodeAsset(
        package: input.packageName,
        name: _assetId,
        linkMode: DynamicLoadingBundled(),
        file: outFile.uri,
      ),
      routing: input.config.linkingEnabled
          ? ToLinkHook(input.packageName)
          : const ToAppBundle(),
    );
    output.dependencies.add(input.packageRoot.resolve('hook/build.dart'));
    output.dependencies.add(input.packageRoot.resolve('pubspec.yaml'));
  });
}

// ── Path 1: Compile from source ────────────────────────────────────────

Future<void> _compileFromSource(BuildInput input, File outFile) async {
  final target = _cargoTarget(input.config.code);
  final manifest = input.packageRoot
      .resolve('vendor/pdf_oxide/Cargo.toml')
      .toFilePath();

  _log.info('compiling from source for $target');

  final result = await Process.run('cargo', [
    'build',
    '--manifest-path', manifest,
    '--lib',
    '--release',
    '--target', target,
    '--features', _features,
  ]);

  if (result.exitCode != 0) {
    throw StateError(
      'cargo build failed (exit ${result.exitCode}).\n'
      'stderr: ${result.stderr}\n\n'
      'Ensure Rust is installed: https://rustup.rs\n'
      'Then: rustup target add $target',
    );
  }

  final releaseDir = input.packageRoot
      .resolve('vendor/pdf_oxide/target/$target/release/')
      .toFilePath();
  final compiled = _findLib(releaseDir, input.config.code.targetOS);

  outFile.parent.createSync(recursive: true);
  File(compiled).copySync(outFile.path);
  _log.info('compiled → ${outFile.path}');
}

String _findLib(String dir, OS os) {
  final names = switch (os) {
    OS.macOS => ['libpdf_oxide.dylib'],
    OS.iOS => ['libpdf_oxide.a'],
    OS.android => ['libpdf_oxide.so'],
    OS.linux => ['libpdf_oxide.so'],
    OS.windows => ['pdf_oxide.dll'],
    _ => <String>[],
  };
  for (final name in names) {
    final path = '$dir$name';
    if (File(path).existsSync()) return path;
  }
  throw StateError('No compiled library in $dir');
}

// ── Path 2: Download from GitHub Releases ──────────────────────────────

Future<void> _downloadPrebuilt(BuildInput input, File outFile) async {
  final version = _readVersion(input.packageRoot);
  final platform = _platformKey(input.config.code);
  final libName = outFile.uri.pathSegments.last;
  final url = '$_releaseRepo/v$version/$platform-$libName';

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
    (_, _) => throw UnimplementedError(
        'Unsupported: ${code.targetOS} ${code.targetArchitecture}'),
  };
}

String _cargoTarget(CodeConfig code) {
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
    (_, _) => throw UnimplementedError(
        'Unsupported: ${code.targetOS} ${code.targetArchitecture}'),
  };
}
