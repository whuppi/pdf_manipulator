// Link hook for pdf_manipulator.
//
// ═══════════════════════════════════════════════════════════════════
// TODAY — passthrough
// ═══════════════════════════════════════════════════════════════════
//
// Forwards the native binary to the app bundle unchanged. The Rust
// engine compiles as one monolithic binary with all features enabled.
//
// ═══════════════════════════════════════════════════════════════════
// NEAR-TERM — user_defines (works today, see icu_kit)
// ═══════════════════════════════════════════════════════════════════
//
// Consumers opt out of features they don't need via pubspec:
//
//   hooks:
//     user_defines:
//       pdf_manipulator:
//         rendering: false
//         office: false
//
// The build hook reads these and passes matching --features to cargo.
// One smaller binary, no link hook changes needed — user_defines flow
// through BuildInput, not through the linker.
//
// This pattern is proven by icu_kit (bundleCldrData user_define
// toggles the compiled_data cargo feature).
//
// ═══════════════════════════════════════════════════════════════════
// LONG-TERM — automatic tree-shaking via @RecordUse
// ═══════════════════════════════════════════════════════════════════
//
// The compiler records which Dart APIs the app actually calls. This
// hook reads the usage record, maps methods to cargo features, and
// recompiles with only the needed features. Zero user config — the
// compiler decides.
//
// Blocked:
//   @RecordUse instance methods — dart-lang/native#2902
//   FFI tree-shaking umbrella   — dart-lang/sdk#52970
//
// ═══════════════════════════════════════════════════════════════════
// WEB — different tradeoff
// ═══════════════════════════════════════════════════════════════════
//
// WASM is served over the network — size matters more than native.
// Native gets one smaller binary via cargo features (no duplicate
// deps). Web could ship multiple per-feature WASM binaries and have
// setup download only the ones needed, but that adds too much user
// complexity for marginal gain.
//
// Better path: wait for web build hook support (dart-lang/native#988)
// which gives web the same automatic cargo-features approach as
// native. Until then, web ships the full WASM binary.
//
// Note: unlike icu_kit where the binary is big because of DATA
// (CLDR locales — solvable by lazy per-locale loading), pdf_manipulator's
// size is CODE (rendering engine, office converter, signing).
// Data splitting doesn't apply here — only code stripping helps.

// Called by Flutter on release/AOT builds when build.dart routes
// the native binary via ToLinkHook. Not called on debug builds
// (routed directly to ToAppBundle) or for web assets.
import 'package:hooks/hooks.dart';

void main(List<String> args) async {
  await link(args, (LinkInput input, LinkOutputBuilder output) async {
    for (final asset in input.assets.encodedAssets) {
      output.assets.addEncodedAsset(asset);
    }
  });
}
