// The trim grammar is a public contract: three legal shapes, everything
// else fails LOUDLY. These tests pin the grammar and the loud-failure
// guarantee (a silent full-binary fallback would lie about the request).

// io-exempt: build-time tooling tests — the parity guards read README.md
// and the engine's Cargo.toml from disk; trim never runs in a browser.
import 'dart:io';

import 'package:pdf_manipulator/src/trim/capabilities.dart';
import 'package:test/test.dart';

void main() {
  group('trim grammar', () {
    test('absent and false mean off', () {
      expect(TrimConfig.parse(null).mode, TrimMode.off);
      expect(TrimConfig.parse(false).mode, TrimMode.off);
    });

    test('auto (canonical) and true (alias) mean auto', () {
      expect(TrimConfig.parse('auto').mode, TrimMode.auto);
      expect(TrimConfig.parse(true).mode, TrimMode.auto);
    });

    test('keep list parses to the exact capability set', () {
      final c = TrimConfig.parse({
        'keep': ['render', 'pdfa'],
      });
      expect(c.mode, TrimMode.manual);
      expect(c.keep, {PdfCapability.render, PdfCapability.pdfa});
    });

    test('keep expands capability dependencies (office brings extract)', () {
      final c = TrimConfig.parse({
        'keep': ['office'],
      });
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
      final c = TrimConfig.parse({
        'keep': ['core', 'render'],
      });
      expect(c.keep, {PdfCapability.render});
      expect(
        TrimConfig.parse({
          'keep': ['core'],
        }).keep,
        isEmpty,
      );
    });

    test('empty keep list is legal — core-only build', () {
      final c = TrimConfig.parse({'keep': <Object>[]});
      expect(c.mode, TrimMode.manual);
      expect(c.keep, isEmpty);
    });

    test('unknown capability fails loudly with the grammar', () {
      expect(
        () => TrimConfig.parse({
          'keep': ['rendering'], // cargo feature name, not a capability
        }),
        throwsA(
          isA<TrimConfigError>().having(
            (e) => e.message,
            'message',
            allOf(contains('rendering'), contains('Valid forms')),
          ),
        ),
      );
    });

    test('unknown map key and junk values fail loudly', () {
      expect(
        () => TrimConfig.parse({'without': <Object>[]}),
        throwsA(isA<TrimConfigError>()),
      );
      expect(() => TrimConfig.parse('yes'), throwsA(isA<TrimConfigError>()));
      expect(() => TrimConfig.parse(42), throwsA(isA<TrimConfigError>()));
    });
  });

  group('feature mapping', () {
    const defaults =
        'icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract';

    test(
      'kept capabilities survive, dropped ones leave, internals untouched',
      () {
        final c = TrimConfig.keep({PdfCapability.render});
        expect(
          c.featuresFor(defaults, c.keep!),
          'icc,legacy-crypto,rendering,native-bridge',
        );
      },
    );

    test('featuresFor expands requires itself — detector sets stay raw', () {
      // Detectors hand over unexpanded sets: office alone must still keep
      // extract, or the emitted feature list lies about what ships.
      final c = TrimConfig.keep({PdfCapability.office});
      expect(
        c.featuresFor(defaults, {PdfCapability.office}),
        'icc,legacy-crypto,native-bridge,office,extract',
      );
    });

    test('empty keep-set drops every capability feature, keeps internals', () {
      final c = TrimConfig.keep(const {});
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

  group('trim-detector selector', () {
    test('absent means analyzer', () {
      expect(TrimDetector.parse(null), TrimDetector.analyzer);
    });

    test('all three wire names parse', () {
      expect(TrimDetector.parse('analyzer'), TrimDetector.analyzer);
      expect(TrimDetector.parse('record-use'), TrimDetector.recordUse);
      expect(TrimDetector.parse('compare'), TrimDetector.compare);
    });

    test('unknown detector fails loudly with the valid set', () {
      expect(
        () => TrimDetector.parse('recorduse'),
        throwsA(
          isA<TrimConfigError>().having(
            (e) => e.message,
            'message',
            allOf(contains('recorduse'), contains('analyzer')),
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
