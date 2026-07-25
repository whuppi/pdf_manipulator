// The keep grammar is a public contract: three legal shapes, everything
// else fails LOUDLY. These tests pin the grammar and the loud-failure
// guarantee (a silent full-binary fallback would lie about the request).

// io-exempt: build-time tooling tests — the parity guards read README.md
// and the engine's Cargo.toml from disk; keep never runs in a browser.
import 'dart:io';

import 'package:pdf_manipulator/src/keep/capabilities.dart';
import 'package:test/test.dart';

void main() {
  group('keep grammar', () {
    test('absent or `all` means keep everything', () {
      expect(KeepConfig.parse(null).mode, KeepMode.all);
      expect(KeepConfig.parse('all').mode, KeepMode.all);
    });

    test('auto means auto', () {
      expect(KeepConfig.parse('auto').mode, KeepMode.auto);
    });

    test('a list parses to the exact capability set', () {
      final c = KeepConfig.parse(['render', 'pdfa']);
      expect(c.mode, KeepMode.manual);
      expect(c.keep, {PdfCapability.render, PdfCapability.pdfa});
    });

    test('a list expands capability dependencies (office brings extract)', () {
      final c = KeepConfig.parse(['office']);
      expect(c.keep, {PdfCapability.office, PdfCapability.extract});
      expect(
        c.featuresFor(
          'icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract',
          c.keep!,
        ),
        'icc,legacy-crypto,native-bridge,office,extract',
      );
    });

    test('core is accepted and adds nothing (always included)', () {
      final c = KeepConfig.parse(['core', 'render']);
      expect(c.keep, {PdfCapability.render});
      expect(KeepConfig.parse(['core']).keep, isEmpty);
    });

    test('empty list is legal — core-only build', () {
      final c = KeepConfig.parse(<Object>[]);
      expect(c.mode, KeepMode.manual);
      expect(c.keep, isEmpty);
    });

    test('unknown capability fails loudly with the grammar', () {
      expect(
        () =>
            KeepConfig.parse(['rendering']), // cargo feature, not a capability
        throwsA(
          isA<PdfConfigError>().having(
            (e) => e.message,
            'message',
            allOf(contains('rendering'), contains('Valid forms')),
          ),
        ),
      );
    });

    test('a map value fails loudly (keep is a list, not a map)', () {
      expect(
        () => KeepConfig.parse({'render': true}),
        throwsA(
          isA<PdfConfigError>().having(
            (e) => e.message,
            'message',
            allOf(contains('list'), contains('Valid forms')),
          ),
        ),
      );
    });

    test('junk scalar values fail loudly (no true/false aliases)', () {
      expect(() => KeepConfig.parse('yes'), throwsA(isA<PdfConfigError>()));
      expect(() => KeepConfig.parse(42), throwsA(isA<PdfConfigError>()));
      expect(() => KeepConfig.parse(true), throwsA(isA<PdfConfigError>()));
      expect(() => KeepConfig.parse(false), throwsA(isA<PdfConfigError>()));
    });
  });

  group('feature mapping', () {
    const defaults =
        'icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract';

    test(
      'kept capabilities survive, dropped ones leave, internals untouched',
      () {
        final c = KeepConfig.keep({PdfCapability.render});
        expect(
          c.featuresFor(defaults, c.keep!),
          'icc,legacy-crypto,rendering,native-bridge',
        );
      },
    );

    test('featuresFor expands requires itself — detector sets stay raw', () {
      // Detectors hand over unexpanded sets: office alone must still keep
      // extract, or the emitted feature list lies about what ships.
      final c = KeepConfig.keep({PdfCapability.office});
      expect(
        c.featuresFor(defaults, {PdfCapability.office}),
        'icc,legacy-crypto,native-bridge,office,extract',
      );
    });

    test('empty keep-set drops every capability feature, keeps internals', () {
      final c = KeepConfig.keep(const {});
      expect(
        c.featuresFor(defaults, c.keep!),
        'icc,legacy-crypto,native-bridge',
      );
    });

    test('every capability maps to a distinct cargo feature', () {
      final features = PdfCapability.values.map((c) => c.cargoFeature).toSet();
      expect(features.length, PdfCapability.values.length);
    });

    test('every detector member maps to a real capability', () {
      expect(PdfCapability.apiMembers.values.toSet(), isNotEmpty);
      for (final name in PdfCapability.apiMembers.keys) {
        expect(name, isNot(contains(' ')));
      }
    });
  });

  group('detector selector', () {
    test('absent means scan', () {
      expect(KeepDetector.parse(null), KeepDetector.scan);
    });

    test('all three wire names parse', () {
      expect(KeepDetector.parse('scan'), KeepDetector.scan);
      expect(KeepDetector.parse('record-use'), KeepDetector.recordUse);
      expect(KeepDetector.parse('compare'), KeepDetector.compare);
    });

    test('unknown detector fails loudly with the valid set', () {
      expect(
        () => KeepDetector.parse('recorduse'),
        throwsA(
          isA<PdfConfigError>().having(
            (e) => e.message,
            'message',
            allOf(contains('recorduse'), contains('scan')),
          ),
        ),
      );
    });
  });

  group('README capability table', () {
    test('lists every detector member (drift guard)', () {
      final readme = File('README.md').readAsStringSync();
      final start = readme.indexOf('| Capability | Keep it if you call |');
      expect(start, greaterThan(0), reason: 'capability table missing');
      final table = readme.substring(start, readme.indexOf('\n\n', start));
      for (final member in PdfCapability.apiMembers.keys) {
        final method = member.split('.').last;
        expect(
          table.contains(method),
          isTrue,
          reason: '$method (from apiMembers) is not in the README table',
        );
      }
      for (final cap in PdfCapability.values) {
        for (final dep in cap.requires) {
          final row = table
              .split('\n')
              .firstWhere((l) => l.contains('`${cap.wire}`'));
          expect(
            row.contains('`${dep.wire}`'),
            isTrue,
            reason:
                "README row for '${cap.wire}' does not show its "
                "'${dep.wire}' dependency",
          );
        }
      }
    });
  });

  group('engine parity', () {
    test('enum requires mirrors the cargo feature graph (drift guard)', () {
      final manifest = File('vendor/pdf_oxide/Cargo.toml').readAsStringSync();
      for (final cap in PdfCapability.values) {
        // The engine feature line, e.g. `office = ["dep:office_oxide", "extract"]`.
        final line = RegExp(
          '^${cap.cargoFeature} = \\[(.*?)\\]',
          multiLine: true,
          dotAll: true,
        ).firstMatch(manifest);
        expect(line, isNotNull, reason: 'engine feature ${cap.cargoFeature}');
        final deps = line!.group(1)!;
        for (final other in PdfCapability.values) {
          final engineDepends = deps.contains('"${other.cargoFeature}"');
          final enumDepends = cap.requires.contains(other);
          expect(
            enumDepends,
            engineDepends,
            reason:
                '${cap.wire} → ${other.wire}: enum says $enumDepends, '
                'engine manifest says $engineDepends',
          );
        }
      }
    });
  });
}
