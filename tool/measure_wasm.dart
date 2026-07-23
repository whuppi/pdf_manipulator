// Builds one wasm variant to a temp dir and prints `raw=<bytes> gz=<bytes>`.
//
//   dart run tool/measure_wasm.dart <features> [optLevel]
//     features  cargo feature string, e.g. "wasm,rendering,signatures,..."
//     optLevel  cargo release opt-level (e.g. "z" for the `build: size`
//               engine); omit for the default (speed) build.
//
// Staging goes to a fresh temp dir via compileWasmEngine's outDir — the same
// mechanism a trimmed / non-default consumer build uses — so web_assets/ (the
// committed default artifact) is never touched. `shake_audit.sh` calls this to
// measure the trimmed and size-build wasm without any backup-and-restore.

import 'dart:io';

import 'package:logging/logging.dart';
import 'package:pdf_manipulator/src/hook/engine_compiler.dart';

Future<void> main(List<String> args) async {
  if (args.isEmpty) {
    stderr.writeln('usage: dart run tool/measure_wasm.dart <features> [optLevel]');
    exit(2);
  }
  Logger.root.onRecord.listen((r) => stderr.writeln(r.message));

  final features = args[0];
  final cargoEnv = (args.length > 1 && args[1].isNotEmpty)
      ? {'CARGO_PROFILE_RELEASE_OPT_LEVEL': args[1]}
      : const <String, String>{};

  final out = Directory.systemTemp.createTempSync('measure_wasm_');
  try {
    await compileWasmEngine(
      packageRoot: Directory.current.uri,
      features: features,
      outDir: out,
      cargoEnv: cargoEnv,
    );
    final wasm = File('${out.path}/pdf_oxide_bg.wasm');
    final raw = wasm.lengthSync();
    final gz = gzip.encode(wasm.readAsBytesSync()).length;
    stdout.writeln('raw=$raw gz=$gz');
  } finally {
    out.deleteSync(recursive: true);
  }
}
