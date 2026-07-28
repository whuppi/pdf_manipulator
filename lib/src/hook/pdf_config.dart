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
  /// Creates a validated config from its parsed axes.
  const PdfManipulatorConfig({
    required this.keep,
    required this.detector,
    required this.build,
    required this.scanDirs,
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

  /// Extra app directories the `keep: auto` scan reads, on top of the app's
  /// own `lib/` and `bin/`. Paths are relative to the app root. Empty unless
  /// the user set `scan-dirs`, and only settable alongside a detector that
  /// actually scans — see [parse].
  final List<String> scanDirs;

  /// The only keys allowed under `pdf_manipulator:`. Anything else is a typo
  /// or a stranded option and fails the build (where the key set is visible —
  /// see [parse]).
  static const knownKeys = {'keep', 'detector', 'build', 'scan-dirs'};

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
    final scanDirs = _parseScanDirs(defines['scan-dirs']);

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

    // `scan-dirs` widens the source scan, so it needs a config that actually
    // scans: `keep: auto`, and a detector other than record-use (which reads
    // post-AOT recordings in the link hook and never looks at source).
    if (defines.containsKey('scan-dirs')) {
      if (keep.mode != KeepMode.auto) {
        throw PdfConfigError(
          'scan-dirs: only applies to `keep: auto` '
          '(keep is currently ${keep.mode.name}).\n'
          'A manual `keep: [...]` already names every capability, so there '
          'is nothing to scan for. Either use `keep: auto`, or remove the '
          '`scan-dirs:` line.',
        );
      }
      if (detector == KeepDetector.recordUse) {
        throw PdfConfigError(
          'scan-dirs: does not apply to `detector: ${KeepDetector.recordUse.wire}`, '
          'which reads recorded usage after compilation instead of scanning '
          'source.\n'
          'Either use `detector: ${KeepDetector.scan.wire}`, or remove the '
          '`scan-dirs:` line.',
        );
      }
    }

    return PdfManipulatorConfig(
      keep: keep,
      detector: detector,
      build: build,
      scanDirs: scanDirs,
    );
  }

  /// Parses the raw `scan-dirs` value: a list of app-relative directory
  /// paths. Absent → empty.
  ///
  /// Absolute paths are rejected — a pubspec is committed and shared, so a
  /// machine-specific path breaks every other checkout and CI.
  static List<String> _parseScanDirs(Object? raw) {
    if (raw == null) return const [];
    if (raw is! List) {
      throw PdfConfigError(
        'scan-dirs: takes a list of app-relative directories.\n'
        'Example:\n'
        '  scan-dirs: [tools, packages/shared/lib]',
      );
    }
    final dirs = <String>[];
    for (final entry in raw) {
      final dir = entry is String ? entry.trim() : '';
      if (dir.isEmpty) {
        throw PdfConfigError(
          'scan-dirs: every entry must be a non-empty directory path '
          '(got ${entry == null ? 'null' : '"$entry"'}).',
        );
      }
      if (dir.startsWith('/') || RegExp(r'^[A-Za-z]:[\\/]').hasMatch(dir)) {
        throw PdfConfigError(
          'scan-dirs: "$dir" is an absolute path. Paths are relative to your '
          'app root, so the same pubspec works on every machine and in CI.',
        );
      }
      dirs.add(dir);
    }
    return List.unmodifiable(dirs);
  }
}
