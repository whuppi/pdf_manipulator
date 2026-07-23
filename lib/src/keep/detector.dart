// The stable keep detector: a dependency-free text scan over the app's
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
// exact minimum state `keep: [...]`.
//
// Deliberately NOT built on package:analyzer: a runtime package must
// never put the analyzer in a consumer's dependency graph (it fights
// the app's own codegen/lint tooling — issue #171). Do not reintroduce
// it here.

import 'dart:io';

import 'package:path/path.dart' as p;

import 'package:pdf_manipulator/src/keep/capabilities.dart';

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
    required this.matchSites,
  });

  /// Capabilities the app can reach. Meaningful only when [resolved].
  final Set<PdfCapability> keep;

  /// True when every scanned file was readable — the keep-set is proven.
  final bool resolved;

  /// Files the scan could not read (drives the fail-closed path).
  final List<String> unresolvedPaths;

  /// The member names that matched, for `compare` mode and logs.
  final Set<String> matchedMembers;

  /// First place each matched member was seen, member → `path:line`
  /// (path relative to the app root) — the diagnostic for "why is this
  /// capability kept".
  final Map<String, String> matchSites;
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
  final sites = <String, String>{};
  for (final path in visible) {
    // Match against comment-blanked source: a member name in a `//` or
    // `/* */` comment is prose, not a call site, and kept capabilities
    // it caused were pure over-keep (issue #175). Blanking preserves
    // offsets, so match positions map straight to source lines.
    final source = _blankComments(sources[path]!);
    for (final e in patterns.entries) {
      final m = e.value.firstMatch(source);
      if (m != null) {
        keep.addAll(byMember[e.key]!);
        matched.add(e.key);
        sites.putIfAbsent(e.key, () {
          final line = '\n'.allMatches(source.substring(0, m.start)).length + 1;
          return '${p.relative(path, from: appRoot)}:$line';
        });
      }
    }
  }

  return DetectorResult(
    keep: keep,
    resolved: unresolved.isEmpty,
    unresolvedPaths: unresolved,
    matchedMembers: matched,
    matchSites: sites,
  );
}

/// Renders [result]'s matches as `member (path:line)` entries, sorted —
/// the shared shape for "why is this capability kept" output.
String describeMatches(DetectorResult result) {
  final members = result.matchedMembers.toList()..sort();
  return members
      .map(
        (m) => result.matchSites.containsKey(m)
            ? '$m (${result.matchSites[m]})'
            : m,
      )
      .join(', ');
}

/// Replaces every comment in Dart [source] with same-length whitespace
/// (newlines kept), leaving all other text — string contents included —
/// verbatim. Comments cannot contain call sites, so blanking them can
/// only remove over-keep; strings stay because interpolations carry real
/// calls (`'${doc.render(0)}'`) and dropping them would under-keep.
String _blankComments(String source) => _CommentBlanker(source).run();

class _CommentBlanker {
  _CommentBlanker(this.src);

  final String src;
  final StringBuffer out = StringBuffer();
  int i = 0;

  String run() {
    _code(insideInterpolation: false);
    return out.toString();
  }

  String? _peek(int ahead) => i + ahead < src.length ? src[i + ahead] : null;

  void _blank(String char) => out.write(char == '\n' ? '\n' : ' ');

  /// Lexes code. Inside a `${...}` interpolation, returns at the
  /// matching `}` (consumed by the caller's string state).
  void _code({required bool insideInterpolation}) {
    var braceDepth = 0;
    while (i < src.length) {
      final c = src[i];
      if (insideInterpolation) {
        if (c == '{') braceDepth++;
        if (c == '}') {
          if (braceDepth == 0) return;
          braceDepth--;
        }
      }
      if (c == '/' && _peek(1) == '/') {
        while (i < src.length && src[i] != '\n') {
          _blank(src[i]);
          i++;
        }
      } else if (c == '/' && _peek(1) == '*') {
        _blockComment();
      } else if (c == "'" || c == '"') {
        _string(raw: false);
      } else if (c == 'r' && (_peek(1) == "'" || _peek(1) == '"')) {
        out.write('r');
        i++;
        _string(raw: true);
      } else {
        out.write(c);
        i++;
      }
    }
  }

  /// `/* */` blocks nest in Dart — track depth, don't stop at the
  /// first `*/`.
  void _blockComment() {
    var depth = 0;
    while (i < src.length) {
      if (src[i] == '/' && _peek(1) == '*') {
        depth++;
        _blank(src[i]);
        _blank(src[i + 1]);
        i += 2;
      } else if (src[i] == '*' && _peek(1) == '/') {
        depth--;
        _blank(src[i]);
        _blank(src[i + 1]);
        i += 2;
        if (depth == 0) return;
      } else {
        _blank(src[i]);
        i++;
      }
    }
  }

  /// Copies a string literal verbatim so `//` inside it is never taken
  /// for a comment. Handles `'`/`"`, their `'''`/`"""` triple forms,
  /// escapes, and `${...}` interpolations (recursing into code state so
  /// quotes and comments inside the expression lex correctly). Raw
  /// strings have neither escapes nor interpolation. An unterminated
  /// literal consumes to EOF — every char stays matchable either way.
  void _string({required bool raw}) {
    final quote = src[i];
    final triple = _peek(1) == quote && _peek(2) == quote;
    final closer = triple ? quote * 3 : quote;
    out.write(closer);
    i += closer.length;

    while (i < src.length) {
      if (src.startsWith(closer, i)) {
        out.write(closer);
        i += closer.length;
        return;
      }
      final c = src[i];
      if (!triple && c == '\n') {
        // Dart's grammar ends single-line literals at the newline; a
        // stray quote must not put the rest of the file in string state.
        return;
      }
      if (!raw && c == r'\' && i + 1 < src.length) {
        out.write(src[i]);
        out.write(src[i + 1]);
        i += 2;
      } else if (!raw && c == r'$' && _peek(1) == '{') {
        out.write(r'${');
        i += 2;
        _code(insideInterpolation: true);
        if (i < src.length) {
          out.write('}');
          i++;
        }
      } else {
        out.write(c);
        i++;
      }
    }
  }
}

// A whole directive, keyword to semicolon. Its body may carry several
// URIs (conditional imports: `import 'a.dart' if (x) 'b.dart';`) —
// every one is a potential path to the API, so all are extracted.
final _directive = RegExp(
  r'''^\s*(import|export)\s+([^;]*);''',
  multiLine: true,
);
final _directiveUri = RegExp(r'''['"]([^'"]+)['"]''');

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
  // package:<appName>/x.dart resolves into <appRoot>/lib/x.dart. YAML
  // allows a quoted name (name: "my_app") — the quotes are syntax, not
  // part of the package name, so strip them or the prefix never matches.
  final rawName = RegExp(
    r'^name:\s*(\S+)',
    multiLine: true,
  ).firstMatch(pubspec ?? '')?[1];
  final appName = rawName != null && (rawName[0] == '"' || rawName[0] == "'")
      ? rawName.substring(1, rawName.length - 1)
      : rawName;
  final selfPrefix = appName == null ? null : 'package:$appName/';

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
      for (final u in _directiveUri.allMatches(d[2]!)) {
        final uri = u[1]!;
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
