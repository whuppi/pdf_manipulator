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
/// builder) is always included and has no capability.
enum PdfCapability {
  /// Page rasterization (`PdfDoc.render`) and image re-compression
  /// (`PdfEditor.optimizeImages`, the `compress` one-shot).
  render('render', 'rendering'),

  /// Digital signatures: signing and verification.
  signatures('signatures', 'signatures'),

  /// PDF/A validation and conversion (embeds the Liberation faces the
  /// spec requires).
  pdfa('pdfa', 'pdfa'),

  /// PDF ↔ office conversion (DOCX / PPTX / XLSX, both directions).
  /// Converting a PDF to office extracts its content first, so this
  /// capability requires [extract] — keep-lists expand it automatically.
  office('office', 'office', requires: {PdfCapability.extract}),

  /// Text extraction and everything built on it: plain/markdown/HTML
  /// extraction, search, page/document classification (includes the CJK
  /// CID→Unicode tables).
  extract('extract', 'extract');

  const PdfCapability(this.wire, this.cargoFeature, {this.requires = const {}});

  /// The name used in `trim: {keep: [...]}`.
  final String wire;

  /// The engine feature this capability maps to (internal).
  final String cargoFeature;

  /// Capabilities this one cannot work without. Keep-lists and the
  /// detector expand these automatically — users never spell them out.
  /// Must mirror the engine's cargo feature dependencies (a parity test
  /// reads the engine manifest and fails on drift).
  final Set<PdfCapability> requires;

  /// [set] closed over [requires], transitively — `{office}` becomes
  /// `{office, extract}`.
  static Set<PdfCapability> expandRequires(Set<PdfCapability> set) {
    final expanded = <PdfCapability>{};
    void add(PdfCapability c) {
      if (!expanded.add(c)) return;
      c.requires.forEach(add);
    }

    set.forEach(add);
    return expanded;
  }

  /// Public API members whose reachability implies this capability.
  /// Consumed by the detector. Verified against the engine's cfg gates —
  /// update BOTH together.
  ///
  /// Key shapes document the API surface: `Class.member` for class
  /// members, bare names for extension and one-shot members. The text
  /// scan matches only the member part (the last segment), so both
  /// shapes behave identically to it — the qualified forms exist for
  /// readers and for the README drift guard, not for lookup precision.
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
    'Pdf.convertTo': PdfCapability.office,
    'convertTo': PdfCapability.office,
    'Pdf.convertToPdf': PdfCapability.office,
    'convertToPdf': PdfCapability.office,
    'PdfDoc.extract': PdfCapability.extract,
    'PdfDoc.search': PdfCapability.extract,
    'PdfDoc.classifyPage': PdfCapability.extract,
    'PdfDoc.classifyDocument': PdfCapability.extract,
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

  /// Manual keep-set (the full override). Expands [PdfCapability.requires]
  /// transitively, so `keep: [office]` also keeps `extract`.
  factory TrimConfig.keep(Set<PdfCapability> capabilities) {
    return TrimConfig._(
      TrimMode.manual,
      PdfCapability.expandRequires(capabilities),
    );
  }

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
      'Capabilities: render, signatures, pdfa, office, extract '
      '(and core, which is always included)';

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
        // 'core' is always included — accepting it lets users state the
        // obvious without an error.
        if (item == 'core') continue;
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

  /// The cargo feature list for [keepSet], applied to [defaultFeatures]
  /// (the build.json set): capability features not in the keep-set are
  /// dropped; everything else passes through untouched.
  ///
  /// [keepSet] may be unexpanded — [PdfCapability.requires] is applied
  /// here, so a detector set holding only `office` still keeps `extract`.
  /// Callers never pre-expand.
  String featuresFor(String defaultFeatures, Set<PdfCapability> keepSet) {
    final kept = PdfCapability.expandRequires(keepSet);
    final dropped = PdfCapability.values
        .where((c) => !kept.contains(c))
        .map((c) => c.cargoFeature)
        .toSet();
    return defaultFeatures
        .split(',')
        .map((f) => f.trim())
        .where((f) => f.isNotEmpty && !dropped.contains(f))
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

/// Which mechanism computes the keep-set for `trim: auto`.
///
/// Selected by the `trim-detector` user-define. [scan] is the stable
/// default; [recordUse] and [compare] belong to the EXPERIMENTAL RecordUse
/// lane (see `record_use_shim.dart`).
enum TrimDetector {
  /// Dependency-free text scan over the app source (default, all
  /// platforms).
  scan('scan'),

  /// EXPERIMENTAL — the SDK's `@RecordUse` recording. Usage data only
  /// exists after AOT compilation (read by the link hook), which is too
  /// late to drive the native build — selecting this fails loudly until
  /// the SDK lane matures. Use [compare] to observe it.
  recordUse('record-use'),

  /// Trims with [scan]; the link hook additionally prints the
  /// RecordUse-recorded capability set so the two can be diffed.
  compare('compare');

  const TrimDetector(this.wire);

  /// The name used in the `trim-detector` user-define.
  final String wire;

  /// Parses the raw `trim-detector` user-define value. Absent → [scan];
  /// anything unrecognized throws [TrimConfigError].
  static TrimDetector parse(Object? raw) {
    if (raw == null) return scan;
    for (final d in TrimDetector.values) {
      if (d.wire == raw) return d;
    }
    throw TrimConfigError(
      'unknown trim-detector "$raw". '
      'Valid: scan (default), record-use (EXPERIMENTAL), compare.',
    );
  }
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
