// readPdfManipulatorUserDefines reads the consuming app's
// `hooks: user_defines: pdf_manipulator:` block. It must yield the SAME
// plain-Dart shapes the native hook receives via BuildInput.userDefines,
// so web and native feed identical values to EngineBuild.parse /
// resolveKeepPlan. Proven against real temp pubspec files.

// io-exempt: build-time tooling test — writes temp pubspec files on disk;
// this reader never runs in a browser.
import 'dart:io';

import 'package:pdf_manipulator/src/hook/engine_build.dart';
import 'package:pdf_manipulator/src/hook/user_defines.dart';
import 'package:test/test.dart';

/// Writes [pubspec] into a fresh temp dir and returns its path.
String _appWith(String pubspec) {
  final dir = Directory.systemTemp.createTempSync('pdfm_defines_');
  addTearDown(() => dir.deleteSync(recursive: true));
  File('${dir.path}/pubspec.yaml').writeAsStringSync(pubspec);
  return dir.path;
}

void main() {
  group('readPdfManipulatorUserDefines', () {
    test('missing pubspec → empty map (defaults apply)', () {
      final dir = Directory.systemTemp.createTempSync('pdfm_empty_');
      addTearDown(() => dir.deleteSync(recursive: true));
      expect(readPdfManipulatorUserDefines(dir.path), isEmpty);
    });

    test('no hooks block → empty map', () {
      final root = _appWith('''
name: my_app
dependencies:
  pdf_manipulator: ^4.0.0
''');
      expect(readPdfManipulatorUserDefines(root), isEmpty);
    });

    test('block present but no pdf_manipulator key → empty map', () {
      final root = _appWith('''
name: my_app
hooks:
  user_defines:
    other_pkg:
      foo: bar
''');
      expect(readPdfManipulatorUserDefines(root), isEmpty);
    });

    test('build is read as a plain string', () {
      final root = _appWith('''
name: my_app
hooks:
  user_defines:
    pdf_manipulator:
      build: size
''');
      final defines = readPdfManipulatorUserDefines(root);
      expect(defines['build'], 'size');
      // The whole point: the extracted value drives the same parser native uses.
      expect(EngineBuild.parse(defines['build']), EngineBuild.size);
    });

    test('block is a plain Map and keep list deep-converts to plain Dart', () {
      final root = _appWith('''
name: my_app
hooks:
  user_defines:
    pdf_manipulator:
      build: debug
      keep: [render, signatures]
''');
      final defines = readPdfManipulatorUserDefines(root);

      // Not a YamlMap/YamlList — real Dart collections, so downstream
      // `is Map` / `is List` checks behave exactly as with native defines.
      expect(defines, isA<Map<String, Object?>>());
      final keep = defines['keep'];
      expect(keep, isA<List<Object?>>());
      expect(keep, ['render', 'signatures']);
      expect(EngineBuild.parse(defines['build']), EngineBuild.debug);
    });

    test('keep: auto is read as a plain scalar', () {
      final root = _appWith('''
name: my_app
hooks:
  user_defines:
    pdf_manipulator:
      keep: auto
''');
      final defines = readPdfManipulatorUserDefines(root);
      expect(defines['keep'], 'auto');
    });
  });
}
