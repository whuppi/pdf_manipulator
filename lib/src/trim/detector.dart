// The stable trim detector: a dependency-free text scan over the app's
// source. Only files that import pdf_manipulator count; within them,
// any capability member name (word-bounded) keeps its capability.
//
// The scan can only err toward OVER-keeping (a same-named identifier in
// an importing file keeps a capability the app never calls) — the app
// always works, the binary is just less trimmed. It cannot realistically
// under-keep: a call site always spells the member name in the text,
// including dynamic calls that resolution-based analysis silently skips.
// Users who want the exact minimum state `trim: {keep: [...]}`.
//
// Deliberately NOT built on package:analyzer: a runtime package must
// never put the analyzer in a consumer's dependency graph (it fights
// the app's own codegen/lint tooling — issue #171). Do not reintroduce
// it here.

import 'dart:io';

import 'package:pdf_manipulator/src/trim/capabilities.dart';

/// What a scan concluded. [resolved] false means the scan could not read
/// part of the app — callers MUST fall back to the full binary and
/// surface [unresolvedPaths].
class DetectorResult {
  /// Creates a result; see field docs for the contract.
  const DetectorResult({
    required this.keep,
    required this.resolved,
    required this.unresolvedPaths,
    required this.matchedMembers,
  });

  /// Capabilities the app can reach. Meaningful only when [resolved].
  final Set<PdfCapability> keep;

  /// True when every scanned file was readable — the keep-set is proven.
  final bool resolved;

  /// Files the scan could not read (drives the fail-closed path).
  final List<String> unresolvedPaths;

  /// The member names that matched, for `compare` mode and logs.
  final Set<String> matchedMembers;
}

/// Scans the app rooted at [appRoot] (its `lib/` plus any additional
/// [extraDirs], e.g. `bin/`) for reachable pdf_manipulator capabilities.
Future<DetectorResult> detectCapabilities(
  String appRoot, {
  List<String> extraDirs = const [],
}) async {
  final roots = <String>[
    Directory('$appRoot/lib').existsSync() ? '$appRoot/lib' : appRoot,
    for (final d in extraDirs)
      if (Directory(d).existsSync()) d,
  ];

  // Member name → capabilities it implies. Qualified apiMembers keys
  // collapse to their member part: text has no resolution, `doc.extract`
  // and a bare `extract` read the same.
  final byMember = <String, Set<PdfCapability>>{};
  for (final e in PdfCapability.apiMembers.entries) {
    byMember.putIfAbsent(e.key.split('.').last, () => {}).add(e.value);
  }
  final patterns = {
    for (final m in byMember.keys) m: RegExp('\\b${RegExp.escape(m)}\\b'),
  };

  final keep = <PdfCapability>{};
  final matched = <String>{};
  final unresolved = <String>[];

  for (final root in roots) {
    for (final entry in Directory(root).listSync(recursive: true)) {
      if (entry is! File || !entry.path.endsWith('.dart')) continue;
      final String source;
      try {
        source = entry.readAsStringSync();
      } on IOException {
        unresolved.add(entry.path);
        continue;
      }
      if (!source.contains('package:pdf_manipulator/')) continue;
      for (final e in patterns.entries) {
        if (e.value.hasMatch(source)) {
          keep.addAll(byMember[e.key]!);
          matched.add(e.key);
        }
      }
    }
  }

  return DetectorResult(
    keep: keep,
    resolved: unresolved.isEmpty,
    unresolvedPaths: unresolved,
    matchedMembers: matched,
  );
}
