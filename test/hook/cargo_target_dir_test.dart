// The hook's cargo target dir has one default (inside the hook's own
// output dir, never the package root) and one override door: a file under
// HOME, because hooks_runner strips every env var of ours before spawning
// the hook. These tests pin both, and that a broken marker falls back to
// the default instead of pointing cargo somewhere relative.
//
// io-exempt: hook-side helper — reads a marker file under HOME by design.

import 'dart:io';

import 'package:path/path.dart' as p;
import 'package:pdf_manipulator/src/hook/engine_compiler.dart';
import 'package:test/test.dart';

void main() {
  final outputDir = Uri.directory(
    p.join(Directory.systemTemp.path, 'pdfm_hook_out'),
  );
  final homeKey = Platform.isWindows ? 'USERPROFILE' : 'HOME';

  late Directory home;
  setUp(() {
    home = Directory.systemTemp.createTempSync('pdfm_home_');
  });
  tearDown(() => home.deleteSync(recursive: true));

  File marker() =>
      File(p.join(home.path, '.pdf_manipulator', 'cargo-target-dir'));

  String defaultDir() => p.join(p.fromUri(outputDir), 'cargo_target');

  test('no marker → cargo_target inside the hook output dir', () {
    expect(
      hookCargoTargetDir(outputDir, environment: {homeKey: home.path}),
      defaultDir(),
    );
  });

  test('no HOME at all → default, never a crash', () {
    expect(hookCargoTargetDir(outputDir, environment: {}), defaultDir());
  });

  test('marker with an absolute path → that path, trimmed', () {
    final target = p.join(home.path, 'shared', 'target');
    marker()
      ..createSync(recursive: true)
      ..writeAsStringSync('$target\n');
    expect(
      hookCargoTargetDir(outputDir, environment: {homeKey: home.path}),
      target,
    );
  });

  test('marker with a relative path → ignored, default wins', () {
    marker()
      ..createSync(recursive: true)
      ..writeAsStringSync('vendor/pdf_oxide/target');
    expect(
      hookCargoTargetDir(outputDir, environment: {homeKey: home.path}),
      defaultDir(),
    );
  });

  test('empty marker → ignored, default wins', () {
    marker()
      ..createSync(recursive: true)
      ..writeAsStringSync('   \n');
    expect(
      hookCargoTargetDir(outputDir, environment: {homeKey: home.path}),
      defaultDir(),
    );
  });
}
