// Setup script for pdf_manipulator on web targets.
//
// Usage from a Flutter / Dart web app:
//
//   dart run pdf_manipulator:setup           # copy if missing
//   dart run pdf_manipulator:setup --force   # overwrite even if present
//
// Copies the pre-built WASM binary, JS glue, and Web Worker into
// the consuming app's web/pdf_manipulator/ directory. Flutter web's
// build pipeline serves anything under web/ as a static asset.
//
// On native platforms, this step isn't needed — the build hook
// compiles pdf_oxide via cargo automatically.

import 'dart:io';

import 'package:package_config/package_config.dart';

const _help = '''
Usage: dart run pdf_manipulator:setup [--force]

Copies pdf_manipulator web assets into your project's web/pdf_manipulator/ folder.

Options:
  --force      Overwrite existing files.
  -h, --help   Show this help.

Files installed:
  web/pdf_manipulator/pdf_oxide.js         — ESM module (JS glue for WASM)
  web/pdf_manipulator/pdf_oxide_bg.wasm    — Compiled WASM binary (~11 MB, ~3 MB compressed)
  web/pdf_manipulator/worker.js            — Web Worker for off-main-thread dispatch
''';

const _assetFiles = [
  'pdf_oxide.js',
  'pdf_oxide_bg.wasm',
  'worker.js',
];

void main(List<String> args) async {
  if (args.contains('-h') || args.contains('--help')) {
    print(_help);
    return;
  }
  final force = args.contains('--force');

  // Find our own package root via package_config
  final config = await findPackageConfig(Directory.current);
  if (config == null) {
    stderr.writeln('Error: not inside a Dart/Flutter project '
        '(no .dart_tool/package_config.json found).');
    exit(1);
  }

  final pkgOrNull = config.packages.where(
    (p) => p.name == 'pdf_manipulator',
  );
  if (pkgOrNull.isEmpty) {
    stderr.writeln('Error: pdf_manipulator not found in package config. '
        'Add it to pubspec.yaml first.');
    exit(1);
  }
  final pkg = pkgOrNull.first;

  final webAssetsDir = Directory.fromUri(pkg.root.resolve('web_assets/'));
  if (!webAssetsDir.existsSync()) {
    stderr.writeln('Error: web_assets/ not found in pdf_manipulator package. '
        'The WASM binary may not be built yet.');
    exit(1);
  }

  final destDir = Directory('web/pdf_manipulator');
  if (!destDir.existsSync()) {
    destDir.createSync(recursive: true);
    print('Created ${destDir.path}/');
  }

  var copied = 0;
  for (final name in _assetFiles) {
    final src = File.fromUri(webAssetsDir.uri.resolve(name));
    final dst = File('${destDir.path}/$name');

    if (!src.existsSync()) {
      stderr.writeln('Warning: $name not found in web_assets/. Skipping.');
      continue;
    }

    if (dst.existsSync() && !force) {
      print('  $name — already exists (use --force to overwrite)');
      continue;
    }

    src.copySync(dst.path);
    final size = (src.lengthSync() / 1024).toStringAsFixed(0);
    print('  $name — copied ($size KB)');
    copied++;
  }

  if (copied > 0) {
    print('\nDone. $copied file(s) installed to ${destDir.path}/');
  } else {
    print('\nNothing to do. All files already present.');
  }
}
