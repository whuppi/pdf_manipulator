//! Executor table — key registry, global thread budget, and the FFI
//! surface the Dart Router drives.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! ## Keys, never pointers
//!
//! Dart holds executor KEYS — opaque u64s looked up in a locked table —
//! never raw pointers. A call racing a terminate finds nothing and returns
//! a well-defined "executor disposed" result instead of touching freed
//! memory. Keys are never reused (monotonic counter), so a stale key
//! can never alias a newer executor.
//!
//! ## The no-exhaustion budget
//!
//! At most `MAX_EXECUTORS_GLOBAL` executor threads exist process-wide, across
//! ALL instances. Hitting the cap never errors and never spins:
//! the executor is created immediately (its mailbox accepts and buffers
//! jobs), but its THREAD starts later — the boot request waits in a
//! FIFO, and a dying executor's last act is handing its budget slot to
//! the next waiter. Executors terminated while still waiting are skipped.
//!
//! ## Terminate semantics
//!
//! `terminate` is instant and idempotent: set the executor's cancel flag, wake
//! every parked I/O channel (lock-ordered — see `executor.rs`), drop the
//! mailbox sender, remove from the table. It never joins the thread.
//! After `terminate` returns, no notify callback will ever fire for this
//! executor, and the Dart Router owns the completion of its pending jobs.

use crate::host::binary_codec::ResponseWriter;
use crate::host::native::executor::{
    executor_main, post_result, Job, ExecutorController, Mail, SinkChannel, SourceChannel,
    ENGINE_DEAD_MESSAGE,
};
use crossbeam_channel::{unbounded, Receiver};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

/// Hard cap on executor threads across the whole process. Far above what
/// any sane workload runs concurrently, far below any OS thread limit.
pub const MAX_EXECUTORS_GLOBAL: usize = 128;

// ── Global state: the table and the budget ─────────────────────────

/// Monotonic key source. Starts at 1 (0 reads as "no executor" on the
/// wire). Never reused — ABA-proof by construction.
static NEXT_EXECUTOR_KEY: AtomicU64 = AtomicU64::new(1);

fn table() -> &'static Mutex<HashMap<u64, Arc<ExecutorController>>> {
    static TABLE: OnceLock<Mutex<HashMap<u64, Arc<ExecutorController>>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// An executor whose thread hasn't started yet (budget was full at boot).
struct PendingStart {
    key: u64,
    controller: Arc<ExecutorController>,
    mailbox: Receiver<Mail>,
}

struct Budget {
    /// Most executor threads started at once.
    limit: usize,
    /// Lane threads currently started (running or draining).
    live: usize,
    /// Lanes waiting for a thread slot, strict FIFO.
    waiters: VecDeque<PendingStart>,
}

fn budget() -> &'static Mutex<Budget> {
    static BUDGET: OnceLock<Mutex<Budget>> = OnceLock::new();
    BUDGET.get_or_init(|| Mutex::new(Budget::new(MAX_EXECUTORS_GLOBAL)))
}

impl Budget {
    fn new(limit: usize) -> Self {
        Budget { limit, live: 0, waiters: VecDeque::new() }
    }
}

/// Answers every job queued for an executor that never started with a
/// cancelled post — EVERY accepted job posts exactly once, started executor
/// or not. That post is the host's memory-safe signal to free the job's
/// buffers.
fn cancel_pending(p: PendingStart) {
    while let Ok(mail) = p.mailbox.try_recv() {
        let port = match mail {
            Mail::Job(job) => job.result_port,
            Mail::Reset { result_port } => result_port,
        };
        post_result(port, ResponseWriter::cancelled());
    }
    p.controller.cancel_flags.lock().unwrap().clear();
}

