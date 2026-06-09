//! Fixed-size thread pool for PDF engine operations.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Each task runs in its own bumpalo arena. Drop the arena → all engine
//! memory for that operation freed in one shot. Pool size adapts to
//! hardware: `max(2, available_parallelism / 2)`. Bounded channel
//! provides backpressure. Shutdown drops the sender — workers exit.

use bumpalo::Bump;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

/// A task submitted to the pool.
pub struct Task {
    work: Box<dyn FnOnce(&Bump) + Send>,
    /// Shared cancellation flag checked before execution.
    pub cancel: Arc<AtomicBool>,
}

impl std::fmt::Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Task")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

impl Task {
    /// Create a new task with the given work closure and cancellation flag.
    pub fn new(
        work: impl FnOnce(&Bump) + Send + 'static,
        cancel: Arc<AtomicBool>,
    ) -> Self {
        Self { work: Box::new(work), cancel }
    }

    /// Check whether this task has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// Fixed-size thread pool with bounded task queue.
pub struct ThreadPool {
    sender: Option<Sender<Task>>,
    workers: Vec<JoinHandle<()>>,
}

impl ThreadPool {
    /// Create with default size: `max(2, available_parallelism / 2)`.
    pub fn new() -> Self {
        Self::with_capacity(Self::default_size(), 64)
    }

    /// Create a pool with a specific thread count and bounded queue depth.
    pub fn with_capacity(size: usize, queue_depth: usize) -> Self {
        let (sender, receiver) = bounded::<Task>(queue_depth);
        let mut workers = Vec::with_capacity(size);

        for i in 0..size {
            let rx = receiver.clone();
            let handle = thread::Builder::new()
                .name(format!("pdf-pool-{i}"))
                .spawn(move || worker_loop(rx))
                .expect("failed to spawn pool thread");
            workers.push(handle);
        }

        Self { sender: Some(sender), workers }
    }

    /// Submit a task. Blocks if queue is full (backpressure).
    pub fn submit(&self, task: Task) -> Result<(), Task> {
        match &self.sender {
            Some(sender) => sender.send(task).map_err(|e| e.0),
            None => Err(task),
        }
    }

    /// Drop the sender so worker threads will exit after draining.
    pub fn shutdown(&mut self) {
        self.sender = None;
    }

    /// Join all worker threads, blocking until they finish.
    pub fn join(&mut self) {
        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
    }

    /// Return the number of worker threads.
    pub fn size(&self) -> usize {
        self.workers.len()
    }

    fn default_size() -> usize {
        thread::available_parallelism()
            .map(|n| n.get() / 2)
            .unwrap_or(2)
            .max(2)
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.shutdown();
        self.join();
    }
}

fn worker_loop(receiver: Receiver<Task>) {
    loop {
        match receiver.recv() {
            Ok(task) => {
                if task.is_cancelled() {
                    continue;
                }
                let arena = Bump::new();
                (task.work)(&arena);
                drop(arena);
            }
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn pool_runs_tasks() {
        let pool = ThreadPool::with_capacity(2, 8);
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..10 {
            let c = counter.clone();
            let cancel = Arc::new(AtomicBool::new(false));
            pool.submit(Task::new(
                move |_arena| { c.fetch_add(1, Ordering::Relaxed); },
                cancel,
            )).unwrap();
        }

        drop(pool);
        assert_eq!(counter.load(Ordering::Relaxed), 10);
    }

    #[test]
    fn cancelled_tasks_are_skipped() {
        let pool = ThreadPool::with_capacity(1, 8);
        let counter = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicBool::new(true));

        let c = counter.clone();
        pool.submit(Task::new(
            move |_arena| { c.fetch_add(1, Ordering::Relaxed); },
            cancel,
        )).unwrap();

        drop(pool);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn pool_adapts_to_hardware() {
        let pool = ThreadPool::new();
        assert!(pool.size() >= 2);
    }

    #[test]
    fn arena_dropped_after_task() {
        let pool = ThreadPool::with_capacity(1, 8);
        let cancel = Arc::new(AtomicBool::new(false));

        pool.submit(Task::new(
            move |arena| {
                let _data = arena.alloc_slice_fill_default::<u8>(1024 * 1024);
            },
            cancel,
        )).unwrap();

        drop(pool);
    }
}
