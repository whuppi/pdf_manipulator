// Link hook — forwards every native asset to the app bundle unchanged,
// and (EXPERIMENTAL) reports @RecordUse-recorded capabilities.
//
// Do NOT delete this file as "dead code": on release/AOT builds Flutter
// routes native assets through the link hook (build.dart emits them
// ToLinkHook), and a missing hook fails the build. Debug builds bypass it
// (assets route ToAppBundle); web assets never pass through here.
//
// The RecordUse report exists so the experimental detector can be observed
// against the analyzer's keep-set (trim-detector: compare) while the SDK
// lane matures. It never modifies assets — usage data arrives here AFTER
// the native library was already built, so it cannot drive trimming yet.

import 'dart:io';

import 'package:hooks/hooks.dart';
import 'package:pdf_manipulator/src/trim/capabilities.dart';
import 'package:record_use/record_use.dart';

void main(List<String> args) async {
  await link(args, (LinkInput input, LinkOutputBuilder output) async {
    for (final asset in input.assets.encodedAssets) {
      output.assets.addEncodedAsset(asset);
    }
    _reportRecordedCapabilities(input);
  });
}

/// Prints the capability set the compiler recorded via `TrimRecord.op`
/// calls, when the SDK produced recordings (requires the record-use
/// experiment). Diagnostics only — see the library comment.
void _reportRecordedCapabilities(LinkInput input) {
  final detector = TrimDetector.parse(input.userDefines['trim-detector']);
  if (detector == TrimDetector.analyzer) return;

  // This whole lane is opt-in experimental; consuming the hooks package's
  // experimental surface is the deliberate choice, not an accident.
  // ignore: experimental_member_use
  final recordings = input.recordedUses;
  if (recordings == null) {
    stdout.writeln(
      'pdf_manipulator trim-detector ${detector.wire}: no recorded usages '
      'in this build (the SDK record-use experiment is not active) — '
      'nothing to report.',
    );
    return;
  }
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
  final capabilities = recorded.map(PdfCapability.byWire).nonNulls.toSet();
  stdout.writeln(
    'pdf_manipulator trim-detector ${detector.wire} (EXPERIMENTAL): '
    'RecordUse observed capabilities '
    '{${capabilities.map((c) => c.wire).join(', ')}}. '
    'Diff this against the analyzer keep-set the build used.',
  );
}