/// Returns the budget slot on thread exit — even on unwind, so a
/// panicking executor can never leak its slot.
struct SlotGuard(&'static Mutex<Budget>);

impl Drop for SlotGuard {
    fn drop(&mut self) {
        let mut b = self.0.lock().unwrap();
        loop {
            match b.waiters.pop_front() {
                // Terminated while waiting and not yet taken off the queue
                // by `terminate_in`: answer its jobs, then hand the slot to
                // the next waiter.
                Some(p) if p.controller.cancel.load(Ordering::SeqCst) => {
                    cancel_pending(p);
                    continue;
                }
                // Transfer the slot: `live` stays unchanged.
                Some(p) => {
                    start_thread(self.0, p.key, p.mailbox, p.controller);
                    return;
                }
                None => {
                    b.live -= 1;
                    return;
                }
            }
        }
    }
}

fn start_thread(
    budget: &'static Mutex<Budget>,
    key: u64,
    mailbox: Receiver<Mail>,
    controller: Arc<ExecutorController>,
) {
    thread::Builder::new()
        .name(format!("pdf-executor-{key}"))
        .spawn(move || {
            let _slot = SlotGuard(budget);
            executor_main(mailbox, controller);
        })
        .expect("failed to start executor thread (under MAX_EXECUTORS_GLOBAL — OS misconfigured?)");
}

// ── Public operations (called by the FFI surface and by tests) ─────

/// Boot an executor. Returns its key immediately; the thread starts now
/// (budget permitting) or when a slot frees (FIFO). Never blocks,
/// never errors.
pub fn boot() -> u64 {
    boot_in(budget())
}

fn boot_in(budget: &'static Mutex<Budget>) -> u64 {
    let (sender, receiver) = unbounded::<Mail>();
    let controller = Arc::new(ExecutorController::new(sender));

    let key = NEXT_EXECUTOR_KEY.fetch_add(1, Ordering::Relaxed);
    table().lock().unwrap().insert(key, controller.clone());

    let mut b = budget.lock().unwrap();
    if b.live < b.limit {
        b.live += 1;
        start_thread(budget, key, receiver, controller);
    } else {
        b.waiters.push_back(PendingStart { key, controller, mailbox: receiver });
    }

    key
}

/// Start a new lease on an executor and return its id, or 0 when the executor is
/// gone, has a job outstanding, or died. Ids count up from 1, per executor,
/// and are never reused; from this call on, a message under an older
/// lease is dropped.
pub fn begin_lease(executor_key: u64) -> u64 {
    let Some(controller) = table().lock().unwrap().get(&executor_key).cloned() else {
        return 0;
    };
    controller.begin_lease().unwrap_or(0)
}

/// Submit a job to an executor under `lease`. Never blocks (unbounded mailbox).
/// If the executor is gone, posts a "executor disposed" error to the result port;
/// under a lease that is not current, posts the stale-lease error and
/// touches nothing else — the caller always gets exactly one completion
/// signal per job.
pub fn submit(executor_key: u64, lease: u64, job: Job) {
    let Some(controller) = table().lock().unwrap().get(&executor_key).cloned() else {
        post_result(job.result_port, ResponseWriter::cancelled());
        return;
    };

    // Its cancel flag goes in BEFORE the enqueue so a cancel arriving right
    // after submit can always find its job.
    if let Err(refusal) = controller.admit_job(lease, &job) {
        post_result(job.result_port, ResponseWriter::error(refusal));
        return;
    }

    let sender = controller.mailbox.lock().unwrap().clone();
    match sender {
        Some(sender) => {
            let result_port = job.result_port;
            let job_id = job.job_id;
            if sender.send(Mail::Job(job)).is_err() {
                // Receiver gone: the executor was terminated while waiting for a
                // budget slot and its mailbox was discarded.
                controller.cancel_flags.lock().unwrap().remove(&job_id);
                post_result(result_port, ResponseWriter::cancelled());
            }
        }
        None => {
            // Terminated, or stopped after a panic, between the table lookup
            // and here.
            controller.cancel_flags.lock().unwrap().remove(&job.job_id);
            let bytes = if controller.dead.load(Ordering::SeqCst) {
                ResponseWriter::error(ENGINE_DEAD_MESSAGE)
            } else {
                ResponseWriter::cancelled()
            };
            post_result(job.result_port, bytes);
        }
    }
}

/// Cancel one job. Instant, idempotent. Queued → it will be skipped
/// at dequeue, never reaches the engine, and this returns true so the
/// host can complete it at once. Running → its I/O wakes with
/// FLAG_CANCELLED and the op unwinds at the next I/O boundary. The executor
/// and every other job on it are untouched. A cancel under a lease that
/// is not current changes nothing.
pub fn cancel_job(executor_key: u64, lease: u64, job_id: u64) -> bool {
    let Some(controller) = table().lock().unwrap().get(&executor_key).cloned() else {
        return false;
    };
    controller.cancel_job(lease, job_id)
}

/// Replace an executor's engine state with a fresh one and post the answer to
/// `result_port` exactly once: success once the thread has done it, or the
/// error that refused it (stale lease, a job outstanding, a dead executor).
/// The thread takes the reset between jobs, so a reset never interrupts
/// the engine.
pub fn reset(executor_key: u64, lease: u64, result_port: i64) {
    let Some(controller) = table().lock().unwrap().get(&executor_key).cloned() else {
        post_result(result_port, ResponseWriter::cancelled());
        return;
    };
    if let Err(refusal) = controller.request_reset(lease, result_port) {
        post_result(result_port, ResponseWriter::error(refusal));
    }
}

/// True once the executor's engine panicked and the executor stopped serving.
pub fn is_dead(executor_key: u64) -> bool {
    table()
        .lock()
        .unwrap()
        .get(&executor_key)
        .is_some_and(|controller| controller.dead.load(Ordering::SeqCst))
}

/// The executor's peak heap in bytes since its last reset, 0 for an executor that is
/// gone.
pub fn heap_peak(executor_key: u64) -> usize {
    table().lock().unwrap().get(&executor_key).map_or(0, |controller| controller.heap_peak())
}

/// Terminate an executor. Instant, idempotent, never joins. After this returns
/// no notify callback will ever fire for this executor (see the terminate
/// protocol in `executor.rs`); the thread drains and frees its state in
/// the background; the budget slot is handed on when it exits.
pub fn terminate(executor_key: u64) {
    terminate_in(budget(), executor_key);
}

fn terminate_in(budget: &Mutex<Budget>, executor_key: u64) {
    let Some(controller) = table().lock().unwrap().remove(&executor_key) else {
        return;
    };

    // Order is load-bearing:
    // 1. Flag first — anything that wakes or dequeues next sees it.
    controller.cancel.store(true, Ordering::SeqCst);
    // 2. Wake every parked I/O channel (lock-ordered: after this, no
    //    notify callback can ever fire again for this executor).
    controller.channels.cancel_all();
    // 3. Close the mailbox — the thread drains and exits.
    *controller.mailbox.lock().unwrap() = None;
    // 4. A lane still waiting for a slot has no thread to drain it: take it
    //    off the queue and answer its jobs now, not when some other executor
    //    exits.
    let waiting = {
        let mut b = budget.lock().unwrap();
        b.waiters.iter().position(|p| p.key == executor_key).and_then(|at| b.waiters.remove(at))
    };
    if let Some(p) = waiting {
        cancel_pending(p);
    }
}

/// Number of started executor threads (running or draining). Diagnostics
/// and tests only — never used for decisions.
pub fn live_executor_count() -> usize {
    budget().lock().unwrap().live
}

#[cfg(test)]
pub(crate) fn controller_for(executor_key: u64) -> Option<Arc<ExecutorController>> {
    table().lock().unwrap().get(&executor_key).cloned()
}

// ── FFI surface — what the Dart Router calls ───────────────────────
//
// Every function is a direct translation of one
// Router verb. No logic lives here.

/// Boot an executor. Returns its opaque key.
#[no_mangle]
pub extern "C" fn executor_boot() -> u64 {
    boot()
}

/// Start a new lease on an executor. Returns the lease id (from 1, never
/// reused on an executor), or 0 when the executor is gone, has a job outstanding or
/// a held channel registered, or died.
#[no_mangle]
pub extern "C" fn executor_lease_begin(executor_key: u64) -> u64 {
    begin_lease(executor_key)
}

/// Replace the executor's engine state with a fresh one. The answer is posted
/// to `result_port` exactly once: a success once the executor has reset, or an
/// error that refuses it (stale lease, a job outstanding, a dead executor).
#[no_mangle]
pub extern "C" fn executor_reset(executor_key: u64, lease: u64, result_port: i64) {
    reset(executor_key, lease, result_port);
}

/// Submit one job to an executor under `lease`. A lease that is not the executor's
/// current one makes the executor post the stale-lease error and nothing else.
///
/// Sources: parallel arrays of (buffer ptr, notify fn ptr, length,
/// keep flag), `source_count` entries each. A keep flag of 1 marks a
/// channel that outlives the job (handle-creating ops move the
/// source into the document) — it stays registered for terminate/cancel
/// wakes until `executor_hold_release`. Sinks: parallel arrays of (buffer
/// ptr, notify fn ptr), `sink_count` entries each. The result is
/// posted to `result_port` exactly once.
///
/// # Safety
/// `request_ptr` must point to `request_len` readable bytes; the
/// array pointers must each point to `source_count`/`sink_count`
/// readable elements. Buffers must stay alive until the job's result
/// has been posted.
#[no_mangle]
pub unsafe extern "C" fn executor_submit(
    executor_key: u64,
    lease: u64,
    job_id: u64,
    request_ptr: *const u8,
    request_len: i32,
    source_count: i32,
    source_bufs: *const *mut u8,
    source_notifys: *const Option<unsafe extern "C" fn()>,
    source_lengths: *const i64,
    source_keeps: *const u8,
    sink_count: i32,
    sink_bufs: *const *mut u8,
    sink_notifys: *const Option<unsafe extern "C" fn()>,
    result_port: i64,
) {
    let request = std::slice::from_raw_parts(request_ptr, request_len as usize).to_vec();

    let sources = (0..source_count as usize)
        .map(|i| SourceChannel {
            buf: *source_bufs.add(i) as usize,
            notify: *source_notifys.add(i),
            length: *source_lengths.add(i),
            keep: *source_keeps.add(i) != 0,
        })
        .collect();

    let sinks = (0..sink_count as usize)
        .map(|i| SinkChannel {
            buf: *sink_bufs.add(i) as usize,
            notify: *sink_notifys.add(i),
        })
        .collect();

    submit(
        executor_key,
        lease,
        Job {
            job_id,
            request,
            sources,
            sinks,
            result_port,
            cancel: Arc::new(AtomicBool::new(false)),
        },
    );
}

/// Cancel one job on an executor. Returns 1 when the job had not started and
/// the executor will skip it, 0 otherwise, and always 0, changing nothing,
/// under a lease that is not current.
#[no_mangle]
pub extern "C" fn executor_job_cancel(executor_key: u64, lease: u64, job_id: u64) -> u8 {
    cancel_job(executor_key, lease, job_id) as u8
}

/// 1 when the executor's engine panicked and the executor stopped serving, else 0.
#[no_mangle]
pub extern "C" fn executor_is_dead(executor_key: u64) -> u8 {
    is_dead(executor_key) as u8
}

/// The most bytes the executor's thread has held since its last reset, as of the
/// last job or reset it answered; 0 for an executor that is gone.
#[no_mangle]
pub extern "C" fn executor_heap_peak(executor_key: u64) -> u64 {
    heap_peak(executor_key) as u64
}

/// Terminate an executor and everything on it. Instant; never blocks.
#[no_mangle]
pub extern "C" fn executor_terminate(executor_key: u64) {
    terminate(executor_key);
}

/// Release one held channel (its handle was disposed). The caller
/// must free the buffer only AFTER this returns. No-op if the executor
/// is already gone (terminate covers its channels) or the lease is not current
/// (a lease changes hands only once no held channel is registered, so a
/// stale lease owes no release).
#[no_mangle]
pub extern "C" fn executor_hold_release(executor_key: u64, lease: u64, buf: *mut u8) {
    if let Some(controller) = table().lock().unwrap().get(&executor_key).cloned() {
        if controller.is_current(lease) {
            controller.channels.release_one(buf as usize);
        }
    }
}

/// Started executor threads, process-wide. Diagnostics only.
#[no_mangle]
pub extern "C" fn executor_live_count() -> u32 {
    live_executor_count() as u32
}

// ── Channel buffer FFI helpers (Dart allocates, Rust syncs) ────────

use crate::host::native::shared_buffer as sb;

/// Required size of a read-channel buffer, in bytes.
#[no_mangle]
pub extern "C" fn channel_read_buffer_size() -> i32 {
    sb::read_channel::TOTAL_SIZE as i32
}

/// Required size of a write-channel buffer, in bytes.
#[no_mangle]
pub extern "C" fn channel_write_buffer_size() -> i32 {
    sb::write_channel::TOTAL_SIZE as i32
}

/// Initialize the sync pair inside a read-channel buffer.
/// # Safety
/// `buf` must point to `channel_read_buffer_size()` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn channel_init_read(buf: *mut u8) {
    sb::init_sync(buf, sb::read_channel::OFFSET_SYNC_PTR);
}

