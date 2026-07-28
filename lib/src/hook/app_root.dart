// Locating the consuming app from inside a build hook.
//
// A build hook is never told where the app is. `BuildInput` exposes
// `packageName`, `packageRoot` and the output directories — nothing else —
// and `Directory.current` is NOT the app: the runner starts every hook with
// its working directory set to the package's own root (hooks_runner's
// `build_runner.dart`: `workingDirectory = input.packageRoot`), for
// `dart run` and `flutter build` alike, on every runner version shipped so
// far.
//
// Scanning `Directory.current` therefore scans pdf_manipulator itself, where
// every capability's implementation lives, and concludes the app reaches all
// of them — a confident, wrong "keep everything" for every consumer.
//
// The output directories are the one handle on the app a hook does get: they
// are created under the app's `.dart_tool/`, e.g.
// `<app>/.dart_tool/hooks_runner/shared/pdf_manipulator/build/<hash>/`.

library;

/// The consuming app's root, derived from a hook [outputDirectory].
///
/// Returns null when the path holds no `.dart_tool` segment. Callers MUST
/// treat null as "the app was not located" and fall closed to the full
/// binary — never as "the app uses nothing".
String? appRootFromHookOutput(Uri outputDirectory) {
  final segments = outputDirectory.pathSegments
      .where((s) => s.isNotEmpty)
      .toList();
  // The LAST segment: an app may itself live under some other package's
  // `.dart_tool`, and the innermost one is the app being built.
  final marker = segments.lastIndexOf('.dart_tool');
  // `< 1` covers both "absent" and "at the filesystem root", where the
  // parent would be `/` — never a plausible app root.
  if (marker < 1) return null;
  return outputDirectory
      .replace(pathSegments: segments.sublist(0, marker))
      .toFilePath();
}
