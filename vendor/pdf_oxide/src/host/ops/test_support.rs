// Ops that exist only to let a test drive a failure no real input can reach
// on demand. Built under the `test-support` feature, never in a shipped
// engine; outside the wire contract, so the Dart op enum does not name them.

use crate::host::ops::op_unit;

op_unit!(PANIC, "testPanic", pdf_op_test_panic_anchor, |_ctx| {
    panic!("testPanic: deliberate engine panic")
});
