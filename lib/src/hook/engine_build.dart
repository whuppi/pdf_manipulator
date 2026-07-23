// The engine build mode — the `build:` user-define read from the app's
// pubspec by both the native build hook and the web setup script. Orthogonal
// to keep: keep picks WHICH capabilities compile, this picks HOW they compile.
//
// All three ride on cargo's `release` profile via CARGO_PROFILE_RELEASE_*
// env overrides rather than named cargo profiles. That keeps everything in
// this package (no vendored-manifest edit) AND keeps `panic = "unwind"` — so
// the native engine's `catch_unwind` crash isolation ("one bad PDF → typed
// error, engine survives") keeps working. A `panic = "abort"` size build
// would break that, which is why `size` only lowers opt-level.
//
// Only `speed` has prebuilt binaries; `size` and `debug` always compile
// from source, exactly like a custom keep set.

import 'package:pdf_manipulator/src/keep/capabilities.dart' show PdfConfigError;

/// How the engine is compiled. Default [speed].
enum EngineBuild {
  /// Shipped default: opt-level 3, LTO, stripped. Prebuilt binaries are this.
  speed('speed'),

  /// opt-level "z" — smaller code, slower. Compiles from source. Stacks with
  /// keep (fewest capabilities + densest codegen = smallest engine).
  size('size'),

  /// Speed codegen (identical behaviour, reproduces the same crash) plus
  /// full debug info and no stripping — so a native crash symbolicates to
  /// file:line. Compiles from source. For diagnosing engine crashes.
  debug('debug');

  const EngineBuild(this.wire);

  /// The value written under `hooks: user_defines: pdf_manipulator: build:`.
  final String wire;

  /// True for the shipped default — the only build with prebuilt binaries.
  bool get isDefault => this == EngineBuild.speed;

  /// cargo env applied on top of `--release`. Empty for [speed]; the others
  /// tweak the release profile in place (panic stays unwind — native-safe).
  Map<String, String> get cargoEnv => switch (this) {
    EngineBuild.speed => const {},
    EngineBuild.size => const {'CARGO_PROFILE_RELEASE_OPT_LEVEL': 'z'},
    EngineBuild.debug => const {
      'CARGO_PROFILE_RELEASE_DEBUG': '2',
      'CARGO_PROFILE_RELEASE_STRIP': 'none',
    },
  };

  /// Parses the `build:` user-define value. Absent → [speed]. An unknown
  /// value throws — a config typo must fail the build loudly, never silently
  /// ship the wrong build.
  static EngineBuild parse(Object? value) {
    if (value == null) return EngineBuild.speed;
    final v = value.toString().trim().toLowerCase();
    for (final b in EngineBuild.values) {
      if (b.wire == v) return b;
    }
    throw PdfConfigError(
      'unknown build "$value". '
      'Valid: ${EngineBuild.values.map((b) => b.wire).join(', ')} '
      '(default: speed).',
    );
  }
}
