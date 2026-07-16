// The trim vocabulary: user-facing capabilities → cargo features → the
// public API members that reach them. One artifact (a keep-set) feeds the
// whole trim pipeline, whether it came from the detector (`trim: auto`) or
// the user's keep-list (`trim: {keep: [...]}`).
//
// Users never see cargo feature names — capabilities are named after the
// API they call and stay stable across engine bumps. Engine internals
// (native-bridge, wasm, icc, legacy-crypto) are deliberately not
// expressible here: not droppable, not a foot-gun.

/// One optional heavy module of the engine. Core (parse/write/edit/forms/
/// extract/builder) is always included and has no capability.
enum PdfCapability {
  /// Page rasterization (`PdfDoc.render`) and image re-compression
  /// (`PdfEditor.optimizeImages`, the `compress` one-shot).
  render('render', 'rendering'),

  /// Digital signatures: signing and verification.
  signatures('signatures', 'signatures'),

  /// PDF/A validation and conversion (embeds the Liberation faces the
  /// spec requires).
  pdfa('pdfa', 'pdfa');

  const PdfCapability(this.wire, this.cargoFeature);

  /// The name used in `trim: {keep: [...]}`.
  final String wire;

  /// The engine feature this capability maps to (internal).
  final String cargoFeature;

  /// Public API members (`Class.member` or top-level function name) whose
  /// reachability implies this capability. Consumed by the detector.
  /// Verified against the engine's cfg gates — update BOTH together.
  static const Map<String, PdfCapability> apiMembers = {
    'PdfDoc.render': PdfCapability.render,
    'PdfEditor.optimizeImages': PdfCapability.render,
    'compress': PdfCapability.render,
    'Pdf.sign': PdfCapability.signatures,
    'sign': PdfCapability.signatures,
    'PdfDoc.getSignatures': PdfCapability.signatures,
    'PdfDoc.verifySignatures': PdfCapability.signatures,
    'PdfDoc.validatePdfA': PdfCapability.pdfa,
    'PdfDoc.validatePdfUa': PdfCapability.pdfa,
    'PdfEditor.convertToPdfA': PdfCapability.pdfa,
    'convertToPdfA': PdfCapability.pdfa,
  };

  /// The capability whose [wire] name is [name], or null.
  static PdfCapability? byWire(String name) {
    for (final c in PdfCapability.values) {
      if (c.wire == name) return c;
    }
    return null;
  }
}

/// The parsed `trim:` user-define. Exactly three legal shapes:
/// absent/false → [TrimConfig.off]; `auto`/`true` → [TrimConfig.auto];
/// `{keep: [...]}` → a manual keep-set. Anything else throws
/// [TrimConfigError] — a config mistake must fail the build loudly,
/// never silently produce a full binary.
class TrimConfig {
  const TrimConfig._(this.mode, this.keep);

  /// Manual keep-set (the full override).
  factory TrimConfig.keep(Set<PdfCapability> capabilities) =>
      TrimConfig._(TrimMode.manual, capabilities);

  /// No trimming: the full default binary.
  static const off = TrimConfig._(TrimMode.off, null);

  /// Detector-computed keep-set.
  static const auto = TrimConfig._(TrimMode.auto, null);

  /// Which of the three trim modes this config selects.
  final TrimMode mode;

  /// Non-null only for [TrimMode.manual].
  final Set<PdfCapability>? keep;

  static const _grammar =
      'Valid forms:\n'
      '  trim: auto                     # detector decides\n'
      '  trim:\n'
      '    keep: [render, signatures]   # exactly these capabilities\n'
      'Capabilities: render, signatures, pdfa';

  /// Parses the raw `trim` user-define value (YAML-decoded).
  static TrimConfig parse(Object? raw) {
    if (raw == null || raw == false) return off;
    if (raw == 'auto' || raw == true) return auto;
    if (raw is Map) {
      final keys = raw.keys.map((k) => '$k').toSet();
      if (keys.length != 1 || !keys.contains('keep')) {
        throw TrimConfigError(
          'trim map supports exactly one key: "keep" (got: $keys).\n$_grammar',
        );
      }
      final list = raw['keep'];
      if (list is! List) {
        throw TrimConfigError(
          'trim.keep must be a list of capability names.\n$_grammar',
        );
      }
      final set_ = <PdfCapability>{};
      for (final item in list) {
        final cap = PdfCapability.byWire('$item');
        if (cap == null) {
          throw TrimConfigError(
            'unknown capability "$item" in trim.keep.\n$_grammar',
          );
        }
        set_.add(cap);
      }
      return TrimConfig.keep(set_);
    }
    throw TrimConfigError('unrecognized trim value: $raw.\n$_grammar');
  }

  /// The cargo feature list for [keep], applied to [defaultFeatures]
  /// (the build.json set): capability features not in the keep-set are
  /// dropped; everything else passes through untouched.
  String featuresFor(String defaultFeatures, Set<PdfCapability> keepSet) {
    final dropped = PdfCapability.values
        .where((c) => !keepSet.contains(c))
        .map((c) => c.cargoFeature)
        .toSet();
    return defaultFeatures
        .split(',')
        .where((f) => !dropped.contains(f))
        .join(',');
  }
}

/// Which of the three trim modes a config selects.
enum TrimMode {
  /// No trimming — the full default binary.
  off,

  /// Detector-computed keep-set.
  auto,

  /// User-supplied keep-set.
  manual,
}

/// A malformed `trim:` user-define. Fails the build with the grammar.
class TrimConfigError extends Error {
  /// Creates the error with [message] (includes the valid grammar).
  TrimConfigError(this.message);

  /// What was wrong plus the valid grammar.
  final String message;

  @override
  String toString() => 'pdf_manipulator trim config error: $message';
}
