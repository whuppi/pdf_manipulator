// A fallback cache restore never serves a stale build of a path package.
//
// CI restores a cargo target built from another engine revision when the
// exact one is missing, then pins every vendor source's mtime to one past
// instant, which makes cargo trust restored units of path packages. The
// step "Drop every path package a fallback restore brought" in
// .github/actions/capabilities/rust/action.yml is the only thing between
// that and a stale binary. This test runs that step, as written, on a
// small cargo workspace laid out like vendor/: it builds version 1, keeps
// the target the way the cache does, moves the sources to version 2,
// restores, runs the step, pins, and runs the tool. It must print version
// 2 for the tool and for its path dependency. Without the step the same
// sequence prints version 1, which the test checks too, so the scenario is
// proven to reproduce the bug it guards.
//
// io-exempt: builds a cargo workspace in a temp folder and runs bash and
// cargo, the tools CI runs the step with.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';
import 'package:yaml/yaml.dart';

const _stepName = 'Drop every path package a fallback restore brought';

String _dropStep() {
  final action =
      loadYaml(
            File(
              '.github/actions/capabilities/rust/action.yml',
            ).readAsStringSync(),
          )
          as YamlMap;
  final steps = (action['runs'] as YamlMap)['steps'] as YamlList;
  final step = steps.cast<YamlMap>().firstWhere(
    (s) => s['name'] == _stepName,
    orElse: () => throw StateError('no step named "$_stepName"'),
  );
  return step['run'] as String;
}

void _write(Directory root, String path, String text) =>
    File('${root.path}/$path')
      ..createSync(recursive: true)
      ..writeAsStringSync(text);

// Like the real bindgen_runner, the tool depends on no other vendored
// crate, so nothing rebuilds it but its own fingerprint being dropped.
String _tool(String version) => 'fn main() { println!("tool $version"); }\n';

String _skia(String version) =>
    'pub fn v() -> &\'static str { "skia $version" }\n';

ProcessResult _run(Directory dir, String exe, List<String> args) {
  final r = Process.runSync(exe, args, workingDirectory: dir.path);
  if (r.exitCode != 0) {
    fail('$exe ${args.join(' ')} failed in ${dir.path}:\n${r.stderr}');
  }
  return r;
}

void main() {
  test('a fallback restore rebuilds every path package', () {
    final root = Directory.systemTemp.createTempSync('fallback_cache_');
    try {
      _write(root, 'vendor/pdf_oxide/Cargo.toml', '''
[workspace]
members = [".", "tool"]
[package]
name = "pdf_oxide"
version = "0.1.0"
edition = "2021"
[dependencies]
tiny-skia = { path = "../tiny_skia" }
''');
      _write(
        root,
        'vendor/pdf_oxide/src/lib.rs',
        'pub fn v() -> &\'static str { tiny_skia::v() }\n',
      );
      // The engine's own binary covers the path-dependency chain.
      _write(
        root,
        'vendor/pdf_oxide/src/main.rs',
        'fn main() { println!("engine, {}", pdf_oxide::v()); }\n',
      );
      _write(root, 'vendor/pdf_oxide/tool/Cargo.toml', '''
[package]
name = "bindgen_runner"
version = "0.1.0"
edition = "2021"
''');
      _write(root, 'vendor/pdf_oxide/tool/src/main.rs', _tool('v1'));
      _write(root, 'vendor/office_oxide/Cargo.toml', '''
[workspace]
[package]
name = "office_oxide"
version = "0.1.0"
edition = "2021"
''');
      _write(root, 'vendor/office_oxide/src/lib.rs', '');
      _write(root, 'vendor/tiny_skia/Cargo.toml', '''
[workspace]
[package]
name = "tiny-skia"
version = "0.1.0"
edition = "2021"
''');
      _write(root, 'vendor/tiny_skia/src/lib.rs', _skia('v1'));

      final engine = Directory('${root.path}/vendor/pdf_oxide');
      final target = Directory('${engine.path}/target');
      final saved = '${root.path}/saved.tar';
      _run(engine, 'cargo', ['generate-lockfile', '-q', '--offline']);
      _run(engine, 'cargo', ['build', '-q', '--workspace', '--offline']);
      // The cache archive keeps the mtimes it was saved with.
      _run(root, 'tar', ['-cf', saved, 'vendor/pdf_oxide/target']);
      sleep(const Duration(seconds: 1));
      _write(root, 'vendor/pdf_oxide/tool/src/main.rs', _tool('v2'));
      _write(root, 'vendor/tiny_skia/src/lib.rs', _skia('v2'));

      void restore() {
        target.deleteSync(recursive: true);
        _run(root, 'tar', ['-xf', saved]);
      }

      // The step "Pin vendor source mtimes for cargo freshness".
      const pinScript =
          "find vendor -type f -not -path '*/target/*' "
          "-exec touch -t 200001010000 {} + && "
          "find vendor -name Cargo.toml -not -path '*/target/*' "
          "-exec touch -t 200001020000 {} +";
      void pin() => _run(root, 'bash', ['-c', pinScript]);

      String run(String package) =>
          (_run(engine, 'cargo', [
                    'run',
                    '-q',
                    '--offline',
                    '-p',
                    package,
                  ]).stdout
                  as String)
              .trim();
      String both() => '${run('bindgen_runner')} | ${run('pdf_oxide')}';

      restore();
      pin();
      expect(
        both(),
        'tool v1 | engine, skia v1',
        reason: 'without the step the scenario serves the stale build',
      );

      restore();
      final step = File('${root.path}/drop.sh')..writeAsStringSync(_dropStep());
      _run(root, 'bash', ['-eo', 'pipefail', step.path]);
      pin();
      expect(both(), 'tool v2 | engine, skia v2');
    } finally {
      root.deleteSync(recursive: true);
    }
  }, timeout: const Timeout(Duration(minutes: 5)));

  test('the step fails when cargo cannot list the path packages', () {
    final root = Directory.systemTemp.createTempSync('fallback_cache_');
    try {
      Directory('${root.path}/vendor/pdf_oxide').createSync(recursive: true);
      final step = File('${root.path}/drop.sh')..writeAsStringSync(_dropStep());
      final r = Process.runSync('bash', [
        '-eo',
        'pipefail',
        step.path,
      ], workingDirectory: root.path);
      expect(r.exitCode, isNot(0), reason: 'an empty list means a stale build');
    } finally {
      root.deleteSync(recursive: true);
    }
  });
}
