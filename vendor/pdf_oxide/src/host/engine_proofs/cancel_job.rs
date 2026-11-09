//! The running job's cancel check, reachable without a parameter: a loop in
//! code that has no `CancelCheck` in scope polls it with `cancel::poll`.

use crate::cancel::{enter, poll, CancelCheck};
use crate::error::{Error, Result};

fn is_cancelled(error: &Error) -> bool {
    matches!(error, Error::Io(e) if e.to_string() == "operation cancelled")
}

fn nested() -> Result<()> {
    deeper()
}

fn deeper() -> Result<()> {
    poll()
}

/// No Dart battery sees it: whether a poll three calls below the entry sees
/// the check the job entered, at the exact poll it trips on, and sees nothing
/// on a thread or at a time no job entered, is a property of the engine's
/// thread-local that a cancel timed from Dart reaches only by chance.
#[test]
fn poll_in_a_nested_function_trips_at_the_nth_poll() {
    assert!(nested().is_ok(), "with nothing entered a poll is Ok");
    let check = CancelCheck::tripping_at(3);
    {
        let _job = enter(&check);
        assert!(nested().is_ok());
        assert!(nested().is_ok());
        let error = nested().expect_err("the third poll trips");
        assert!(is_cancelled(&error), "{error:?}");
        assert_eq!(check.polls(), 3);
        std::thread::spawn(|| assert!(nested().is_ok(), "another thread has no job"))
            .join()
            .expect("the other thread runs");
        assert_eq!(check.polls(), 3, "another thread's poll did not reach the check");
    }
    assert!(nested().is_ok(), "the guard removed the check");
    assert_eq!(check.polls(), 3);
}

/// No Dart battery sees it: a job that starts another under the same thread
/// (an op that runs a probe entry inside a request) must get its own check
/// for the inner run and the outer check back afterwards, and only polls
/// counted on two checks tell the two apart.
#[test]
fn guard_restores_the_previous_check() {
    let outer = CancelCheck::tripping_at(usize::MAX);
    let inner = CancelCheck::tripping_at(1);
    let _outer_job = enter(&outer);
    assert!(poll().is_ok());
    assert_eq!((outer.polls(), inner.polls()), (1, 0));
    {
        let _inner_job = enter(&inner);
        let error = poll().expect_err("the inner check trips at its first poll");
        assert!(is_cancelled(&error), "{error:?}");
        assert_eq!((outer.polls(), inner.polls()), (1, 1));
    }
    assert!(poll().is_ok(), "the outer check is back");
    assert_eq!((outer.polls(), inner.polls()), (2, 1));
}
