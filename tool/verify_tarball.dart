// Asserts the pub tarball carries every file the compile-from-source
// waterfall needs. Pub's file filtering honors the VENDORED repos' own
// .gitignore files even for git-tracked paths, so an upstream ignore
// rule can silently strip a build-critical file from the published
// archive while the git tag keeps it (that is how 2.2.0 shipped without
// vendor/pdf_oxide/Cargo.lock — issue #171). The dry-run reproduces the
// release filtering locally, so this runs on every `make check`.

import 'dart:io';

/// Every file the tarball must carry for a consumer source build.
/// Grouped by what breaks when it goes missing.
const requiredFiles = [
  // Engine compile: cargo needs manifest + lock (exact pinned deps);
  // the lock also pins bindgen_runner's wasm-bindgen-cli-support +
  // wasm-opt to the wasm-bindgen version the engine compiles with.
  'vendor/pdf_oxide/Cargo.toml',
  'vendor/pdf_oxide/Cargo.lock',
  'vendor/office_oxide/Cargo.toml',
  'vendor/office_oxide/Cargo.lock',
  // Source sentinels — one per vendored crate tree, plus the wasm
  // post-processing runner the web compile `cargo run`s.
  'vendor/pdf_oxide/src/lib.rs',
  'vendor/office_oxide/src/lib.rs',
  'vendor/pdf_oxide/bindgen_runner/Cargo.toml',
  'vendor/pdf_oxide/bindgen_runner/src/main.rs',
  // Build config the hooks read.
  'build.json',
  // Hooks + the hand-written web assets.
  'hook/build.dart',
  'hook/link.dart',
  'web_assets/lane_worker.js',
];

void main() {
  final result = Process.runSync('dart', ['pub', 'publish', '--dry-run']);
  final output = '${result.stdout}\n${result.stderr}';

  final files = _treeToPaths(output);
  if (files.length < 100) {
    stderr.writeln(
      'verify_tarball: dry-run listed only ${files.length} files — the '
      'listing parse or the dry-run itself failed.\n$output',
    );
    exit(1);
  }

  final missing = requiredFiles.where((f) => !files.contains(f)).toList();
  if (missing.isNotEmpty) {
    stderr.writeln(
      'TARBALL GATE FAIL — build-critical files missing from the pub '
      'archive:\n  ${missing.join('\n  ')}\n'
      'Check the vendored repos\' .gitignore files: pub honors them even '
      'for git-tracked paths.',
    );
    exit(1);
  }
  stdout.writeln(
    'tarball OK — ${files.length} files, all ${requiredFiles.length} '
    'build-critical paths present',
  );
}

/// Reconstructs full paths from `pub publish --dry-run`'s tree listing.
Set<String> _treeToPaths(String output) {
  final entry = RegExp(
    r'^([│ ]*)(?:├──|└──) (.+?)(?: \(<?\d+(?:\.\d+)? .?B\))?$',
  );
  final stack = <String>[];
  final paths = <String>{};
  for (final line in output.split('\n')) {
    final m = entry.firstMatch(line.trimRight());
    if (m == null) continue;
    final depth = m.group(1)!.length ~/ 4;
    if (depth > stack.length) {
      // A depth jump means the listing shape changed — growing a
      // non-nullable list throws anyway, so bail to empty and let the
      // caller's sanity check fail loudly with the raw output.
      return {};
    }
    stack.length = depth;
    stack.add(m.group(2)!);
    paths.add(stack.join('/'));
  }
  return paths;
}
