// The engine build profile — the `profile:` user-define read from the app's
// pubspec by both the native build hook and the web setup script. Orthogonal
// to trim: trim picks WHICH capabilities compile, this picks HOW they compile.
//
// All three ride on cargo's `release` profile via CARGO_PROFILE_RELEASE_*
// env overrides rather than named cargo profiles. That keeps everything in
// this package (no vendored-manifest edit) AND keeps `panic = "unwind"` — so
// the native engine's `catch_unwind` crash isolation ("one bad PDF → typed
// error, engine survives") keeps working. A `panic = "abort"` size profile
// would break that, which is why `small` only lowers opt-level.
//
// Only `release` has prebuilt release binaries; `small` and `debug` always
// compile from source, exactly like a custom trim set.

/// How the engine is compiled. Default [release].
enum EngineProfile {
  /// Shipped default: opt-level 3, LTO, stripped. Prebuilt binaries are this.
  release('release'),

  /// opt-level "z" — smaller code, slower. Compiles from source. Stacks with
  /// trim (fewest features + densest codegen = smallest engine).
  small('small'),

  /// Release codegen (identical behaviour, reproduces the same crash) plus
  /// full debug info and no stripping — so a native crash symbolicates to
  /// file:line. Compiles from source. For diagnosing engine crashes.
  debug('debug');

  const EngineProfile(this.wire);

  /// The value written under `hooks: user_defines: pdf_manipulator: profile:`.
  final String wire;

  /// True for the shipped default — the only profile with prebuilt binaries.
  bool get isDefault => this == EngineProfile.release;

  /// cargo env applied on top of `--release`. Empty for [release]; the others
  /// tweak the release profile in place (panic stays unwind — native-safe).
  Map<String, String> get cargoEnv => switch (this) {
    EngineProfile.release => const {},
    EngineProfile.small => const {'CARGO_PROFILE_RELEASE_OPT_LEVEL': 'z'},
    EngineProfile.debug => const {
      'CARGO_PROFILE_RELEASE_DEBUG': '2',
      'CARGO_PROFILE_RELEASE_STRIP': 'none',
    },
  };

  /// Parses the `profile:` user-define value. Absent → [release]. An unknown
  /// value throws — a config typo must fail the build loudly, never silently
  /// ship the wrong profile.
  static EngineProfile parse(Object? value) {
    if (value == null) return EngineProfile.release;
    final v = value.toString().trim().toLowerCase();
    for (final p in EngineProfile.values) {
      if (p.wire == v) return p;
    }
    throw ArgumentError(
      'pdf_manipulator: unknown engine profile "$value". '
      'Valid: ${EngineProfile.values.map((p) => p.wire).join(', ')} '
      '(default: release).',
    );
  }
}
