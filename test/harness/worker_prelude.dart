// Web-only: runs a script inside every Web Worker the page spawns while a
// body runs, before the worker's own script.
//
// The host boots a cross-origin lane worker through a blob that imports it;
// the prelude takes the same route one step earlier, so a test can patch a
// browser API (OPFS deletes, XMLHttpRequest, message delivery) or hold a
// worker at a point it chooses, in the worker the host really boots.

import 'dart:js_interop';
import 'dart:js_interop_unsafe';

const _install = r'''
(function (prelude) {
  const Real = globalThis.Worker;
  globalThis.Worker = class extends Real {
    constructor(url, options) {
      const script = prelude + '\nawait import(' + JSON.stringify(String(url)) + ');';
      super(URL.createObjectURL(new Blob([script], { type: 'application/javascript' })), options);
    }
  };
  return () => { globalThis.Worker = Real; };
})
''';

/// Runs [body] with [prelude] (module code) in front of every worker it
/// spawns. Workers keep their prelude after [body] returns.
Future<T> withWorkerPrelude<T>(
  String prelude,
  Future<T> Function() body,
) async {
  final install = globalContext.callMethod<JSFunction>(
    'eval'.toJS,
    _install.toJS,
  );
  final restore = install.callAsFunction(null, prelude.toJS)! as JSFunction;
  try {
    return await body();
  } finally {
    restore.callAsFunction();
  }
}
