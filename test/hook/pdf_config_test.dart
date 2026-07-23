// PdfManipulatorConfig is the ONE parser both the native hook and web setup
// go through. Its contract: invalid configs are unrepresentable — every bad
// value, unknown key, and axis mismatch fails LOUDLY, never a silent ignore.
// These tests pin that contract.

import 'package:pdf_manipulator/src/hook/engine_build.dart';
import 'package:pdf_manipulator/src/hook/pdf_config.dart';
import 'package:pdf_manipulator/src/keep/capabilities.dart';
import 'package:test/test.dart';

void main() {
  group('PdfManipulatorConfig.parse', () {
    test('empty block → defaults (keep all, scan, speed)', () {
      final cfg = PdfManipulatorConfig.parse(const {});
      expect(cfg.keep.mode, KeepMode.all);
      expect(cfg.detector, KeepDetector.scan);
      expect(cfg.build, EngineBuild.speed);
    });

    test('all three axes parse together', () {
      final cfg = PdfManipulatorConfig.parse(const {
        'keep': 'auto',
        'detector': 'record-use',
        'build': 'size',
      });
      expect(cfg.keep.mode, KeepMode.auto);
      expect(cfg.detector, KeepDetector.recordUse);
      expect(cfg.build, EngineBuild.size);
    });

    test('explicit keep list + build, no detector', () {
      final cfg = PdfManipulatorConfig.parse(const {
        'keep': ['render'],
        'build': 'debug',
      });
      expect(cfg.keep.mode, KeepMode.manual);
      expect(cfg.build, EngineBuild.debug);
    });

    group('impossible to configure wrong', () {
      test('unknown key fails loudly, naming it and the valid keys', () {
        expect(
          () => PdfManipulatorConfig.parse(const {'keeep': 'auto'}),
          throwsA(
            isA<PdfConfigError>().having(
              (e) => e.message,
              'message',
              allOf(contains('keeep'), contains('keep, detector, build')),
            ),
          ),
        );
      });

      test('detector with an explicit keep list is rejected (axis mismatch)', () {
        expect(
          () => PdfManipulatorConfig.parse(const {
            'keep': ['render'],
            'detector': 'scan',
          }),
          throwsA(
            isA<PdfConfigError>().having(
              (e) => e.message,
              'message',
              contains('detector: only applies to `keep: auto`'),
            ),
          ),
        );
      });

      test('detector with keep absent (= all) is rejected', () {
        expect(
          () => PdfManipulatorConfig.parse(const {'detector': 'record-use'}),
          throwsA(isA<PdfConfigError>()),
        );
      });

      test('detector IS allowed with keep: auto', () {
        final cfg = PdfManipulatorConfig.parse(const {
          'keep': 'auto',
          'detector': 'compare',
        });
        expect(cfg.detector, KeepDetector.compare);
      });

      test('bad build value fails loudly', () {
        expect(
          () => PdfManipulatorConfig.parse(const {'build': 'fast'}),
          throwsA(
            isA<PdfConfigError>().having(
              (e) => e.message,
              'message',
              allOf(contains('fast'), contains('speed')),
            ),
          ),
        );
      });

      test('bad keep value fails loudly', () {
        expect(
          () => PdfManipulatorConfig.parse(const {'keep': 'yes'}),
          throwsA(isA<PdfConfigError>()),
        );
      });
    });
  });
}
