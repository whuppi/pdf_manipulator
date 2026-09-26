// keep_sets.dart — every valid keep-set for one compile target.
//
//   dart tool/keep_sets.dart native
//   dart tool/keep_sets.dart wasm
//
// A valid keep-set is a subset S of PdfCapability.values closed under
// requires (PdfCapability.expandRequires(S) == S) — the set of keep-lists a
// user could actually write. This is the one home of that list; the
// requires graph itself stays in capabilities.dart. tool/check_keep_sets.sh
// consumes this to compile every one against the vendored engine.
//
// Prints one line per set, ordered by size then enum order:
//   <keep>\t<features>
// <keep> is the YAML keep list as a user writes it (`[]`, `[render,
// extract]`, ...); <features> is the cargo feature string
// KeepConfig.all.featuresFor produces for that set against the target's
// build.json features.

import 'dart:io';

import 'package:pdf_manipulator/src/hook/build_constants.dart';
import 'package:pdf_manipulator/src/keep/capabilities.dart';

final Uri _pkgRoot = Directory.current.uri;

Future<void> main(List<String> args) async {
  if (args.length != 1 || (args.first != 'native' && args.first != 'wasm')) {
    stderr.writeln('usage: dart tool/keep_sets.dart {native|wasm}');
    exit(64);
  }

  final c = BuildConstants.load(_pkgRoot);
  final defaultFeatures = args.first == 'native'
      ? c.nativeFeatures
      : c.wasmFeatures;

  final sets = _validKeepSets();
  for (final set in sets) {
    final keep = PdfCapability.values
        .where(set.contains)
        .map((cap) => cap.wire)
        .join(', ');
    final features = KeepConfig.all.featuresFor(defaultFeatures, set);
    stdout.writeln('[$keep]\t$features');
  }
}

/// Every subset of [PdfCapability.values] closed under
/// [PdfCapability.expandRequires], ordered by size then enum order.
List<Set<PdfCapability>> _validKeepSets() {
  final caps = PdfCapability.values;
  final valid = <Set<PdfCapability>>[];
  for (var mask = 0; mask < (1 << caps.length); mask++) {
    final set = <PdfCapability>{};
    for (var i = 0; i < caps.length; i++) {
      if (mask & (1 << i) != 0) set.add(caps[i]);
    }
    if (PdfCapability.expandRequires(set).length == set.length) {
      valid.add(set);
    }
  }
  valid.sort((a, b) {
    final bySize = a.length.compareTo(b.length);
    if (bySize != 0) return bySize;
    for (var i = 0; i < caps.length; i++) {
      final inA = a.contains(caps[i]);
      final inB = b.contains(caps[i]);
      if (inA != inB) return inA ? -1 : 1;
    }
    return 0;
  });
  return valid;
}
