// Turns the `trim` / `trim-detector` user-defines into a build decision,
// and recorded `@RecordUse` data into a keep-set. Shared by hook/build.dart
// (source-scan lane, and the defer decision for the RecordUse lane) and
// hook/link.dart (the RecordUse lane's trim itself).

import 'dart:io';

import 'package:record_use/record_use.dart';

import 'package:pdf_manipulator/src/trim/capabilities.dart';
import 'package:pdf_manipulator/src/trim/detector.dart';

/// What the build hook should do about trimming, resolved from the
/// user-defines. [TrimConfigError]s from malformed defines propagate —
/// a config mistake must fail the build loudly.
class TrimPlan {
  const TrimPlan._({
    required this.features,
    required this.isCustom,
    required this.detector,
    required this.deferToLink,
  });

  /// The cargo feature list the build hook compiles (or downloads) with.
  final String features;

  /// True when [features] differs from the defaults — no prebuilt release
  /// asset exists for it, so the resolver must compile locally.
  final bool isCustom;

  /// Which detector the user selected.
  final TrimDetector detector;

  /// True for `trim: auto` + `trim-detector: record-use`: the build hook
  /// ships the FULL binary and the link hook trims it on release builds
  /// (recorded usage data only exists after AOT compilation).
  final bool deferToLink;
}

/// Resolves the build hook's trim decision. [appRootCandidate] is where
/// `trim: auto`'s source scan looks for the app (the hooks API exposes
/// no app root, so callers pass their best heuristic — a directory
/// without a pubspec.yaml fails CLOSED to the full binary).
Future<TrimPlan> resolveTrimPlan({
  required Object? trimDefine,
  required Object? detectorDefine,
  required String defaultFeatures,
  required String appRootCandidate,
}) async {
  final config = TrimConfig.parse(trimDefine);
  final detector = TrimDetector.parse(detectorDefine);

  Set<PdfCapability>? keep;
  var deferToLink = false;

  switch (config.mode) {
    case TrimMode.off:
      break;
    case TrimMode.manual:
      keep = config.keep;
    case TrimMode.auto:
      if (detector == TrimDetector.recordUse) {
        // The RecordUse lane (EXPERIMENTAL): recordings appear after AOT
        // compilation, so the trim happens in the link hook. Debug builds
        // skip linking entirely and stay on the full binary by design.
        deferToLink = true;
        break;
      }
      if (!File('$appRootCandidate/pubspec.yaml').existsSync()) {
        stderr.writeln(
          'pdf_manipulator trim: cannot locate the app source from the '
          'build hook — keeping the FULL binary (fail closed). Use '
          'trim: {keep: [...]} for a guaranteed native trim.',
        );
        break;
      }
      final result = await detectCapabilities(appRootCandidate);
      if (!result.resolved) {
        stderr.writeln(
          'pdf_manipulator trim: ${result.unresolvedPaths.length} file(s) '
          'could not be read — keeping the FULL binary (fail closed).',
        );
        break;
      }
      keep = result.keep;
  }

  final features = keep == null
      ? defaultFeatures
      : config.featuresFor(defaultFeatures, keep);
  return TrimPlan._(
    features: features,
    isCustom: features != defaultFeatures,
    detector: detector,
    deferToLink: deferToLink,
  );
}

/// The capabilities the compiler recorded as reachable, extracted from
/// `TrimRecord.op('<capability>')` call sites that survived Dart's own
/// tree shake. Unknown names are ignored (a newer engine may record
/// capabilities this package version doesn't know).
Set<PdfCapability> recordedCapabilities(Recordings recordings) {
  final recorded = <String>{};
  for (final entry in recordings.calls.entries) {
    final parent = entry.key.parent;
    if (entry.key.name != 'op' ||
        parent is! Class ||
        parent.name != 'TrimRecord') {
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
