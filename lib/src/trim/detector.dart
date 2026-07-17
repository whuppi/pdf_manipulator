// The stable trim detector: a dependency-free text scan over the app's
// source. Files that can see pdf_manipulator's API — a direct import,
// or an import of an app file that re-exports it (barrel files, tracked
// transitively) — are searched for capability member names.
//
// The scan errs toward OVER-keeping (a same-named identifier in a
// scanned file keeps a capability the app never calls) — the app always
// works, the binary is just less trimmed. It does not under-keep: a
// call site always spells the member name in the text, including
// dynamic calls that resolution-based analysis silently skips, and the
// re-export tracking covers barrel-mediated usage. Users who want the
// exact minimum state `trim: {keep: [...]}`.
//
// Deliberately NOT built on package:analyzer: a runtime package must
// never put the analyzer in a consumer's dependency graph (it fights
// the app's own codegen/lint tooling — issue #171). Do not reintroduce
// it here.

import 'dart:io';

import 'package:path/path.dart' as p;

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
DetectorResult detectCapabilities(
  String appRoot, {
  List<String> extraDirs = const [],
}) {
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

  final unresolved = <String>[];
  final sources = <String, String>{}; // canonical path → contents

  // Manual walk: an unreadable directory records itself and skips its
  // subtree instead of aborting the whole listing with a throw —
  // filesystem-level failures fall closed like unreadable files do.
  void walk(Directory dir) {
    final List<FileSystemEntity> entries;
    try {
      entries = dir.listSync(followLinks: false);
    } on IOException {
      unresolved.add(dir.path);
      return;
    }
    for (final entry in entries) {
      if (entry is Directory) {
        walk(entry);
      } else if (entry is File && entry.path.endsWith('.dart')) {
        try {
          sources[p.canonicalize(entry.path)] = entry.readAsStringSync();
        } on IOException {
          unresolved.add(entry.path);
        }
      }
    }
  }

  for (final root in roots) {
    walk(Directory(root));
  }

  // Without the pubspec there is no app name, so package:<self>/ barrel
  // imports cannot be resolved — a possible under-keep. Report it as
  // unresolved: the caller falls back to the full binary.
  final pubspec = _tryRead('$appRoot/pubspec.yaml');
  if (pubspec == null) {
    unresolved.add('$appRoot/pubspec.yaml');
  }

  final visible = _filesSeeingApi(appRoot, pubspec, sources);

  final keep = <PdfCapability>{};
  final matched = <String>{};
  for (final path in visible) {
    final source = sources[path]!;
    for (final e in patterns.entries) {
      if (e.value.hasMatch(source)) {
        keep.addAll(byMember[e.key]!);
        matched.add(e.key);
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

final _directive = RegExp(
  r'''^\s*(import|export)\s+['"]([^'"]+)['"]''',
  multiLine: true,
);

/// The files that can see pdf_manipulator's API: a direct import or
/// export, or an import/export of an app file that RE-EXPORTS it,
/// tracked transitively (barrel files). [pubspec] resolves
/// `package:<self>/` imports; when it is null the caller has already
/// recorded the pubspec as unresolved (full-binary fallback).
Set<String> _filesSeeingApi(
  String appRoot,
  String? pubspec,
  Map<String, String> sources,
) {
  // package:<appName>/x.dart resolves into <appRoot>/lib/x.dart.
  final nameMatch = RegExp(
    r'^name:\s*(\S+)',
    multiLine: true,
  ).firstMatch(pubspec ?? '');
  final selfPrefix = nameMatch == null ? null : 'package:${nameMatch[1]}/';

  String? resolve(String fromFile, String uri) {
    if (selfPrefix != null && uri.startsWith(selfPrefix)) {
      return p.canonicalize(
        p.join(appRoot, 'lib', uri.substring(selfPrefix.length)),
      );
    }
    if (!uri.contains(':')) {
      return p.canonicalize(p.join(p.dirname(fromFile), uri));
    }
    return null; // other packages / dart: — not app files
  }

  // Parse directives once; build the reverse export graph so exposure
  // propagates as a worklist from the direct exporters — O(files + edges)
  // instead of re-scanning every file per fixpoint round.
  final direct = <String>{}; // imports OR exports pdf_manipulator itself
  final dependsOn = <String, Set<String>>{}; // file → app files it pulls in
  final exportedBy = <String, List<String>>{}; // target → files exporting it
  final seeds = <String>[]; // files exporting pdf_manipulator directly

  for (final e in sources.entries) {
    for (final d in _directive.allMatches(e.value)) {
      final uri = d[2]!;
      if (uri.startsWith('package:pdf_manipulator/')) {
        direct.add(e.key);
        if (d[1] == 'export') seeds.add(e.key);
        continue;
      }
      final target = resolve(e.key, uri);
      if (target == null) continue;
      (dependsOn[e.key] ??= {}).add(target);
      if (d[1] == 'export') {
        (exportedBy[target] ??= []).add(e.key);
      }
    }
  }

  // A file re-exposes the API if it exports pdf_manipulator directly,
  // or exports a file that re-exposes it.
  final exposes = <String>{};
  final worklist = [...seeds];
  while (worklist.isNotEmpty) {
    final file = worklist.removeLast();
    if (!exposes.add(file)) continue;
    worklist.addAll(exportedBy[file] ?? const []);
  }

  return {
    for (final e in sources.entries)
      if (direct.contains(e.key) ||
          (dependsOn[e.key]?.any(exposes.contains) ?? false))
        e.key,
  };
}

String? _tryRead(String path) {
  try {
    return File(path).readAsStringSync();
  } on IOException {
    return null;
  }
}
