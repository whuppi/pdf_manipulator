// Turns the `keep` / `detector` user-defines into a build decision,
// and recorded `@RecordUse` data into a keep-set. Shared by hook/build.dart
// (source-scan lane, and the defer decision for the RecordUse lane) and
// hook/link.dart (the RecordUse lane's trim itself).

import 'dart:io';

import 'package:record_use/record_use.dart';

import 'package:pdf_manipulator/src/keep/capabilities.dart';
import 'package:pdf_manipulator/src/keep/detector.dart';

/// Which capabilities the build hook keeps, resolved from the config.
/// [PdfConfigError]s from malformed defines propagate — a config mistake
/// must fail the build loudly.
class KeepPlan {
  const KeepPlan._({
    required this.features,
    required this.isCustom,
    required this.detector,
    required this.deferToLink,
    this.appDependencies = const [],
  });

  /// The cargo feature list the build hook compiles (or downloads) with.
  final String features;

  /// True when [features] differs from the defaults — no prebuilt release
  /// asset exists for it, so the resolver must compile locally.
  final bool isCustom;

  /// Which detector the user selected.
  final KeepDetector detector;

  /// The app paths this decision was derived from. A build hook MUST
  /// register these via `output.dependencies`, or the runner caches the
  /// decision and never re-runs the hook when the app changes — an app that
  /// starts calling a trimmed-away capability would keep building without
  /// it. Empty for every mode that reads no app source.
  final List<Uri> appDependencies;

  /// True for `keep: auto` + `detector: record-use`: the build hook
  /// ships the FULL binary and the link hook trims it on release builds
  /// (recorded usage data only exists after AOT compilation).
  final bool deferToLink;
}

/// Resolves the build hook's keep decision from the already-parsed [keep]
/// config and [detector] (both come pre-validated from
/// `PdfManipulatorConfig`). [appRootCandidate] is where `keep: auto`'s source
/// scan looks for the app. The hooks API exposes no app root, so callers
/// derive it (see `appRootFromDartTool`); null, or a directory without a
/// pubspec.yaml, fails CLOSED to the full binary.
///
/// [scanDirs] are the app-relative directories from the `scan-dirs` config,
/// scanned on top of the app's `lib/` and `bin/`. One that does not exist
/// also fails CLOSED — a typo must not quietly narrow the scan.
Future<KeepPlan> resolveKeepPlan({
  required KeepConfig keep,
  required KeepDetector detector,
  required String defaultFeatures,
  required String? appRootCandidate,
  required List<String> scanDirs,
}) async {
  Set<PdfCapability>? keepSet;
  var deferToLink = false;
  var appDependencies = const <Uri>[];

  switch (keep.mode) {
    case KeepMode.all:
      break;
    case KeepMode.manual:
      keepSet = keep.keep;
    case KeepMode.auto:
      if (detector == KeepDetector.recordUse) {
        // The RecordUse lane (EXPERIMENTAL): recordings appear after AOT
        // compilation, so the trim happens in the link hook. Debug builds
        // skip linking entirely and stay on the full binary by design.
        deferToLink = true;
        break;
      }
      // Null means the app could not be located at all; a root without a
      // pubspec means the candidate is not a Dart package. Both are "we do
      // not know what the app uses", and both must build everything — a
      // directory that merely is not the app scans perfectly cleanly and
      // would otherwise produce a confident, wrong trim.
      if (appRootCandidate == null ||
          !File('$appRootCandidate/pubspec.yaml').existsSync()) {
        stderr.writeln(
          'pdf_manipulator keep: cannot locate the app source from the '
          'build hook — keeping the FULL binary (fail closed). Use '
          'keep: [...] for a guaranteed native trim.',
        );
        break;
      }
      // scan-dirs are app-relative by contract (PdfManipulatorConfig rejects
      // absolute ones), so they resolve against the app root the same way
      // `lib/` does — the hook and the web setup script then agree.
      final result = detectCapabilities(
        appRootCandidate,
        extraDirs: [for (final d in scanDirs) '$appRootCandidate/$d'],
      );
      appDependencies = [
        Uri.file('$appRootCandidate/pubspec.yaml'),
        // Every directory catches a file appearing or disappearing in IT —
        // the runner hashes immediate child names, so a parent never sees a
        // change inside a subdirectory...
        for (final dir in result.scannedDirs) Uri.directory(dir),
        // ...and the files themselves catch edits to existing ones.
        for (final file in result.scannedFiles) Uri.file(file),
      ];
      if (!result.resolved) {
        stderr.writeln(
          'pdf_manipulator keep: ${result.unresolvedPaths.length} path(s) '
          'could not be read — keeping the FULL binary (fail closed).',
        );
        break;
      }
      stdout.writeln(
        'pdf_manipulator keep (scan): keeping '
        '{${describeMatches(result)}}',
      );
      keepSet = result.keep;
  }

  final features = keepSet == null
      ? defaultFeatures
      : keep.featuresFor(defaultFeatures, keepSet);
  return KeepPlan._(
    features: features,
    isCustom: features != defaultFeatures,
    detector: detector,
    deferToLink: deferToLink,
    appDependencies: appDependencies,
  );
}

/// The capabilities the compiler recorded as reachable, extracted from
/// `KeepRecord.op('<capability>')` call sites that survived Dart's own
/// tree shake. Unknown names are ignored (a newer engine may record
/// capabilities this package version doesn't know).
Set<PdfCapability> recordedCapabilities(Recordings recordings) {
  final recorded = <String>{};
  for (final entry in recordings.calls.entries) {
    final parent = entry.key.parent;
    if (entry.key.name != 'op' ||
        parent is! Class ||
        parent.name != 'KeepRecord') {
      continue;
    }
    for (final call in entry.value) {
      if (call is CallWithArguments) {
        for (final arg in call.positionalArguments) {
          if (arg is StringConstant) recorded.add(arg.value);
        }
      }
    }
  }
  return recorded.map(PdfCapability.byWire).nonNulls.toSet();
}
