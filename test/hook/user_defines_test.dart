// readPdfManipulatorUserDefines reads the consuming app's
// `hooks: user_defines: pdf_manipulator:` block. It must yield the SAME
// plain-Dart shapes the native hook receives via BuildInput.userDefines,
// so web and native feed identical values to EngineProfile.parse /
// resolveTrimPlan. Proven against real temp pubspec files.

// io-exempt: build-time tooling test — writes temp pubspec files on disk;
// this reader never runs in a browser.
import 'dart:io';

import 'package:pdf_manipulator/src/hook/build_profile.dart';
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

    test('profile is read as a plain string', () {
      final root = _appWith('''
name: my_app
hooks:
  user_defines:
    pdf_manipulator:
      profile: small
''');
      final defines = readPdfManipulatorUserDefines(root);
      expect(defines['profile'], 'small');
      // The whole point: the extracted value drives the same parser native uses.
      expect(EngineProfile.parse(defines['profile']), EngineProfile.small);
    });

    test('nested trim map/list deep-convert to plain Dart', () {
      final root = _appWith('''
name: my_app
hooks:
  user_defines:
    pdf_manipulator:
      profile: debug
      trim:
        keep: [render, signatures]
''');
      final defines = readPdfManipulatorUserDefines(root);

      // Not a YamlMap/YamlList — real Dart collections, so downstream
      // `is Map` / `is List` checks behave exactly as with native defines.
      final trim = defines['trim'];
      expect(trim, isA<Map<String, Object?>>());
      final keep = (trim! as Map)['keep'];
      expect(keep, isA<List<Object?>>());
      expect(keep, ['render', 'signatures']);
      expect(EngineProfile.parse(defines['profile']), EngineProfile.debug);
    });

    test('inline-map trim shape also converts', () {
      final root = _appWith('''
name: my_app
hooks:
  user_defines:
    pdf_manipulator:
      trim: {keep: [render]}
''');
      final defines = readPdfManipulatorUserDefines(root);
      expect(defines['trim'], isA<Map<String, Object?>>());
      expect((defines['trim']! as Map)['keep'], ['render']);
    });
  });
}
