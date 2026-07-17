// The annotated variant — VM / native AOT only (see record_use_shim.dart).

import 'package:meta/meta.dart' show RecordUse;

/// Records that a capability-bearing op is reachable (see the conditional
/// export in record_use_shim.dart).
abstract final class TrimRecord {
  /// Marks [capability] (a `PdfCapability.wire` name) as used.
  @RecordUse()
  static void op(String capability) {}
}
