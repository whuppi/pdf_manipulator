// The one parser for pdf_manipulator's `hooks: user_defines:` block. Both
// callers — the native build hook (`BuildInput.userDefines`) and the web
// setup script (which reads the pubspec itself) — go through here, so web
// and native can never diverge on what a config means.
//
// ═══════════════════════════════════════════════════════════════════════
// INVALID CONFIGS ARE UNREPRESENTABLE BY DESIGN — preserve this forever.
//
// Every option is a closed set of values (`keep`, `detector`, `build` each
// parse a fixed grammar and throw on anything else). When one option only
// makes sense given another's value — `detector` is meaningful ONLY when
// `keep: auto` — that coupling is VALIDATED here, and the build STOPS with a
// clear message. Nothing is ever silently ignored: a stranded `detector`, an
// unknown key, a bad value all fail loudly.
//
// The design law for any future knob:
//   • Two options are independent axes only if EVERY combination is valid —
//     then both are top-level and any pairing is fine (`keep` × `build`).
//   • If an option's validity depends on another's value, it is NOT a
//     separate axis: validate the pair here (or nest it so the bad combo is
//     unwritable). Never leave it as a silently-ignorable sibling.
//   • Group by nesting ONLY when the nesting itself makes a bad state
//     unwritable. The enclosing `pdf_manipulator:` key is already the
//     namespace; don't add cosmetic parent keys.
// If it gets past this parser, it is a valid config.
// ═══════════════════════════════════════════════════════════════════════

import 'package:pdf_manipulator/src/hook/engine_build.dart';
import 'package:pdf_manipulator/src/keep/capabilities.dart';

/// The fully-parsed, validated `pdf_manipulator:` user-defines block.
class PdfManipulatorConfig {
  /// Creates a validated config from its three parsed axes.
  const PdfManipulatorConfig({
    required this.keep,
    required this.detector,
    required this.build,
  });

  /// What capabilities the engine keeps ([KeepConfig.all] / a manual set /
  /// auto-detect).
  final KeepConfig keep;

  /// Which machine computes the keep-set when [keep] is [KeepMode.auto].
  /// Parsed always (defaults to [KeepDetector.scan]); only meaningful — and
  /// only permitted to be set — when [keep] is auto.
  final KeepDetector detector;

  /// What the engine is optimized for ([EngineBuild.speed] / size / debug).
  final EngineBuild build;

  /// The only keys allowed under `pdf_manipulator:`. Anything else is a typo
  /// or a stranded option and fails the build (where the key set is visible —
  /// see [parse]).
  static const knownKeys = {'keep', 'detector', 'build'};

  /// Parses [defines] (the raw `pdf_manipulator:` map). Throws [PdfConfigError]
  /// on any invalid value, unknown key, or axis mismatch.
  ///
  /// [defines] is the full block on web (read from the pubspec, so unknown
  /// keys are caught here). On native the build-hook API exposes values by
  /// key but not the key set, so the caller passes a map of only the known
  /// keys — the unknown-key check then trivially passes, and native leans on
  /// the value-grammar + axis-mismatch checks, which work identically.
  static PdfManipulatorConfig parse(Map<String, Object?> defines) {
    final unknown = defines.keys.where((k) => !knownKeys.contains(k)).toList()
      ..sort();
    if (unknown.isNotEmpty) {
      throw PdfConfigError(
        'unknown pdf_manipulator config key(s): ${unknown.join(', ')}.\n'
        'Valid keys: ${knownKeys.join(', ')}.',
      );
    }

    final keep = KeepConfig.parse(defines['keep']);
    final detector = KeepDetector.parse(defines['detector']);
    final build = EngineBuild.parse(defines['build']);

    // The one cross-axis coupling: `detector` picks HOW `auto` detects, so it
    // means nothing unless `keep` is auto. Setting it otherwise is a mistake —
    // stop the build rather than silently ignore it.
    if (defines.containsKey('detector') && keep.mode != KeepMode.auto) {
      throw PdfConfigError(
        'detector: only applies to `keep: auto` '
        '(keep is currently ${keep.mode.name}).\n'
        'Either use `keep: auto`, or remove the `detector:` line.',
      );
    }

    return PdfManipulatorConfig(keep: keep, detector: detector, build: build);
  }
}
