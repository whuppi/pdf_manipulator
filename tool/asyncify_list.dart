// asyncify_list.dart — which functions of a built engine wasm can be on the
// call stack while the module is paused at a host import, from the module's
// own call graph.
//
//   dart --packages=.dart_tool/package_config.json tool/asyncify_list.dart \
//        [--out <file>] [--pause <import-substring>]... <whole wasm>
//
// binaryen's `--asyncify` instruments exactly the functions it is told to
// (`--pass-arg=asyncify-onlylist@@<file>`); a function left out of that list
// must never be on the stack at a pause. This tool computes the list that is
// safe to give: the callers of the pausing imports, transitively, where a
// `call_indirect` reaches every function of its signature that an element
// segment or a `ref.func` names. It refuses a module whose function tables
// could change at run time (`table.set`, `table.init`, `table.copy`,
// `table.grow`, `table.fill` on a funcref table, an imported table), because
// the list would then be unsound.
//
// Prints the counts that decide whether the list is worth having: the direct
// callers alone, the signature-resolved set, and for each signature the
// functions it adds. `--out` writes the list in the form the pass reads: one
// comma-separated line of names: the module has no name section, and binaryen
// names an unnamed defined function by its index among the defined functions,
// imports not counted.
//
// `--pause` defaults to the two imports the executor worker suspends,
// `host_read_at` and `host_write_chunk`. Run from the package root; `dart run`
// would first run the package's native build hook, hence `dart --packages=`.

import 'dart:io';

import 'package:pdf_manipulator/src/hook/asyncify_list.dart';

void main(List<String> args) {
  final pause = <String>[];
  String? out;
  String? path;
  for (var i = 0; i < args.length; i++) {
    switch (args[i]) {
      case '--out':
        out = args[++i];
      case '--pause':
        pause.add(args[++i]);
      default:
        path = args[i];
    }
  }
  if (path == null) {
    stderr.writeln(
      'usage: dart --packages=.dart_tool/package_config.json '
      'tool/asyncify_list.dart [--out file] [--pause import]... <wasm>',
    );
    exit(2);
  }
  if (pause.isEmpty) pause.addAll(asyncifyPauseImports);

  final bytes = File(path).readAsBytesSync();
  final m = WasmCallGraph.parse(bytes);
  final targets = m.importsNamed(pause);

  final direct = m.ancestors(targets, indirect: false);
  final all = m.ancestors(targets, indirect: true);
  final bySig = m.signatureSteps(targets);
  stdout
    ..writeln(
      'functions: ${m.functionCount} (${m.importCount} imports), '
      'address-taken: ${m.addressTaken.length}',
    )
    ..writeln('pausing imports: ${targets.map((t) => m.importNames[t])}')
    ..writeln('callers by direct calls only: ${direct.length}')
    ..writeln(
      'callers with call_indirect resolved by signature: ${all.length} '
      '(${(100 * all.length / m.functionCount).toStringAsFixed(1)}% of the '
      'module)',
    )
    ..writeln('signature resolution, in the order it grows the set:');
  for (final step in bySig) {
    stdout.writeln(
      '  ${step.signature}  address-taken ancestors ${step.taken}, '
      'call-site functions ${step.sites}, adds ${step.added}',
    );
  }
  if (out != null) {
    final list = asyncifyList(bytes);
    File(out).writeAsStringSync(list);
    stdout.writeln('wrote ${list.split(',').length} function names to $out');
  }
}
