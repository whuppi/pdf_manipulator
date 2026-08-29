// The hook must scan the CONSUMING APP, not pdf_manipulator itself.
//
// Every other test in test/keep/ hands the app root in — `detectCapabilities(
// dir.path)`, `appRootCandidate: Directory.systemTemp.path`. That proves the
// detector works when pointed at an app; none of them can prove the hook
// points it at one. The defect lived on exactly that line.
//
// So this file tests the seam the others skip: the path from a hook's output
// directory to a keep decision, plus a guard that the hook actually walks it.
// io-exempt: builds a throwaway fixture app on disk — the hook's app-root
// resolution can only be proven against a real directory tree.
import 'dart:io';

import 'package:pdf_manipulator/src/hook/app_root.dart';
import 'package:pdf_manipulator/src/hook/keep_plan.dart';
import 'package:pdf_manipulator/src/keep/capabilities.dart';
import 'package:test/test.dart';

/// A throwaway app calling exactly ONE capability.
///
/// `extract` is a real detector marker (`PdfDoc.extract`); `extractText` is
/// not, and an app calling only that yields an EMPTY keep-set — which happens
/// to satisfy "is custom" and "has no signatures" too, so a fixture using it
/// passes this test vacuously.
///
/// Scanned, this yields `extract` alone. pdf_manipulator's own source yields
/// every capability, because that is where they are all implemented.
Directory _appUsingOnlyExtract() {
  final app = Directory.systemTemp.createTempSync('pdf_keep_app_');
  addTearDown(() {
    if (app.existsSync()) app.deleteSync(recursive: true);
  });
  File('${app.path}/pubspec.yaml').writeAsStringSync('name: fixture_app\n');
  Directory('${app.path}/lib').createSync(recursive: true);
  File('${app.path}/lib/app.dart').writeAsStringSync('''
import 'package:pdf_manipulator/pdf_manipulator.dart';

final pdf = Pdf();

Future<void> readIt(PdfDoc doc) async {
  await doc.extract();
}
''');
  return app;
}

/// Where the runner puts a hook's output for [app].
Uri _hookOutputFor(Directory app) => Uri.directory(
  '${app.path}/.dart_tool/hooks_runner/shared/pdf_manipulator/build/9fb02c/',
);

