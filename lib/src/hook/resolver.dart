/// Shared asset resolver for native build hook and web setup.
///
/// Both construct [ResolveRequest]s and pass them to [resolveAsset].
/// The resolver doesn't know or care who created the request — it
/// runs the same 5-step waterfall for every asset:
///
///   1. Cached — file exists locally + hash matches → use it
///   2. Download — fetch from GitHub Releases → cache → use it
///   3. Compile — vendor source on disk → build → use it
///   4. Submodule — .gitmodules exists → init recursive + compile → use it
///   5. Error — nothing worked, clear message with options
library;

import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:logging/logging.dart';
import 'package:path/path.dart' as p;

final _log = Logger('pdf_manipulator:resolver');

const _releaseRepo =
    'https://github.com/whuppi/pdf_manipulator/releases/download';

/// Everything the resolver needs to resolve one asset.
///
/// Built by the Flutter build hook or by setup.dart. The resolver
/// treats both identically.
class ResolveRequest {
  /// Create a resolve request.
  const ResolveRequest({
    required this.assetName,
    required this.dest,
    required this.version,
    required this.packageRoot,
    required this.compile,
    this.cacheFile,
    this.expectedHash,
  });

  /// GitHub Release asset name (e.g. `macos-arm64-libpdf_oxide.dylib`
  /// or `wasm-pdf_oxide_bg.wasm`). Used to build the download URL and
  /// as the hash lookup key.
  final String assetName;

  /// Final output location for the resolved file.
  final File dest;

  /// Optional shared cache location. The build hook provides one via
  /// the Flutter build system's shared output directory; setup.dart
  /// may not.
  final File? cacheFile;

  /// Expected SHA-256 hash from asset_hashes.dart, or null if unknown.
  final String? expectedHash;

  /// Package version (e.g. `1.0.3`). Used to build the download URL.
  /// `0.0.0` means dev version — skip download, go straight to compile.
  final String version;

  /// Package root URI. Used to locate vendor source and .gitmodules.
  final Uri packageRoot;

  /// Compile callback — builds the asset from vendor source and writes
  /// to the provided [File]. The resolver calls this when download fails
  /// and vendor source exists.
  final Future<void> Function(File dest) compile;
}

/// Resolve a single asset through the 5-step waterfall.
Future<void> resolveAsset(ResolveRequest req) async {
  // Step 1 — Cached
  final fileToCheck = req.cacheFile ?? req.dest;
  if (fileToCheck.existsSync()) {
    if (req.expectedHash != null) {
      final actual =
          sha256.convert(await fileToCheck.readAsBytes()).toString();
      if (actual == req.expectedHash) {
        _log.info('using cached ${fileToCheck.path} (hash verified)');
        _copyIfNeeded(fileToCheck, req.dest);
        return;
      }
      _log.info('cached file hash mismatch — resolving fresh');
    } else {
      _log.info('using cached ${fileToCheck.path} (no hash to verify)');
      _copyIfNeeded(fileToCheck, req.dest);
      return;
    }
  }

  // Step 2 — Download
  if (req.version != '0.0.0') {
    final url = '$_releaseRepo/v${req.version}/${req.assetName}';
    final target = req.cacheFile ?? req.dest;
    if (await _download(url, target)) {
      _copyIfNeeded(target, req.dest);
      return;
    }
    _log.info('download unavailable, trying source compile');
  }

  // Step 3 — Compile from vendor source
  if (hasVendorSource(req.packageRoot)) {
    _log.info('compiling from vendor source');
    req.dest.parent.createSync(recursive: true);
    await req.compile(req.dest);
    _cacheIfNeeded(req.dest, req.cacheFile);
    return;
  }

  // Step 4 — Init submodules + compile
  if (hasGitmodules(req.packageRoot)) {
    _log.warning(
      'No pre-built binary. Initializing submodules and compiling '
      'from source (requires Rust toolchain).',
    );
    await initSubmodules(req.packageRoot);
    if (hasVendorSource(req.packageRoot)) {
      req.dest.parent.createSync(recursive: true);
      await req.compile(req.dest);
      _cacheIfNeeded(req.dest, req.cacheFile);
      return;
    }
  }

  // Step 5 — Error
  throw StateError(
    'Could not resolve ${req.assetName}.\n\n'
    'Tried: cache → download → source compile → submodule init.\n\n'
    'Options:\n'
    '  1. Use a published version: pdf_manipulator: ^X.Y.Z\n'
    '  2. Use a git tag with binaries: ref: vX.Y.Z\n'
    '  3. Install Rust (https://rustup.rs) for source compilation\n'
    '  4. Clone with --recursive for submodule access\n',
  );
}

/// Copy [src] to [dest] if they're different paths.
void _copyIfNeeded(File src, File dest) {
  if (src.path == dest.path) return;
  dest.parent.createSync(recursive: true);
  src.copySync(dest.path);
}

/// Cache [src] to [cacheFile] if cache is configured.
void _cacheIfNeeded(File src, File? cacheFile) {
  if (cacheFile == null || src.path == cacheFile.path) return;
  cacheFile.parent.createSync(recursive: true);
  src.copySync(cacheFile.path);
}

/// Check if `vendor/pdf_oxide/Cargo.toml` exists.
bool hasVendorSource(Uri packageRoot) {
  return File.fromUri(
    packageRoot.resolve('vendor/pdf_oxide/Cargo.toml'),
  ).existsSync();
}

/// Check if `.gitmodules` exists in the package root.
bool hasGitmodules(Uri packageRoot) {
  return File.fromUri(packageRoot.resolve('.gitmodules')).existsSync();
}

/// Run `git submodule update --init --recursive`.
Future<void> initSubmodules(Uri packageRoot) async {
  final result = await Process.run(
    'git',
    ['submodule', 'update', '--init', '--recursive'],
    workingDirectory: p.fromUri(packageRoot),
  );
  if (result.exitCode != 0) {
    throw StateError(
      'Failed to initialize submodules.\n'
      'stderr: ${result.stderr}\n\n'
      'Clone manually with --recursive and use a path dependency.',
    );
  }
}

/// Read the package version from pubspec.yaml.
String readVersion(Uri packageRoot) {
  final lines =
      File.fromUri(packageRoot.resolve('pubspec.yaml')).readAsLinesSync();
  for (final line in lines) {
    if (line.startsWith('version:')) {
      return line.substring('version:'.length).trim();
    }
  }
  throw StateError('No version in pubspec.yaml');
}

/// Download a file with redirect following. Returns true on success.
Future<bool> _download(String url, File dest) async {
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
      if (response.isRedirect &&
          response.headers.value('location') != null) {
        await response.drain<void>();
        uri = Uri.parse(response.headers.value('location')!);
        redirects++;
      } else {
        break;
      }
    } while (redirects < 5);

    if (response.statusCode != 200) {
      await response.drain<void>();
      _log.info('download failed (HTTP ${response.statusCode})');
      return false;
    }

    dest.parent.createSync(recursive: true);
    final sink = dest.openWrite();
    await response.pipe(sink);
    final mb = (dest.lengthSync() / 1024 / 1024).toStringAsFixed(1);
    _log.info('downloaded ${dest.path} ($mb MB)');
    return true;
  } catch (e) {
    _log.warning('download error: $e');
    return false;
  } finally {
    client.close();
  }
}
