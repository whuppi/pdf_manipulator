// The web variant — NO @RecordUse annotation (see record_use_shim.dart):
// dart2js (Dart 3.12) crashes in SSA codegen on invocations of annotated
// statics, and web never links recordings anyway (link hooks are
// native-only). Keep the signature identical to the native variant.

/// Records that a capability-bearing op is reachable (see the conditional
/// export in record_use_shim.dart). No-op on web in every sense.
abstract final class KeepRecord {
  /// Marks [capability] (a `PdfCapability.wire` name) as used.
  static void op(String capability) {}
}
