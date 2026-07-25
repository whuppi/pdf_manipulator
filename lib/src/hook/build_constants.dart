// build.json is the single source of truth for build constants, shared
// with tool/ci/release.sh and tool/compile.dart. Both hooks (build +
// link) and bin/setup.dart load it through this class — never parse
// build.json anywhere else.

import 'dart:convert';
import 'dart:io';

/// The parsed contents of the package's `build.json`.
class BuildConstants {
  BuildConstants._({
    required this.crate,
    required this.repo,
    required this.webAssets,
    required this.wasmBuildOutputs,
    required this.nativeFeatures,
    required this.wasmFeatures,
  });

  /// Loads `build.json` from the package root.
  factory BuildConstants.load(Uri packageRoot) {
    final file = File.fromUri(packageRoot.resolve('build.json'));
    final json = jsonDecode(file.readAsStringSync()) as Map<String, dynamic>;
    final features = json['features'] as Map;
    return BuildConstants._(
      crate: json['crate'] as String,
      repo: json['repo'] as String,
      webAssets: Map<String, String>.from(json['web'] as Map),
      wasmBuildOutputs: Set<String>.from(json['wasmBuildOutputs'] as List),
      nativeFeatures: features['native'] as String,
      wasmFeatures: features['wasm'] as String,
    );
  }

  /// Rust crate name (also the library file stem).
  final String crate;

  /// GitHub `owner/repo` hosting the release binaries.
  final String repo;

  /// Local web file name → GitHub Release asset name.
  final Map<String, String> webAssets;

  /// The subset of [webAssets] keys produced by the wasm build (the rest
  /// are hand-written files copied from web_assets/).
  final Set<String> wasmBuildOutputs;

  /// Default cargo feature list for native targets.
  final String nativeFeatures;

  /// Default cargo feature list for the wasm target.
  final String wasmFeatures;

  /// The GitHub Release download URL for [assetName] at [version].
  String downloadUrl(String version, String assetName) =>
      'https://github.com/$repo/releases/download/v$version/$assetName';
}