void main() {
  test('both lanes anchors resolve to the app that owns them', () {
    final app = _appUsingOnlyExtract();

    // Native: the hook's output directory.
    expect(appRootFromDartTool(_hookOutputFor(app)), app.path);
    // Web: the package config the VM is running under — a different path
    // under the SAME `.dart_tool/`, so one derivation serves both and they
    // cannot disagree about which directory the app is.
    expect(
      appRootFromDartTool(
        Uri.file('${app.path}/.dart_tool/package_config.json'),
      ),
      app.path,
    );
    // No `.dart_tool` segment: the app is NOT located. Callers must build.
    expect(appRootFromDartTool(Uri.directory('/elsewhere/build/')), isNull);
  });

  test('driven from that path, the plan trims to what the app uses', () async {
    final app = _appUsingOnlyExtract();

    final plan = await resolveKeepPlan(
      keep: KeepConfig.auto,
      detector: KeepDetector.scan,
      defaultFeatures: 'render,extract,signatures,office,pdfa',
      appRootCandidate: appRootFromDartTool(_hookOutputFor(app))!,
      scanDirs: const [],
    );

    // isCustom is the whole consequence: custom compiles a trimmed engine,
    // non-custom downloads the full prebuilt. An app calling one capability
    // must be custom, or "trimming" ships the same bytes as no trimming.
    expect(plan.isCustom, isTrue, reason: 'kept everything: ${plan.features}');
    // Both halves matter. Without the first, an empty keep-set — the scan
    // finding NOTHING — passes just as well as a correct one.
    expect(plan.features, contains('extract'));
    expect(plan.features, isNot(contains('signatures')));
  });

  test('no volatile path reaches the dependency set', () async {
    // The runner takes a directory dependency's lastModified RECURSIVELY
    // and, when it lands after its cutoff, stores a sentinel hash that
    // re-runs the hook every build. `.dart_tool/` is rewritten by the build
    // itself, so a dependency covering it means a full engine recompile on
    // every single build under `keep: auto`.
    final app = _appUsingOnlyExtract();
    // A pure-Dart layout (bin/, no lib/) used to fall back to scanning the
    // bare app root, which contains .dart_tool/.
    Directory(
      '${app.path}/.dart_tool/build/generated',
    ).createSync(recursive: true);
    File(
      '${app.path}/.dart_tool/generated.dart',
    ).writeAsStringSync('void sign() {}\n');

    final plan = await resolveKeepPlan(
      keep: KeepConfig.auto,
      detector: KeepDetector.scan,
      defaultFeatures: 'render,extract,signatures,office,pdfa',
      appRootCandidate: appRootFromDartTool(_hookOutputFor(app))!,
      scanDirs: const [],
    );
    final paths = plan.appDependencies.map((u) => u.toFilePath()).toList();

    expect(
      paths.where((p) => p.contains('.dart_tool')),
      isEmpty,
      reason: 'a build-volatile path became a hook dependency: $paths',
    );
    // And the generated source must not be scanned as app usage either.
    expect(plan.features, isNot(contains('signatures')));
  });

  test('the plan reports the app source it read', () async {
    final app = _appUsingOnlyExtract();

    final plan = await resolveKeepPlan(
      keep: KeepConfig.auto,
      detector: KeepDetector.scan,
      defaultFeatures: 'render,extract,signatures,office,pdfa',
      appRootCandidate: appRootFromDartTool(_hookOutputFor(app))!,
      scanDirs: const [],
    );
    final paths = plan.appDependencies.map((u) => u.toFilePath()).toSet();

    expect(paths, contains('${app.path}/pubspec.yaml'));
    // The directory catches a file appearing or disappearing...
    expect(paths, contains('${app.path}/lib/'));
    // ...and the file catches an edit, which the directory's mtime does not.
    expect(paths, contains('${app.path}/lib/app.dart'));
  });

  test('every walked directory is registered, not just the roots', () async {
    // The runner hashes a directory's IMMEDIATE child names (recursive:
    // false). Registering `lib/` alone therefore notices a file added
    // directly in `lib/`, but NOT one added in `lib/models/` — that leaves
    // `lib/`'s child names untouched, so the stale keep decision stands and
    // the new call site is never seen.
    final app = _appUsingOnlyExtract();
    Directory('${app.path}/lib/models').createSync(recursive: true);
    File('${app.path}/lib/models/doc.dart').writeAsStringSync('// nothing\n');
    // A subdirectory with no .dart at all still has to invalidate when the
    // first one lands in it, and only its own hash sees that.
    Directory('${app.path}/lib/assets').createSync(recursive: true);

    final plan = await resolveKeepPlan(
      keep: KeepConfig.auto,
      detector: KeepDetector.scan,
      defaultFeatures: 'render,extract,signatures,office,pdfa',
      appRootCandidate: appRootFromDartTool(_hookOutputFor(app))!,
      scanDirs: const [],
    );
    final paths = plan.appDependencies.map((u) => u.toFilePath()).toSet();

    expect(paths, contains('${app.path}/lib/'));
    expect(paths, contains('${app.path}/lib/models/'));
    expect(paths, contains('${app.path}/lib/assets/'));
  });

  test(
    'a dot-directory the USER made is scanned; tool trees are not',
    () async {
      // Skipping every `.`-prefixed directory would be wider than the hazard:
      // `lib/.generated/` is the app's own source, and dropping it silently
      // trims away a capability it calls. Only named tool trees are skipped.
      final app = _appUsingOnlyExtract();
      Directory('${app.path}/lib/.generated').createSync(recursive: true);
      File('${app.path}/lib/.generated/api.dart').writeAsStringSync('''
import 'package:pdf_manipulator/pdf_manipulator.dart';

Future<void> check(PdfDoc doc) async {
  await doc.getSignatures();
}
''');
      // Tool-owned, and `lib/build/` which is the app's own code, not output.
      Directory('${app.path}/lib/.dart_tool').createSync(recursive: true);
      File(
        '${app.path}/lib/.dart_tool/gen.dart',
      ).writeAsStringSync('void validatePdfA() {}\n');
      Directory('${app.path}/lib/build').createSync(recursive: true);
      File('${app.path}/lib/build/maker.dart').writeAsStringSync('''
import 'package:pdf_manipulator/pdf_manipulator.dart';

Future<void> shrink(PdfDoc doc) async {
  await doc.render(pages: PdfPages.all());
}
''');

      final plan = await resolveKeepPlan(
        keep: KeepConfig.auto,
        detector: KeepDetector.scan,
        defaultFeatures: 'render,extract,signatures,office,pdfa',
        appRootCandidate: appRootFromDartTool(_hookOutputFor(app))!,
        scanDirs: const [],
      );

      // The user's dot-directory counted.
      expect(plan.features, contains('signatures'));
      // `lib/build/` is NOT the SDK's output directory — it is app source.
      expect(plan.features, contains('render'));
      // A tool tree never counts, wherever it sits.
      expect(plan.features, isNot(contains('pdfa')));

      final paths = plan.appDependencies.map((u) => u.toFilePath()).toSet();
      expect(paths, contains('${app.path}/lib/.generated/'));
      expect(paths.any((p) => p.contains('.dart_tool')), isFalse);
    },
  );

  test('scan-dirs widens the scan to a directory outside lib/', () async {
    final app = _appUsingOnlyExtract();
    // Source the default scan cannot see: not lib/, not bin/.
    Directory('${app.path}/tools').createSync();
    File('${app.path}/tools/report.dart').writeAsStringSync('''
import 'package:pdf_manipulator/pdf_manipulator.dart';

Future<void> check(PdfDoc doc) async {
  await doc.getSignatures();
}
''');

    Future<KeepPlan> planWith(List<String> scanDirs) => resolveKeepPlan(
      keep: KeepConfig.auto,
      detector: KeepDetector.scan,
      defaultFeatures: 'render,extract,signatures,office,pdfa',
      appRootCandidate: appRootFromDartTool(_hookOutputFor(app))!,
      scanDirs: scanDirs,
    );

    // Without it the tools/ call is invisible — the under-keep this closes.
    expect((await planWith(const [])).features, isNot(contains('signatures')));

    final widened = await planWith(const ['tools']);
    expect(widened.features, contains('signatures'));
    expect(widened.features, contains('extract'));
    // Still a trim, not a silent fallback to everything.
    expect(widened.isCustom, isTrue);
    // The widened directory is registered too, or editing it would not
    // re-run the hook.
    final paths = widened.appDependencies.map((u) => u.toFilePath()).toSet();
    expect(paths, contains('${app.path}/tools/report.dart'));
  });

  test(
    'a scan-dir that does not exist falls closed to the full binary',
    () async {
      final app = _appUsingOnlyExtract();
      const defaults = 'render,extract,signatures,office,pdfa';

      final plan = await resolveKeepPlan(
        keep: KeepConfig.auto,
        detector: KeepDetector.scan,
        defaultFeatures: defaults,
        appRootCandidate: appRootFromDartTool(_hookOutputFor(app))!,
        scanDirs: const ['tolos'], // typo for `tools`
      );

      // A typo must not quietly narrow the scan into a confident wrong trim.
      expect(plan.features, defaults);
      expect(plan.isCustom, isFalse);
    },
  );

  test('an explicit keep-set depends on nothing in the app', () async {
    final app = _appUsingOnlyExtract();

    final plan = await resolveKeepPlan(
      keep: KeepConfig.keep({PdfCapability.extract}),
      detector: KeepDetector.scan,
      defaultFeatures: 'render,extract,signatures,office,pdfa',
      appRootCandidate: appRootFromDartTool(_hookOutputFor(app))!,
      scanDirs: const [],
    );

    // It read no app source, so no app edit should invalidate it.
    expect(plan.appDependencies, isEmpty);
  });

  test('the hook registers the app source it read as a dependency', () {
    // The decision is derived from the APP's source, so the app's source
    // has to be a hook dependency — otherwise the runner caches the answer
    // and never asks again. An app that later calls `sign` would keep
    // building the engine trimmed without it, and fail at runtime.
    final hook = File('hook/build.dart')
        .readAsStringSync()
        .split('\n')
        .where((line) => !line.trimLeft().startsWith('//'))
        .join('\n');

    expect(
      hook,
      contains('appDependencies'),
      reason: 'the keep decision is cached forever — nothing invalidates it',
    );
  });

  test('the hook derives the app root, not Directory.current', () {
    // The runner starts every hook with its working directory set to the
    // package's own root (hooks_runner: `workingDirectory =
    // input.packageRoot`). Reading it scans pdf_manipulator, finds every
    // capability where it is implemented, and keeps all of them — silently,
    // while reporting success. The two tests above only matter if the hook
    // actually walks that seam, so this pins the wiring.
    // Comments are stripped first, for the same reason detectCapabilities
    // blanks them: prose naming the banned thing is not a use of it, and
    // the comment above the fixed line says "NOT Directory.current".
    final hook = File('hook/build.dart')
        .readAsStringSync()
        .split('\n')
        .where((line) => !line.trimLeft().startsWith('//'))
        .join('\n');

    expect(
      hook,
      isNot(contains('Directory.current')),
      reason: 'the hook read its own working directory as the app root',
    );
    expect(hook, contains('appRootFromDartTool'));
  });
}
