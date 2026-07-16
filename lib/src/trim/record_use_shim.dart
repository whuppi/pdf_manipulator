// EXPERIMENTAL — the RecordUse trim lane (trim-detector: record-use /
// compare). Each capability-bearing public op calls [TrimRecord.op] with a
// const capability name; on AOT builds with the SDK's record-use experiment
// the compiler records reachable calls and the link hook reads them back.
// This static shim exists ONLY because @RecordUse cannot annotate instance
// methods yet (dart-lang/native#2902) — delete it and annotate the ops
// directly when that ships. The call is an empty static: zero behavior,
// tree-shaken like any other no-op when recording is off.

import 'package:meta/meta.dart' show RecordUse;

/// Records that a capability-bearing op is reachable (see library comment).
abstract final class TrimRecord {
  /// Marks [capability] (a `PdfCapability.wire` name) as used.
  @RecordUse()
  static void op(String capability) {}
}
