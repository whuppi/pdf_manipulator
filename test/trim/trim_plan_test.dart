// The trim plan is the build hook's whole trim decision, and
// recordedCapabilities is the RecordUse lane's detector. Both must be
// provable without a real hook invocation — the plan from raw user-define
// values, the extraction from an in-memory Recordings.

import 'dart:io';

import 'package:pdf_manipulator/src/hook/trim_plan.dart';
import 'package:pdf_manipulator/src/trim/capabilities.dart';
import 'package:record_use/record_use.dart';
import 'package:test/test.dart';

const _defaults =
    'icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract';

void main() {
  group('resolveTrimPlan', () {
    test('trim absent → defaults, nothing custom, nothing deferred', () async {
      final plan = await resolveTrimPlan(
        trimDefine: null,
        detectorDefine: null,
        defaultFeatures: _defaults,
        appRootCandidate: Directory.systemTemp.path,
      );
      expect(plan.features, _defaults);
      expect(plan.isCustom, isFalse);
      expect(plan.deferToLink, isFalse);
    });

    test('manual keep trims regardless of detector', () async {
      final plan = await resolveTrimPlan(
        trimDefine: {
          'keep': ['render'],
        },
        detectorDefine: 'record-use',
        defaultFeatures: _defaults,
        appRootCandidate: Directory.systemTemp.path,
      );
      expect(plan.features, 'icc,legacy-crypto,rendering,native-bridge');
      expect(plan.isCustom, isTrue);
      expect(plan.deferToLink, isFalse);
    });

    test(
      'auto + record-use defers to the link hook with a full build',
      () async {
        final plan = await resolveTrimPlan(
          trimDefine: 'auto',
          detectorDefine: 'record-use',
          defaultFeatures: _defaults,
          appRootCandidate: Directory.systemTemp.path,
        );
        expect(plan.deferToLink, isTrue);
        expect(plan.features, _defaults);
        expect(plan.isCustom, isFalse);
      },
    );

    test('auto + analyzer with no app at the candidate fails CLOSED', () async {
      final noApp = Directory.systemTemp.createTempSync('trim_plan_test');
      addTearDown(() => noApp.deleteSync(recursive: true));
      final plan = await resolveTrimPlan(
        trimDefine: 'auto',
        detectorDefine: null,
        defaultFeatures: _defaults,
        appRootCandidate: noApp.path,
      );
      expect(plan.features, _defaults);
      expect(plan.isCustom, isFalse);
    });

    test('malformed trim value propagates the loud grammar error', () {
      expect(
        () => resolveTrimPlan(
          trimDefine: 'yes',
          detectorDefine: null,
          defaultFeatures: _defaults,
          appRootCandidate: Directory.systemTemp.path,
        ),
        throwsA(isA<TrimConfigError>()),
      );
    });
  });

  group('recordedCapabilities', () {
    Recordings recordingsFor(List<CallReference> calls, {String? className}) {
      final library = const Library(
        'package:pdf_manipulator/src/trim/record_use_shim.dart',
      );
      final method = Method('op', Class(className ?? 'TrimRecord', library));
      return Recordings(calls: {method: calls}, instances: const {});
    }

    CallReference callWith(String capability) => CallWithArguments(
      positionalArguments: [StringConstant(capability)],
      namedArguments: const {},
      loadingUnit: const LoadingUnit('1'),
    );

    test('extracts capabilities from TrimRecord.op const calls', () {
      final recordings = recordingsFor([
        callWith('render'),
        callWith('office'),
      ]);
      expect(recordedCapabilities(recordings), {
        PdfCapability.render,
        PdfCapability.office,
      });
    });

    test('ignores unknown capability names (newer engine, older package)', () {
      final recordings = recordingsFor([callWith('render'), callWith('ocr')]);
      expect(recordedCapabilities(recordings), {PdfCapability.render});
    });

    test('ignores op methods on other classes', () {
      final recordings = recordingsFor([
        callWith('render'),
      ], className: 'SomeoneElse');
      expect(recordedCapabilities(recordings), isEmpty);
    });

    test('no recorded calls → empty keep-set (core-only trim)', () {
      final recordings = Recordings(calls: const {}, instances: const {});
      expect(recordedCapabilities(recordings), isEmpty);
    });
  });
}
