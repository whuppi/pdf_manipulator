//! A response the executor posts leaves the executor's heap figure, whether Dart takes it or not.

use crate::host::heap;
use crate::host::native::executor::post_result;
use allo_isolate::ffi::DartCObject;
use std::sync::Mutex;
use std::thread;

const SIZE: usize = 1 << 20;

/// The channel, the Vec and its allocator rounding come to a few hundred bytes.
fn near(a: usize, b: usize) -> bool {
    a.abs_diff(b) <= 4096
}

type Finalizer = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void);

/// The finalizer, data and peer of the one message the fake Dart accepted.
static ACCEPTED: Mutex<Option<(Finalizer, usize, usize)>> = Mutex::new(None);

/// A `Dart_PostCObject` that takes the message and keeps it, as Dart does until
/// its garbage collector runs the finalizer.
unsafe extern "C" fn dart_accepts(_port: i64, message: *mut DartCObject) -> bool {
    let external = (*message).value.as_external_typed_data;
    *ACCEPTED.lock().unwrap() = Some((external.callback, external.data as usize, external.peer as usize));
    true
}

/// A response posted to a closed port is freed on the executor's thread inside the
/// post, and one Dart accepts is freed later by Dart's finalizer on another
/// thread; either way the executor's figure ends where it began, the first because
/// `post_result` counts the bytes again once the failed post has freed them, the
/// second because it stops counting them before the post (and the Vec's shrink to
/// its length and its box, which allo-isolate does on the posting thread, net to
/// nothing). No Dart battery can observe this: the counter is the engine's own.
#[test]
fn a_posted_response_leaves_the_executor_heap_where_it_was() {
    let response = || {
        let mut bytes = Vec::with_capacity(2 * SIZE);
        bytes.resize(SIZE, 1u8);
        bytes
    };

    let before = heap::live();
    post_result(1, response());
    assert!(near(heap::live(), before), "a post that failed left {} bytes", heap::live() as isize - before as isize);

    unsafe { allo_isolate::store_dart_post_cobject(dart_accepts) };
    let before = heap::live();
    post_result(1, response());
    assert!(near(heap::live(), before), "a post Dart took left {} bytes", heap::live() as isize - before as isize);

    let (finalizer, data, peer) = ACCEPTED.lock().unwrap().take().expect("the fake Dart took the message");
    thread::spawn(move || unsafe { finalizer(data as *mut _, peer as *mut _) }).join().unwrap();
    assert!(near(heap::live(), before), "Dart's free on its own thread moved this thread's figure");
}
