// Every package file the native build hook reads is a dependency it
// registers. The hooks runner re-runs the hook only when a registered file
// changes, so a file read but not registered is an input the cache ignores:
// unregistered, a new RUST_VERSION in tool/versions.env keeps the build the
// older toolchain made.
//
// The guard reads each literal `packageRoot.resolve('<path>')` in hook/ and
// lib/src/hook/ and expects the path in `_registerDependencies`, either by
// name or as a config file of a vendored crate it registers. Two reads are
// exempt: `.gitmodules`, which only decides whether to fetch the vendored
// sources, and `web_assets/`, which the web setup reads outside the runner.
//
// io-exempt: reads the hook's sources from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';

final _resolve = RegExp(r"packageRoot\.resolve\(\s*'([^'$]+)'\s*\)");

const _exempt = {'.gitmodules', 'web_assets'};

const _crates = [
  'vendor/pdf_oxide',
  'vendor/office_oxide',
  'vendor/tiny_skia',
  'vendor/tiny_skia/path',
];

const _crateConfigs = ['Cargo.toml', 'Cargo.lock', 'build.rs'];

Iterable<File> _hookSources() => [
  File('hook/build.dart'),
  ...Directory(
    'lib/src/hook',
  ).listSync().whereType<File>().where((f) => f.path.endsWith('.dart')),
];

String _registerBody(String build) {
  final start = build.indexOf('void _registerDependencies(');
  expect(start, isNot(-1), reason: '_registerDependencies not found');
  final end = build.indexOf('\n}\n', start);
  return build.substring(start, end);
}

void main() {
  test('every file the hook reads is a registered dependency', () {
    final register = _registerBody(File('hook/build.dart').readAsStringSync());
    final registered = {
      for (final m in _resolve.allMatches(register)) m.group(1)!,
      for (final crate in _crates)
        for (final config in _crateConfigs) '$crate/$config',
    };
    expect(
      _crates.every(register.contains),
      isTrue,
      reason: '_registerDependencies no longer lists the vendored crates',
    );

    final unregistered = <String>[];
    for (final file in _hookSources()) {
      for (final m in _resolve.allMatches(file.readAsStringSync())) {
        final path = m.group(1)!;
        if (_exempt.contains(path) || registered.contains(path)) continue;
        unregistered.add('${file.path}: $path');
      }
    }
    expect(
      unregistered,
      isEmpty,
      reason:
          'add each path to _registerDependencies in hook/build.dart, or the '
          'runner keeps a build made before the file changed',
    );
  });
}
