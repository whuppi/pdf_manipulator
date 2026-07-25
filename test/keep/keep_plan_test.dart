// The keep plan is the build hook's whole keep decision, and
// recordedCapabilities is the RecordUse lane's detector. Both must be
// provable without a real hook invocation — the plan from raw user-define
// values, the extraction from an in-memory Recordings.

// io-exempt: build-time tooling tests — the fail-closed plan cases probe
// real directories for a pubspec; keep never runs in a browser.
import 'dart:io';

import 'package:pdf_manipulator/src/hook/keep_plan.dart';
import 'package:pdf_manipulator/src/keep/capabilities.dart';
import 'package:record_use/record_use.dart';
import 'package:test/test.dart';

const _defaults =
    'icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract';

void main() {
  group('resolveKeepPlan', () {
    test('keep all → defaults, nothing custom, nothing deferred', () async {
      final plan = await resolveKeepPlan(
        keep: KeepConfig.all,
        detector: KeepDetector.scan,
        defaultFeatures: _defaults,
        appRootCandidate: Directory.systemTemp.path,
      );
      expect(plan.features, _defaults);
      expect(plan.isCustom, isFalse);
      expect(plan.deferToLink, isFalse);
    });

    test('manual keep list trims regardless of detector', () async {
      final plan = await resolveKeepPlan(
        keep: KeepConfig.parse(['render']),
        detector: KeepDetector.recordUse,
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
        final plan = await resolveKeepPlan(
          keep: KeepConfig.auto,
          detector: KeepDetector.recordUse,
          defaultFeatures: _defaults,
          appRootCandidate: Directory.systemTemp.path,
        );
        expect(plan.deferToLink, isTrue);
        expect(plan.features, _defaults);
        expect(plan.isCustom, isFalse);
      },
    );

    test('auto + scan with no app at the candidate fails CLOSED', () async {
      final noApp = Directory.systemTemp.createTempSync('keep_plan_test');
      addTearDown(() => noApp.deleteSync(recursive: true));
      final plan = await resolveKeepPlan(
        keep: KeepConfig.auto,
        detector: KeepDetector.scan,
        defaultFeatures: _defaults,
        appRootCandidate: noApp.path,
      );
      expect(plan.features, _defaults);
      expect(plan.isCustom, isFalse);
    });
  });

  group('recordedCapabilities', () {
    Recordings recordingsFor(List<CallReference> calls, {String? className}) {
      final library = const Library(
        'package:pdf_manipulator/src/keep/record_use_shim.dart',
      );
      final method = Method('op', Class(className ?? 'KeepRecord', library));
      return Recordings(calls: {method: calls}, instances: const {});
    }

    CallReference callWith(String capability) => CallWithArguments(
      positionalArguments: [StringConstant(capability)],
      namedArguments: const {},
      loadingUnit: const LoadingUnit('1'),
    );

    test('extracts capabilities from KeepRecord.op const calls', () {
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
