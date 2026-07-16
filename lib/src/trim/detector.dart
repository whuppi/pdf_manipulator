// The stable trim detector: resolved-AST reachability over the app's
// source. Conservative by contract — any file that fails to resolve makes
// the scan fail CLOSED (full binary), never a guess. Tooling-only: imported
// by bin/setup.dart and the build hook, never by the library barrel, so
// package:analyzer adds no bytes to any app.

import 'dart:io';

import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/ast/visitor.dart';
import 'package:analyzer/dart/element/element.dart';

import 'package:pdf_manipulator/src/trim/capabilities.dart';

/// What a scan concluded. [resolved] false means the scan could not prove
/// reachability (unanalyzable files) — callers MUST fall back to the full
/// binary and surface [unresolvedPaths].
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

  /// True when every scanned file resolved — the keep-set is proven.
  final bool resolved;

  /// Files the analyzer could not resolve (drives the fail-closed path).
  final List<String> unresolvedPaths;

  /// The `Class.member` names that matched, for `compare` mode and logs.
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
  final collection = AnalysisContextCollection(
    includedPaths: roots.map((r) => Directory(r).absolute.path).toList(),
  );

  final keep = <PdfCapability>{};
  final matched = <String>{};
  final unresolved = <String>[];

  for (final context in collection.contexts) {
    for (final path in context.contextRoot.analyzedFiles()) {
      if (!path.endsWith('.dart')) continue;
      final unit = await context.currentSession.getResolvedUnit(path);
      if (unit is! ResolvedUnitResult) {
        unresolved.add(path);
        continue;
      }
      unit.unit.accept(_MemberFinder(keep, matched));
    }
  }

  return DetectorResult(
    keep: keep,
    resolved: unresolved.isEmpty,
    unresolvedPaths: unresolved,
    matchedMembers: matched,
  );
}

class _MemberFinder extends RecursiveAstVisitor<void> {
  _MemberFinder(this.keep, this.matched);

  final Set<PdfCapability> keep;
  final Set<String> matched;

  void _record(Element? element) {
    if (element == null) return;
    final lib = element.library;
    if (lib == null) return;
    if (!lib.uri.toString().startsWith('package:pdf_manipulator/')) return;
    final enclosing = element.enclosingElement;
    final key = enclosing is InterfaceElement
        ? '${enclosing.name}.${element.name}'
        : '${element.name}';
    final cap = PdfCapability.apiMembers[key];
    if (cap != null) {
      keep.add(cap);
      matched.add(key);
    }
  }

  @override
  void visitMethodInvocation(MethodInvocation node) {
    _record(node.methodName.element);
    super.visitMethodInvocation(node);
  }

  @override
  void visitPropertyAccess(PropertyAccess node) {
    _record(node.propertyName.element);
    super.visitPropertyAccess(node);
  }

  @override
  void visitPrefixedIdentifier(PrefixedIdentifier node) {
    _record(node.identifier.element);
    super.visitPrefixedIdentifier(node);
  }
}
