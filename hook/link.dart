// Link hook — the RecordUse trim lane (EXPERIMENTAL), plus passthrough.
//
// Do NOT delete this file as "dead code": on release/AOT builds Flutter
// routes native assets through the link hook (build.dart emits them
// ToLinkHook), and a missing hook fails the build. Debug builds bypass it
// (assets route ToAppBundle); web assets never pass through here.
//
// With `trim: auto` + `trim-detector: record-use`, the build hook ships
// the FULL engine here and THIS hook trims it: the SDK's `@RecordUse`
// recordings (which capabilities survived Dart's own tree shake) become a
// keep-set, the shared engine compiler builds the trimmed library, and it
// replaces the full one in the bundle. Recordings absent (the SDK
// experiment is not active) → the full binary ships unchanged, loudly
// (fail closed). `compare` never modifies assets — it reports the
// recorded keep-set so it can be diffed against the scan's.

import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:hooks/hooks.dart';
import 'package:path/path.dart' as p;

import 'package:pdf_manipulator/src/hook/build_constants.dart';
import 'package:pdf_manipulator/src/hook/engine_compiler.dart';
import 'package:pdf_manipulator/src/hook/trim_plan.dart';
import 'package:pdf_manipulator/src/trim/capabilities.dart';

void main(List<String> args) async {
  await link(args, (LinkInput input, LinkOutputBuilder output) async {
    final detector = TrimDetector.parse(input.userDefines['trim-detector']);
    final config = TrimConfig.parse(input.userDefines['trim']);
    final driving =
        detector == TrimDetector.recordUse && config.mode == TrimMode.auto;

    for (final asset in input.assets.encodedAssets) {
      if (driving && asset.isCodeAsset) {
        final trimmed = await _trimFromRecordings(
          input,
          CodeAsset.fromEncoded(asset),
        );
        output.assets.code.add(trimmed);
        continue;
      }
      output.assets.addEncodedAsset(asset);
    }

    if (detector == TrimDetector.compare) {
      _reportRecordedCapabilities(input);
    }
  });
}

/// Rebuilds [full]'s library with only the capabilities the compiler
/// recorded as reachable, or returns [full] unchanged when there is
/// nothing to prove (no recordings) or nothing to drop.
Future<CodeAsset> _trimFromRecordings(LinkInput input, CodeAsset full) async {
  final recordings = input.recordedUses;
  if (recordings == null) {
    stderr.writeln(
      'pdf_manipulator trim-detector record-use: no recorded usages in '
      'this build (the SDK record-use experiment is not active) — '
      'shipping the FULL binary (fail closed).',
    );
    return full;
  }

  final constants = BuildConstants.load(input.packageRoot);
  final keep = recordedCapabilities(recordings);
  final features = TrimConfig.keep(
    keep,
  ).featuresFor(constants.nativeFeatures, keep);
  if (features == constants.nativeFeatures) {
    stdout.writeln(
      'pdf_manipulator trim (record-use): every capability is reachable — '
      'the full binary already is the trimmed binary.',
    );
    return full;
  }

  stdout.writeln(
    'pdf_manipulator trim (record-use, EXPERIMENTAL): recorded keep-set '
    '{${(keep.map((c) => c.wire).toList()..sort()).join(', ')}} '
    '→ features [$features] — rebuilding the engine.',
  );

  // CodeAsset.file is nullable in the hooks API; without a library file
  // there is nothing to rebuild — ship the asset unchanged (fail closed).
  final fullFile = full.file;
  if (fullFile == null) {
    stderr.writeln(
      'pdf_manipulator trim (record-use): incoming code asset ${full.id} '
      'carries no file — shipping it unchanged.',
    );
    return full;
  }

  final codeConfig = input.config.code;
  final targetTriple = targetTripleFor(codeConfig);
  final libFileName = p.basename(p.fromUri(fullFile));
  final outFile = File.fromUri(input.outputDirectory.resolve(libFileName));

  await compileEngineForTarget(
    packageRoot: input.packageRoot,
    crateName: constants.crate,
    targetTriple: targetTriple,
    features: features,
    // Per-asset cargo dir: two code assets in one link run must not share
    // build state, or the second compile clobbers the first's artifact.
    targetDir: p.join(
      p.fromUri(input.outputDirectory),
      'cargo_target',
      p.basenameWithoutExtension(libFileName),
    ),
    libFileName: libFileName,
    outFile: outFile,
    environment: androidLinkerEnv(codeConfig, targetTriple),
  );

  // CodeAsset.id is 'package:<package>/<name>' — rebuild both parts so the
  // emitted asset keeps the identity @DefaultAsset resolves against.
  final idPath = full.id.substring('package:'.length);
  return CodeAsset(
    package: idPath.split('/').first,
    name: idPath.split('/').skip(1).join('/'),
    linkMode: full.linkMode,
    file: outFile.uri,
  );
}

/// Prints the capability set the compiler recorded via `TrimRecord.op`
/// calls, when the SDK produced recordings. Diagnostics only — lets the
/// RecordUse lane be diffed against the scan while it matures.
void _reportRecordedCapabilities(LinkInput input) {
  final recordings = input.recordedUses;
  if (recordings == null) {
    stdout.writeln(
      'pdf_manipulator trim-detector compare: no recorded usages in this '
      'build (the SDK record-use experiment is not active) — nothing to '
      'diff.',
    );
    return;
  }
  final capabilities = recordedCapabilities(recordings);
  stdout.writeln(
    'pdf_manipulator trim-detector compare (EXPERIMENTAL): RecordUse '
    'observed capabilities '
    '{${(capabilities.map((c) => c.wire).toList()..sort()).join(', ')}}. '
    'Diff this against the scan keep-set the build used.',
  );
}
