// Setup script for pdf_manipulator on web targets.
//
// Usage from a Flutter / Dart web app:
//
//   dart run pdf_manipulator:setup           # install if missing
//   dart run pdf_manipulator:setup --force   # overwrite even if present
//
// Copies the JS glue + Web Worker from the package's web_assets/ directory,
// and downloads the WASM binary from GitHub Releases. Flutter web's build
// pipeline serves anything under web/ as a static asset.
//
// On native platforms, this step isn't needed — the build hook handles
// everything automatically.

import 'dart:io';

import 'package:package_config/package_config.dart';

const _help = '''
Usage: dart run pdf_manipulator:setup [--force]

Installs pdf_manipulator web assets into your project's web/pdf_manipulator/ folder.

Options:
  --force      Overwrite existing files.
  -h, --help   Show this help.

Files installed:
  web/pdf_manipulator/coordinator.js       — Coordinator Worker (manages WASM worker pool)
  web/pdf_manipulator/worker.js            — WASM Worker (runs engine operations)
  web/pdf_manipulator/pdf_oxide.js         — ESM module (JS glue for WASM)
  web/pdf_manipulator/pdf_oxide_bg.wasm    — Compiled WASM binary (downloaded from GitHub Releases)
''';

const _localFiles = ['coordinator.js', 'worker.js', 'pdf_oxide.js'];
const _wasmFile = 'pdf_oxide_bg.wasm';
const _releaseRepo = 'https://github.com/whuppi/pdf_manipulator/releases/download';

void main(List<String> args) async {
  if (args.contains('-h') || args.contains('--help')) {
    print(_help);
    return;
  }
  final force = args.contains('--force');

  final config = await findPackageConfig(Directory.current);
  if (config == null) {
    stderr.writeln('Error: not inside a Dart/Flutter project.');
    exit(1);
  }

  final pkgOrNull =
      config.packages.where((p) => p.name == 'pdf_manipulator');
  if (pkgOrNull.isEmpty) {
    stderr.writeln('Error: pdf_manipulator not in pubspec.yaml.');
    exit(1);
  }
  final pkg = pkgOrNull.first;

  final webAssetsDir = Directory.fromUri(pkg.root.resolve('web_assets/'));
  final version = _readVersion(pkg.root);

  final destDir = Directory('web/pdf_manipulator');
  if (!destDir.existsSync()) {
    destDir.createSync(recursive: true);
    print('Created ${destDir.path}/');
  }

  var installed = 0;

  // Write version stamp so WebBridge can detect stale assets
  final versionFile = File('${destDir.path}/.version');
  versionFile.writeAsStringSync(version);
  print('  .version — $version');

  // Copy JS glue + worker from package (small text files, committed in git)
  for (final name in _localFiles) {
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
    if (name == 'coordinator.js') {
      // Stamp version into coordinator so WebBridge can detect stale assets
      var content = src.readAsStringSync();
      content = content.replaceFirst("'__VERSION__'", "'$version'");
      dst.writeAsStringSync(content);
    } else {
      src.copySync(dst.path);
    }
    print('  $name — copied (${(src.lengthSync() / 1024).toStringAsFixed(0)} KB)');
    installed++;
  }

  // Download WASM binary from GitHub Releases (or copy local if available)
  final wasmDst = File('${destDir.path}/$_wasmFile');
  if (wasmDst.existsSync() && !force) {
    print('  $_wasmFile — already exists (use --force to overwrite)');
  } else {
    final localWasm = File.fromUri(webAssetsDir.uri.resolve(_wasmFile));
    if (localWasm.existsSync()) {
      // Contributor has locally built WASM
      localWasm.copySync(wasmDst.path);
      final mb = (localWasm.lengthSync() / 1024 / 1024).toStringAsFixed(1);
      print('  $_wasmFile — copied from local build ($mb MB)');
      installed++;
    } else {
      // Consumer: download from GitHub Releases
      final url = '$_releaseRepo/v$version/wasm-$_wasmFile';
      print('  $_wasmFile — downloading from GitHub Releases...');
      try {
        await _download(url, wasmDst);
        final mb = (wasmDst.lengthSync() / 1024 / 1024).toStringAsFixed(1);
        print('  $_wasmFile — downloaded ($mb MB)');
        installed++;
      } catch (e) {
        stderr.writeln('Error downloading WASM: $e\n'
            'URL: $url\n\n'
            'Options:\n'
            '  1. Build locally: ./tool/build_wasm.sh\n'
            '  2. Check that version $version has a GitHub Release with WASM\n');
        exit(1);
      }
    }
  }

  if (installed > 0) {
    print('\nDone. $installed file(s) installed to ${destDir.path}/');
  } else {
    print('\nNothing to do. All files already present.');
  }
}

String _readVersion(Uri packageRoot) {
  final pubspec = File.fromUri(packageRoot.resolve('pubspec.yaml'));
  for (final line in pubspec.readAsLinesSync()) {
    if (line.startsWith('version:')) {
      return line.substring('version:'.length).trim();
    }
  }
  throw StateError('No version in pubspec.yaml');
}

Future<void> _download(String url, File dest) async {
  final client = HttpClient();
  try {
    var uri = Uri.parse(url);
    HttpClientResponse response;
    var redirects = 0;
    do {
      final req = await client.getUrl(uri);
      req.followRedirects = false;
      response = await req.close();
      if (response.isRedirect && response.headers.value('location') != null) {
        await response.drain<void>();
        uri = Uri.parse(response.headers.value('location')!);
        redirects++;
      } else {
        break;
      }
    } while (redirects < 5);

    if (response.statusCode != 200) {
      await response.drain<void>();
      throw StateError('HTTP ${response.statusCode}');
    }

    dest.parent.createSync(recursive: true);
    final sink = dest.openWrite();
    await response.pipe(sink);
  } finally {
    client.close();
  }
}