/// Destroy the sync pair inside a read-channel buffer.
/// # Safety
/// `buf` must have been initialized via `channel_init_read`.
#[no_mangle]
pub unsafe extern "C" fn channel_destroy_read(buf: *mut u8) {
    sb::destroy_sync(buf, sb::read_channel::OFFSET_SYNC_PTR);
}

/// Signal the read-channel condvar (host filled a request).
/// # Safety
/// `buf` must have been initialized via `channel_init_read`.
#[no_mangle]
pub unsafe extern "C" fn channel_signal_read(buf: *mut u8) {
    sb::notify(buf, sb::read_channel::OFFSET_SYNC_PTR);
}

/// Initialize the sync pair inside a write-channel buffer.
/// # Safety
/// `buf` must point to `channel_write_buffer_size()` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn channel_init_write(buf: *mut u8) {
    sb::init_sync(buf, sb::write_channel::OFFSET_SYNC_PTR);
}

/// Destroy the sync pair inside a write-channel buffer.
/// # Safety
/// `buf` must have been initialized via `channel_init_write`.
#[no_mangle]
pub unsafe extern "C" fn channel_destroy_write(buf: *mut u8) {
    sb::destroy_sync(buf, sb::write_channel::OFFSET_SYNC_PTR);
}

/// Signal the write-channel condvar (host acknowledged a chunk).
/// # Safety
/// `buf` must have been initialized via `channel_init_write`.
#[no_mangle]
pub unsafe extern "C" fn channel_signal_write(buf: *mut u8) {
    sb::notify(buf, sb::write_channel::OFFSET_SYNC_PTR);
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Tests share process-global state (table + budget), so they
    /// poll for convergence instead of asserting instantaneous
    /// values that another test could perturb.
    fn wait_until(deadline: Duration, cond: impl Fn() -> bool) -> bool {
        let end = Instant::now() + deadline;
        while Instant::now() < end {
            if cond() {
                return true;
            }
            thread::sleep(Duration::from_millis(2));
        }
        cond()
    }

    /// Minimal parseable request: op "open" with zero fields. With a
    /// parked source channel this makes the executor block on its first
    /// read — the canonical "stuck cook" every terminate test needs.
    fn open_request() -> Vec<u8> {
        let mut bytes = vec![4u8];
        bytes.extend_from_slice(b"open");
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes
    }

    unsafe extern "C" fn noop_notify() {}

    /// A fresh executor and the first lease on it.
    fn spawn_leased() -> (u64, u64) {
        let key = boot();
        let lease = begin_lease(key);
        assert_ne!(lease, 0, "a fresh lane could not be leased");
        (key, lease)
    }

    /// A live read-channel buffer whose host never answers — any
    /// reader on it parks forever until cancelled.
    struct ParkedSource {
        buf: Vec<u8>,
    }

    impl ParkedSource {
        fn new() -> Self {
            let mut buf = vec![0u8; sb::read_channel::TOTAL_SIZE];
            unsafe { sb::init_sync(buf.as_mut_ptr(), sb::read_channel::OFFSET_SYNC_PTR) };
            Self { buf }
        }

        fn channel(&mut self) -> SourceChannel {
            SourceChannel {
                buf: self.buf.as_mut_ptr() as usize,
                notify: Some(noop_notify),
                length: 1024,
                keep: false,
            }
        }
    }

    impl Drop for ParkedSource {
        fn drop(&mut self) {
            unsafe { sb::destroy_sync(self.buf.as_mut_ptr(), sb::read_channel::OFFSET_SYNC_PTR) };
        }
    }

    fn parked_job(job_id: u64, source: &mut ParkedSource) -> Job {
        Job {
            job_id,
            request: open_request(),
            sources: vec![source.channel()],
            sinks: vec![],
            result_port: 0, // no Dart VM in tests — posts are no-ops
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    fn tickets_empty(key_controller: &Arc<ExecutorController>) -> bool {
        key_controller.cancel_flags.lock().unwrap().is_empty()
    }

    /// Wait until a submitted job's I/O channels are registered —
    /// i.e. the executor thread has genuinely started and is running the
    /// job. Needed because under a saturated global budget (e.g. the
    /// budget test running in parallel) a fresh executor may legitimately
    /// wait for a thread slot.
    fn wait_running(controller: &Arc<ExecutorController>) -> bool {
        wait_until(Duration::from_secs(20), || controller.channels.live_count() > 0)
    }

    fn kept_job(job_id: u64, source: &mut ParkedSource) -> Job {
        let mut channel = source.channel();
        channel.keep = true;
        Job {
            job_id,
            request: open_request(),
            sources: vec![channel],
            sinks: vec![],
            result_port: 0,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Run a job that keeps its source's channel, cancel it while it is
    /// parked on that channel, and wait until the channel is parked as a
    /// held one, as a document's source is after its open.
    fn leave_held_channel(
        key: u64,
        lease: u64,
        controller: &Arc<ExecutorController>,
        job_id: u64,
        source: &mut ParkedSource,
    ) {
        submit(key, lease, kept_job(job_id, source));
        assert!(wait_running(controller), "lane never started the job");
        cancel_job(key, lease, job_id);
        assert!(wait_until(Duration::from_secs(5), || {
            tickets_empty(controller) && controller.channels.live_count() == 1
        }));
    }

    fn quiet_job(job_id: u64) -> Job {
        Job {
            job_id,
            request: open_request(),
            sources: vec![],
            sinks: vec![],
            result_port: 0,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn a_stale_submit_and_a_stale_cancel_never_reach_the_current_lease() {
        let (key, stale) = spawn_leased();
        let controller = controller_for(key).unwrap();
        let current = begin_lease(key);
        assert!(current > stale);

        // The stale submit files no cancel flag and runs nothing: its job would
        // park the executor, and the channel registry stays empty.
        let mut source = ParkedSource::new();
        submit(key, stale, parked_job(1, &mut source));
        assert!(tickets_empty(&controller));
        assert_eq!(controller.channels.live_count(), 0);

        // Both leases count job ids from 1, so the stale cancel names the
        // current lease's job. It must not flag it.
        let mut running = ParkedSource::new();
        submit(key, current, parked_job(1, &mut running));
        assert!(wait_running(&controller), "lane never started the job");
        assert!(!cancel_job(key, stale, 1));
        let flagged = controller.cancel_flags.lock().unwrap()[&1].load(Ordering::SeqCst);
        assert!(!flagged, "a stale cancel flagged the current lease's job");

        cancel_job(key, current, 1);
        assert!(wait_until(Duration::from_secs(5), || tickets_empty(&controller)));
        terminate(key);
    }

    #[test]
    fn the_mailbox_buffers_a_second_job_but_the_lane_never_runs_two() {
        let (key, lease) = spawn_leased();
        let controller = controller_for(key).unwrap();
        let mut first = ParkedSource::new();
        let mut second = ParkedSource::new();

        submit(key, lease, parked_job(1, &mut first));
        assert!(wait_running(&controller), "lane never started job 1");
        submit(key, lease, parked_job(2, &mut second));

        // The mailbox accepted job 2 while job 1 runs...
        assert_eq!(controller.cancel_flags.lock().unwrap().len(), 2);
        // ...and the thread has not touched it: only job 1's channel is
        // registered, however long job 2 waits.
        for _ in 0..25 {
            assert_eq!(controller.channels.live_count(), 1);
            assert_eq!(*controller.running.lock().unwrap(), Some(1));
            thread::sleep(Duration::from_millis(2));
        }

        // Job 2 starts when job 1 has posted, not before.
        cancel_job(key, lease, 1);
        assert!(wait_until(Duration::from_secs(5), || {
            *controller.running.lock().unwrap() == Some(2) && controller.channels.live_count() == 1
        }));
        assert_eq!(controller.cancel_flags.lock().unwrap().len(), 1);

        cancel_job(key, lease, 2);
        assert!(wait_until(Duration::from_secs(5), || tickets_empty(&controller)));
        terminate(key);
    }

    #[test]
    fn a_reset_drops_the_held_channels_and_an_idle_lane_resets_again() {
        let (key, lease) = spawn_leased();
        let controller = controller_for(key).unwrap();
        let mut source = ParkedSource::new();

        leave_held_channel(key, lease, &controller, 1, &mut source);
        assert_eq!(begin_lease(key), 0, "a lease does not change hands over a held channel");

        for _ in 0..2 {
            reset(key, lease, 0);
            assert!(
                wait_until(Duration::from_secs(5), || controller.channels.live_count() == 0),
                "the reset left a held channel registered"
            );
        }
        assert!(!is_dead(key));
        assert!(begin_lease(key) > lease, "a reset lane takes the next lease");
        terminate(key);
    }

    #[test]
    fn a_reset_while_a_job_runs_is_refused_and_never_queued() {
        let (key, lease) = spawn_leased();
        let controller = controller_for(key).unwrap();
        let mut source = ParkedSource::new();

        submit(key, lease, kept_job(1, &mut source));
        assert!(wait_running(&controller), "lane never started the job");
        reset(key, lease, 0);
        assert_eq!(*controller.running.lock().unwrap(), Some(1));

        // The job ends and parks its kept channel as a held one. A reset
        // wrongly queued behind it would run before the quiet job below
        // finishes, and drop that channel.
        cancel_job(key, lease, 1);
        assert!(wait_until(Duration::from_secs(5), || tickets_empty(&controller)));
        submit(key, lease, quiet_job(2));
        assert!(wait_until(Duration::from_secs(5), || tickets_empty(&controller)));
        assert_eq!(controller.channels.live_count(), 1, "the refused reset ran after all");
        terminate(key);
    }

    #[test]
    fn a_stale_reset_is_refused_and_changes_nothing() {
        let (key, stale) = spawn_leased();
        let controller = controller_for(key).unwrap();
        let mut source = ParkedSource::new();
        leave_held_channel(key, stale, &controller, 1, &mut source);

        // A new lease cannot start over the held channel, so the held one
        // is dropped first; a stale reset after that must not drop what the
        // next tenant holds.
        reset(key, stale, 0);
        assert!(wait_until(Duration::from_secs(5), || controller.channels.live_count() == 0));
        let current = begin_lease(key);
        let mut next = ParkedSource::new();
        leave_held_channel(key, current, &controller, 1, &mut next);
        reset(key, stale, 0);
        submit(key, current, quiet_job(2));
        assert!(wait_until(Duration::from_secs(5), || tickets_empty(&controller)));
        assert_eq!(controller.channels.live_count(), 1, "a stale reset dropped the current lease's channel");
        terminate(key);
    }

    #[test]
    fn spawn_and_kill_lane_with_no_jobs() {
        let key = boot();
        assert!(controller_for(key).is_some());
        terminate(key);
        assert!(controller_for(key).is_none());
        // Idempotent.
        terminate(key);
    }

    #[test]
    fn kill_wakes_a_parked_job_instantly() {
        // THE test this architecture exists for: a job parked on I/O
        // that no host will ever answer must die promptly on terminate —
        // not after a timeout, not never.
        let mut source = ParkedSource::new();
        let (key, lease) = spawn_leased();
        let controller = controller_for(key).unwrap();

        submit(key, lease, parked_job(1, &mut source));

        // The job is genuinely running and parked on its I/O channel.
        assert!(wait_running(&controller), "lane never started the job");
        assert!(!tickets_empty(&controller));

        let killed_at = Instant::now();
        terminate(key);
        assert!(
            wait_until(Duration::from_secs(5), || tickets_empty(&controller)),
            "parked job did not unwind after terminate"
        );
        // Promptly — far inside the 30s heartbeat, proving the wake
        // came from the terminate, not the safety-net timer.
        assert!(killed_at.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn job_cancel_kills_one_job_and_lane_survives() {
        let mut source = ParkedSource::new();
        let (key, lease) = spawn_leased();
        let controller = controller_for(key).unwrap();

        submit(key, lease, parked_job(7, &mut source));
        assert!(wait_running(&controller), "lane never started the job");

        cancel_job(key, lease, 7);
        assert!(
            wait_until(Duration::from_secs(5), || tickets_empty(&controller)),
            "cancelled job did not unwind"
        );

        // Lane is alive and accepts further work.
        assert!(controller_for(key).is_some());
        let mut source2 = ParkedSource::new();
        submit(key, lease, parked_job(8, &mut source2));
        assert!(wait_running(&controller), "lane did not run the second job");
        cancel_job(key, lease, 8);
        assert!(wait_until(Duration::from_secs(5), || tickets_empty(&controller)));

        terminate(key);
    }

    #[test]
    fn queued_jobs_complete_as_cancelled_on_kill() {
        let mut parked = ParkedSource::new();
        let (key, lease) = spawn_leased();
        let controller = controller_for(key).unwrap();

        // Job 1 parks the executor; jobs 2 and 3 queue behind it.
        submit(key, lease, parked_job(1, &mut parked));
        assert!(wait_running(&controller), "lane never started job 1");
        submit(
            key,
            lease,
            Job {
                job_id: 2,
                request: open_request(),
                sources: vec![],
                sinks: vec![],
                result_port: 0,
                cancel: Arc::new(AtomicBool::new(false)),
            },
        );
        submit(
            key,
            lease,
            Job {
                job_id: 3,
                request: open_request(),
                sources: vec![],
                sinks: vec![],
                result_port: 0,
                cancel: Arc::new(AtomicBool::new(false)),
            },
        );

        terminate(key);
        // All three cancel flags clear: job 1 via the I/O wake, jobs 2+3
        // via the dequeue-time executor-cancel check.
        assert!(
            wait_until(Duration::from_secs(5), || tickets_empty(&controller)),
            "queued jobs were not drained after terminate"
        );
    }

    #[test]
    fn submit_to_dead_lane_is_safe() {
        let (key, lease) = spawn_leased();
        terminate(key);

        let mut source = ParkedSource::new();
        // Must not panic, must not leave a cancel flag anywhere.
        submit(key, lease, parked_job(42, &mut source));
    }

    #[test]
    fn churn_spawn_kill_releases_all_threads() {
        // Rapid create/dispose cycles must never exhaust anything.
        let before = live_executor_count();
        for _ in 0..500 {
            let key = boot();
            terminate(key);
        }
        assert!(
            wait_until(Duration::from_secs(10), || live_executor_count() <= before),
            "lane threads leaked after churn: live={}",
            live_executor_count()
        );
    }

    #[test]
    fn budget_cap_holds_and_waiters_start_fifo() {
        // Fill the budget past the cap; waiting executors must still
        // accept jobs and must start once slots free up.
        let keys: Vec<u64> = (0..MAX_EXECUTORS_GLOBAL + 8).map(|_| boot()).collect();
        assert!(live_executor_count() <= MAX_EXECUTORS_GLOBAL);

        // An executor beyond the cap accepts a job (queued, not running).
        let last = *keys.last().unwrap();
        let controller = controller_for(last).unwrap();
        let lease = begin_lease(last);
        assert_ne!(lease, 0, "a lane that has not started can still be leased");
        submit(
            last,
            lease,
            Job {
                job_id: 1,
                request: open_request(),
                sources: vec![],
                sinks: vec![],
                result_port: 0,
                cancel: Arc::new(AtomicBool::new(false)),
            },
        );

        // Free slots; the waiter must start and run its job. (The
        // no-source "open" job completes immediately with an error
        // result — completion == cancel flag removal.)
        for &key in &keys[..MAX_EXECUTORS_GLOBAL] {
            terminate(key);
        }
        assert!(
            wait_until(Duration::from_secs(10), || tickets_empty(&controller)),
            "waiting lane never started after slots freed"
        );

        for &key in &keys {
            terminate(key);
        }
        assert!(wait_until(Duration::from_secs(10), || {
            controller_for(last).is_none()
        }));
    }

    #[test]
    fn lane_killed_while_waiting_for_budget_is_skipped() {
        let keys: Vec<u64> = (0..MAX_EXECUTORS_GLOBAL + 4).map(|_| boot()).collect();

        // Terminate the over-cap waiters before they ever start.
        for &key in &keys[MAX_EXECUTORS_GLOBAL..] {
            terminate(key);
        }
        // Then everything else.
        for &key in &keys[..MAX_EXECUTORS_GLOBAL] {
            terminate(key);
        }

        // No thread may remain for the skipped waiters (or anything
        // else from this batch). Other tests run in parallel, so the
        // assertion only requires convergence, not an exact count.
        assert!(
            wait_until(Duration::from_secs(10), || {
                keys.iter().all(|k| controller_for(*k).is_none())
            }),
            "terminated executors still present in table"
        );
    }

    /// A budget of its own, so a test holds the slot and nothing else can
    /// free it.
    fn budget_of(limit: usize) -> &'static Mutex<Budget> {
        Box::leak(Box::new(Mutex::new(Budget::new(limit))))
    }

    #[test]
    fn a_lane_killed_while_waiting_for_a_slot_completes_its_jobs_at_once() {
        let slots = budget_of(1);
        let holder = boot_in(slots);
        assert!(
            wait_until(Duration::from_secs(5), || slots.lock().unwrap().live == 1),
            "the holder never took the slot"
        );

        let waiter = boot_in(slots);
        let controller = controller_for(waiter).unwrap();
        assert_eq!(slots.lock().unwrap().waiters.len(), 1, "the second lane waits");
        let lease = begin_lease(waiter);
        assert_ne!(lease, 0);
        submit(waiter, lease, quiet_job(1));
        assert!(!tickets_empty(&controller), "the job is queued behind the slot");

        terminate_in(slots, waiter);

        // The holder still owns the only slot: nothing but the terminate
        // can have answered the job.
        assert!(
            wait_until(Duration::from_secs(5), || tickets_empty(&controller)),
            "the job of a lane killed in the queue waited for a slot to free"
        );
        assert!(
            slots.lock().unwrap().waiters.is_empty(),
            "the killed lane stayed in the queue"
        );
        assert_eq!(slots.lock().unwrap().live, 1, "the holder kept its slot");

        terminate_in(slots, holder);
        assert!(wait_until(Duration::from_secs(5), || slots.lock().unwrap().live == 0));
    }

    /// The test-support op that panics the engine.
    #[cfg(feature = "test-support")]
    fn panic_request() -> Vec<u8> {
        let mut bytes = vec![9u8];
        bytes.extend_from_slice(b"testPanic");
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes
    }

    #[cfg(feature = "test-support")]
    fn panic_job(job_id: u64) -> Job {
        Job {
            job_id,
            request: panic_request(),
            sources: vec![],
            sinks: vec![],
            result_port: 0,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn cancelling_a_queued_job_skips_it_and_nothing_reaches_the_engine() {
        // The queued job would panic the engine if it ran, so an executor still
        // alive afterwards proves it never reached it.
        let mut parked = ParkedSource::new();
        let (key, lease) = spawn_leased();
        let controller = controller_for(key).unwrap();

        submit(key, lease, parked_job(1, &mut parked));
        assert!(wait_running(&controller), "lane never started job 1");
        submit(key, lease, panic_job(2));

        assert!(cancel_job(key, lease, 2), "job 2 had not started");
        assert!(!cancel_job(key, lease, 1), "job 1 is running");
        assert!(wait_until(Duration::from_secs(5), || tickets_empty(&controller)));
        assert!(!is_dead(key), "the cancelled job reached the engine");
        assert!(!cancel_job(key, lease, 2), "a finished job is no longer queued");

        terminate(key);
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn a_panic_ends_the_lane_and_every_job_behind_it_fails() {
        let mut parked = ParkedSource::new();
        let (key, lease) = spawn_leased();
        let controller = controller_for(key).unwrap();

        submit(key, lease, parked_job(1, &mut parked));
        assert!(wait_running(&controller), "lane never started job 1");
        submit(key, lease, panic_job(2));
        submit(key, lease, panic_job(3));
        submit(key, lease, panic_job(4));

        // Job 1 yields to its cancel; job 2 panics; 3 and 4 are failed
        // without running.
        assert!(!cancel_job(key, lease, 1));
        assert!(
            wait_until(Duration::from_secs(5), || is_dead(key) && tickets_empty(&controller)),
            "the lane did not stop after a panic"
        );

        // A job arriving after the panic is failed at the door.
        submit(key, lease, panic_job(5));
        assert!(tickets_empty(&controller));
        assert!(controller.mailbox.lock().unwrap().is_none());

        // A fresh executor serves as before.
        let other = boot();
        assert!(!is_dead(other));
        terminate(other);
        terminate(key);
    }
}
