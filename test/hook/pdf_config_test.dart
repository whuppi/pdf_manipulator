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

      test(
        'detector with an explicit keep list is rejected (axis mismatch)',
        () {
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
        },
      );

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

    group('scan-dirs', () {
      test('absent → empty, and it parses with keep: auto', () {
        expect(PdfManipulatorConfig.parse(const {}).scanDirs, isEmpty);
        final cfg = PdfManipulatorConfig.parse(const {
          'keep': 'auto',
          'scan-dirs': ['tools', 'packages/shared/lib'],
        });
        expect(cfg.scanDirs, ['tools', 'packages/shared/lib']);
      });

      // It widens the SOURCE SCAN, so it is stranded on any config that does
      // not scan — the same rule `detector` follows against `keep`.
      test('rejected without keep: auto', () {
        expect(
          () => PdfManipulatorConfig.parse(const {
            'keep': ['render'],
            'scan-dirs': ['tools'],
          }),
          throwsA(
            isA<PdfConfigError>().having(
              (e) => e.message,
              'message',
              allOf(contains('scan-dirs'), contains('keep: auto')),
            ),
          ),
        );
      });

      test('rejected with detector: record-use (it never reads source)', () {
        expect(
          () => PdfManipulatorConfig.parse(const {
            'keep': 'auto',
            'detector': 'record-use',
            'scan-dirs': ['tools'],
          }),
          throwsA(
            isA<PdfConfigError>().having(
              (e) => e.message,
              'message',
              allOf(contains('scan-dirs'), contains('record-use')),
            ),
          ),
        );
      });

      test('allowed with detector: compare (compare trims with the scan)', () {
        final cfg = PdfManipulatorConfig.parse(const {
          'keep': 'auto',
          'detector': 'compare',
          'scan-dirs': ['tools'],
        });
        expect(cfg.scanDirs, ['tools']);
      });

      test('a non-list value fails loudly', () {
        expect(
          () => PdfManipulatorConfig.parse(const {
            'keep': 'auto',
            'scan-dirs': 'tools',
          }),
          throwsA(isA<PdfConfigError>()),
        );
      });

      test('an empty or non-string entry fails loudly', () {
        for (final bad in [
          const ['tools', ''],
          const ['tools', null],
          const ['tools', 7],
        ]) {
          expect(
            () =>
                PdfManipulatorConfig.parse({'keep': 'auto', 'scan-dirs': bad}),
            throwsA(isA<PdfConfigError>()),
            reason: 'accepted $bad',
          );
        }
      });

      // A committed pubspec is shared: an absolute path is right on exactly
      // one machine and wrong in CI and every other checkout.
      test('an absolute path fails loudly', () {
        for (final abs in ['/Users/me/app/tools', r'C:\app\tools']) {
          expect(
            () => PdfManipulatorConfig.parse({
              'keep': 'auto',
              'scan-dirs': [abs],
            }),
            throwsA(
              isA<PdfConfigError>().having(
                (e) => e.message,
                'message',
                contains('relative'),
              ),
            ),
            reason: 'accepted $abs',
          );
        }
      });
    });
  });
}
