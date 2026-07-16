// The trim grammar is a public contract: three legal shapes, everything
// else fails LOUDLY. These tests pin the grammar and the loud-failure
// guarantee (a silent full-binary fallback would lie about the request).

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
        'icc,legacy-crypto,rendering,signatures,native-bridge,pdfa';

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
}
