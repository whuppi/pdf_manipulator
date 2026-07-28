// Locating the consuming app — the ONE derivation both lanes use.
//
// Neither lane can trust `Directory.current`:
//
//   • The build hook is never told where the app is. `BuildInput` exposes
//     `packageName`, `packageRoot` and the output directories, nothing more,
//     and the runner starts every hook with its working directory set to the
//     package's OWN root (hooks_runner's `build_runner.dart`:
//     `workingDirectory = input.packageRoot`), for `dart run` and
//     `flutter build` alike. Reading it scans pdf_manipulator, where every
//     capability is implemented, and concludes the app reaches all of them —
//     a confident, wrong "keep everything" for every consumer.
//
//   • The web setup script is user-invoked, so its working directory is
//     wherever the user happened to be standing — the app root if they ran it
//     from there, a subdirectory if they did not.
//
// What both lanes DO have is a path inside the app's own `.dart_tool/`: the
// hook gets its output directory
// (`<app>/.dart_tool/hooks_runner/shared/pdf_manipulator/build/<hash>/`), and
// the setup script gets `Isolate.packageConfig`
// (`<app>/.dart_tool/package_config.json`) — the VM's answer to "whose
// package resolution is running me", which does not depend on the working
// directory. Same anchor, so one function serves both and they cannot
// disagree about which directory the app is.

library;

/// The consuming app's root, derived from any [pathInsideDartTool] under the
/// app's `.dart_tool/` — a build hook's output directory, or the package
/// config the VM is running under.
///
/// Returns null when the path holds no `.dart_tool` segment. Callers MUST
/// treat null as "the app was not located" and fall closed to the full
/// binary — never as "the app uses nothing".
String? appRootFromDartTool(Uri pathInsideDartTool) {
  final segments = pathInsideDartTool.pathSegments
      .where((s) => s.isNotEmpty)
      .toList();
  // The LAST segment: an app may itself live under some other package's
  // `.dart_tool`, and the innermost one is the app being built.
  final marker = segments.lastIndexOf('.dart_tool');
  // `< 1` covers both "absent" and "at the filesystem root", where the
  // parent would be `/` — never a plausible app root.
  if (marker < 1) return null;
  return pathInsideDartTool
      .replace(pathSegments: segments.sublist(0, marker))
      .toFilePath();
}
