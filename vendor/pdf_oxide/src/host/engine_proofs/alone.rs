//! A proof that allocates tens of MiB runs in a process of its own.
//!
//! The allocator counter of `filter_readers` is the process's, and the proofs
//! that read it (`mem_fixture`, `page_operators`, `filter_readers`) fail when
//! another test thread allocates a few MiB beside them. A proof of ours that
//! builds a large fixture is therefore an `#[ignore]`d test whose parent test
//! runs this binary again on that one test, single-threaded, and waits.

use std::process::Command;

/// Runs the `#[ignore]`d test `path` of this binary alone in a child process
/// and fails with its output when it fails.
pub(super) fn run_alone(path: &str) {
    let out = Command::new(std::env::current_exe().expect("this test binary"))
        .args([path, "--exact", "--ignored", "--test-threads=1", "--nocapture"])
        .output()
        .expect("the test binary runs again");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    print!("{stdout}");
    assert!(out.status.success(), "{path} failed alone:\n{stdout}\n{stderr}");
    assert!(stdout.contains("1 passed"), "{path} did not run alone:\n{stdout}");
}
