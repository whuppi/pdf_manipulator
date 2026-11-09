//! The allocator counts per thread, and adopt and release move bytes across one.

use crate::host::heap;
use std::sync::mpsc;
use std::thread;

const SIZE: usize = 1 << 20;

/// The channel's own blocks cross threads too, a few hundred bytes of them.
fn near(a: usize, b: usize) -> bool {
    a.abs_diff(b) <= 4096
}

/// A buffer one thread allocates and another frees leaves the first holding
/// it until it releases the bytes, and the second owing them until it adopts
/// them; after both, each thread's figure is back where it began. The peak
/// records the buffer while it lived, and `growth` reports the most a closure
/// held. No Dart battery can observe this: the counter is the engine's own and
/// no wire op reads it.
#[test]
fn bytes_that_cross_a_thread_are_moved_by_release_and_adopt() {
    let (to_freer, from_allocator) = mpsc::channel::<Vec<u8>>();
    let (freed_tx, freed_rx) = mpsc::channel::<()>();

    let freer = thread::spawn(move || {
        let before = heap::live();
        let buffer = from_allocator.recv().unwrap();
        heap::adopt(buffer.capacity());
        assert!(near(heap::live() - before, SIZE), "adopt counts the buffer");
        drop(buffer);
        assert!(near(heap::live(), before), "freeing the adopted buffer nets to nothing");
        freed_tx.send(()).unwrap();
    });

    let before = heap::live();
    heap::reset_peak();
    let buffer = vec![0u8; SIZE];
    assert!(near(heap::live() - before, SIZE));
    heap::release(buffer.capacity());
    assert!(near(heap::live(), before), "release hands the buffer over");
    to_freer.send(buffer).unwrap();
    freed_rx.recv().unwrap();
    freer.join().unwrap();
    assert!(near(heap::live(), before), "the other thread's free did not touch this thread");
    assert!(heap::peak() - before >= SIZE, "the peak recorded the buffer while it lived");

    let ((), grew) = heap::growth(|| drop(vec![1u8; SIZE]));
    assert!(grew >= SIZE && grew < SIZE + 4096, "growth was {grew}");
}
