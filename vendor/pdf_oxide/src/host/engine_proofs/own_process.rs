//! Runs a proof that reads the allocator counter in a process of its own.
//!
//! The counter in `filter_readers` is the process's: it counts every thread, so
//! a proof that reads it and every test allocating beside it spoil each
//! other's reading. A proof that holds tens of mebibytes, or that asserts on
//! growth, runs here instead: the test starts this binary again for itself
//! alone, on one thread, and passes when that run does.

use std::process::Command;

const CHILD: &str = "PDF_OXIDE_PROOF_IN_OWN_PROCESS";

/// Runs `body` in a process of its own: the test `name` of module `module`
/// (`module_path!()`), re-run by this binary with nothing beside it. Inside
/// that process, it runs `body` itself.
pub(super) fn alone(module: &str, name: &str, body: impl FnOnce()) {
    if std::env::var_os(CHILD).is_some() {
        return body();
    }
    let path = format!("{}::{name}", module.split_once("::").map_or(module, |(_, rest)| rest));
    let output = Command::new(std::env::current_exe().expect("the test binary"))
        .args([path.as_str(), "--exact", "--test-threads=1", "--nocapture"])
        .env(CHILD, "1")
        .output()
        .expect("the test binary starts");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // A name that matches no test runs none and exits cleanly: that is a failure.
    assert!(
        output.status.success() && report.contains("1 passed"),
        "{path} in a process of its own:\n{report}"
    );
    eprintln!("{path} in a process of its own:\n{report}");
}
