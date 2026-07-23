// user_defines.dart — reads the consuming app's
// `hooks: user_defines: pdf_manipulator:` block from its pubspec.yaml.
//
// The native build hook receives this block already parsed as
// BuildInput.userDefines. The web setup script is a plain CLI, not a build
// hook, so it reads the same block itself. That gives web and native ONE
// config source (the app's pubspec) and ONE set of keys — `keep`, `build`,
// `detector` — so the setup/run commands never carry flags.

import 'dart:io';

import 'package:yaml/yaml.dart';

/// Reads `hooks: user_defines: pdf_manipulator:` from [appRoot]'s
/// pubspec.yaml. Returns an empty map when the file or block is absent — the
/// defaults (speed build, keep everything) then apply. Values are plain Dart
/// (Map/List/scalars), matching exactly what the native hook receives via
/// `BuildInput.userDefines`, so the downstream parsers (`EngineBuild.parse`,
/// `resolveKeepPlan`) behave identically for both callers.
Map<String, Object?> readPdfManipulatorUserDefines(String appRoot) {
  final file = File('$appRoot/pubspec.yaml');
  if (!file.existsSync()) return const {};
  final doc = loadYaml(file.readAsStringSync());
  final block = _dig(doc, const ['hooks', 'user_defines', 'pdf_manipulator']);
  final plain = _toDart(block);
  return plain is Map<String, Object?> ? plain : const {};
}

/// Walks [path] into nested maps. Returns null the moment a key is missing —
/// an absent block is normal (the consumer just didn't configure anything).
Object? _dig(Object? node, List<String> path) {
  var cur = node;
  for (final key in path) {
    if (cur is Map && cur.containsKey(key)) {
      cur = cur[key] as Object?;
    } else {
      return null;
    }
  }
  return cur;
}

/// Deep-converts YamlMap/YamlList into plain Dart collections so downstream
/// `is Map` / `is List` checks see the same shapes the native hook carries.
Object? _toDart(Object? node) {
  if (node is YamlMap) {
    return <String, Object?>{
      for (final e in node.entries) e.key.toString(): _toDart(e.value),
    };
  }
  if (node is YamlList) {
    return node.map(_toDart).toList();
  }
  return node;
}
