// EXPERIMENTAL — the RecordUse trim lane (trim-detector: record-use /
// compare). Each capability-bearing public op calls [TrimRecord.op] with a
// const capability name; on AOT builds with the SDK's record-use experiment
// the compiler records reachable calls and the link hook reads them back.
// This static shim exists ONLY because @RecordUse cannot annotate instance
// methods yet (dart-lang/native#2902) — delete it and annotate the ops
// directly when that ships. The call is an empty static: zero behavior,
// tree-shaken like any other no-op when recording is off.
//
// Two variants: web compilers get the un-annotated one. dart2js (Dart
// 3.12) crashes in SSA codegen on invocations of @RecordUse-annotated
// statics, and recordings are meaningless on web anyway — link hooks
// only run for native targets. Keep both variants' signatures identical.

export 'record_use_shim_native.dart'
    if (dart.library.js_interop) 'record_use_shim_web.dart';
