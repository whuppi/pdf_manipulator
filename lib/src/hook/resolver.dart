/// Shared asset resolver for native build hook and web setup.
///
/// Both construct [ResolveRequest]s and pass them to [resolveAsset].
/// The resolver doesn't know or care who created the request — it
/// runs the same 5-step waterfall for every asset:
///
///   1. Cached — existing file, ONLY with a proof (see below)
///   2. Download — fetch from GitHub Releases, hash-verified → use it
///   3. Compile — vendor source on disk → build → use it
///   4. Submodule — .gitmodules exists → init recursive + compile → use it
///   5. Error — nothing worked, clear message with options
///
/// Every step obeys one rule: **a binary is only used with a proof** —
/// a matching content hash, or cargo's own freshness receipts (by
/// falling through to the compile step). "The file exists" is never a
/// proof: a stale binary looks exactly like a current one until it
/// crashes on a missing symbol.
library;

import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:logging/logging.dart';
import 'package:path/path.dart' as p;

final _log = Logger('pdf_manipulator:resolver');

/// Everything the resolver needs to resolve one asset.
///
/// Built by the Flutter build hook or by setup.dart. The resolver
/// treats both identically.
class ResolveRequest {
  /// Create a resolve request.
  const ResolveRequest({
    required this.assetName,
    required this.downloadUrl,
    required this.dest,
    required this.version,
    required this.packageRoot,
    required this.compile,
    this.cacheFile,
    this.expectedHash,
    this.force = false,
  });

  /// GitHub Release asset name (e.g. `macos-arm64-libpdf_oxide.dylib`
  /// or `wasm-pdf_oxide_bg.wasm`). Used as the hash lookup key.
  final String assetName;

  /// Full download URL for this asset on GitHub Releases.
  final String downloadUrl;

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

  /// When true, skip the cache check and re-resolve from download or
  /// compile. Used by `setup --force`.
  final bool force;

  /// Compile callback — builds the asset from vendor source and writes
  /// to the provided [File]. The resolver calls this when download fails
  /// and vendor source exists.
  final Future<void> Function(File dest) compile;
}

/// Resolve a single asset through the 5-step waterfall.
/// Returns true if the asset was freshly resolved (downloaded, compiled),
/// false if it was served from cache.
Future<bool> resolveAsset(ResolveRequest req) async {
  // Step 1 — Cached
  if (!req.force && await _tryCache(req)) return false;

  // Step 2 — Download (dev builds have no release to download from)
  if (req.version != '0.0.0') {
    final target = req.cacheFile ?? req.dest;
    if (await _download(req.downloadUrl, target)) {
      // Verify at download time, not on the next run's cache check —
      // release assets are not immutable by construction; the hash is.
      if (req.expectedHash != null &&
          await _sha256Of(target) != req.expectedHash) {
        _log.warning(
          '${req.assetName}: downloaded file does not match the '
          'pinned hash — discarding and falling back to source.',
        );
        target.deleteSync();
        return _resolveFromSource(req);
      }
      _copyIfNeeded(target, req.dest);
      return true;
    }
    _log.info('download unavailable, trying source compile');
  }

  return _resolveFromSource(req);
}

/// Step 1: serve an existing file ONLY with a proof.
///
/// Exactly two reuses are legitimate:
///   - the file's content matches the pinned hash, or
///   - there is no hash AND no source to rebuild from — reusing the
///     file is the only move that exists (loud when that's a release
///     version, because it means asset_hashes.dart has a gap).
///
/// Everything else falls through: when vendor sources are present,
/// freshness is cargo's question — its fingerprint check is ~1s when
/// nothing changed, and the alternative is trusting an unprovable
/// binary.
Future<bool> _tryCache(ResolveRequest req) async {
  final cached = req.cacheFile ?? req.dest;
  if (!cached.existsSync()) return false;

  if (req.expectedHash != null) {
    if (await _sha256Of(cached) == req.expectedHash) {
      _log.info('using cached ${cached.path} (hash verified)');
      _copyIfNeeded(cached, req.dest);
      return true;
    }
    _log.info('cached file hash mismatch — resolving fresh');
    return false;
  }

  if (hasVendorSource(req.packageRoot)) {
    _log.info(
      'no hash for ${req.assetName} (v${req.version}) — '
      'delegating freshness to source compile',
    );
    return false;
  }

  if (req.version == '0.0.0') {
    _log.info('using cached ${cached.path} (dev, no sources to rebuild)');
  } else {
    _log.warning(
      '${req.assetName}: no hash available for v${req.version}. '
      'Using cached file WITHOUT verification. '
      'This may indicate a missing entry in asset_hashes.dart.',
    );
  }
  _copyIfNeeded(cached, req.dest);
  return true;
}

Future<String> _sha256Of(File file) async =>
    sha256.convert(await file.readAsBytes()).toString();

/// Steps 3 + 4 of the waterfall: vendor-source compile, then
/// submodule init + compile, then the explanatory failure.
Future<bool> _resolveFromSource(ResolveRequest req) async {
  // Step 3 — Compile from vendor source
  if (hasVendorSource(req.packageRoot)) {
    _log.info('compiling from vendor source');
    req.dest.parent.createSync(recursive: true);
    await req.compile(req.dest);
    _cacheIfNeeded(req.dest, req.cacheFile);
    return true;
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
      return true;
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
  final result = await Process.run('git', [
    'submodule',
    'update',
    '--init',
    '--recursive',
  ], workingDirectory: p.fromUri(packageRoot));
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
  final lines = File.fromUri(
    packageRoot.resolve('pubspec.yaml'),
  ).readAsLinesSync();
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
