//! Lane table — key registry, global thread budget, and the FFI
//! surface the Dart Router drives.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! ## Keys, never pointers
//!
//! Dart holds lane KEYS — opaque u64s looked up in a locked table —
//! never raw pointers. A call racing a kill finds nothing and returns
//! a well-defined "lane disposed" result instead of touching freed
//! memory. Keys are never reused (monotonic counter), so a stale key
//! can never alias a newer lane.
//!
//! ## The no-exhaustion budget
//!
//! At most `MAX_LANES_GLOBAL` lane threads exist process-wide, across
//! ALL instances. Hitting the cap never errors and never spins:
//! the lane is created immediately (its mailbox accepts and buffers
//! jobs), but its THREAD starts later — the spawn request waits in a
//! FIFO, and a dying lane's last act is handing its budget slot to
//! the next waiter. Lanes killed while still waiting are skipped.
//!
//! ## Kill semantics
//!
//! `kill` is instant and idempotent: set the lane's cancel flag, wake
//! every parked I/O channel (lock-ordered — see `lane.rs`), drop the
//! mailbox sender, remove from the table. It never joins the thread.
//! After `kill` returns, no notify callback will ever fire for this
//! lane, and the Dart Router owns the completion of its pending jobs.

use crate::host::binary_codec::ResponseWriter;
use crate::host::native::lane::{
    lane_main, post_result, ChannelRegistry, Job, LaneController, SinkChannel, SourceChannel,
};
use crossbeam_channel::{unbounded, Receiver};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

/// Hard cap on lane threads across the whole process. Far above what
/// any sane workload runs concurrently, far below any OS thread limit.
pub const MAX_LANES_GLOBAL: usize = 128;

// ── Global state: the table and the budget ─────────────────────────

/// Monotonic key source. Starts at 1 (0 reads as "no lane" on the
/// wire). Never reused — ABA-proof by construction.
static NEXT_LANE_KEY: AtomicU64 = AtomicU64::new(1);

fn table() -> &'static Mutex<HashMap<u64, Arc<LaneController>>> {
    static TABLE: OnceLock<Mutex<HashMap<u64, Arc<LaneController>>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A lane whose thread hasn't started yet (budget was full at spawn).
struct PendingStart {
    key: u64,
    controller: Arc<LaneController>,
    mailbox: Receiver<Job>,
}

struct Budget {
    /// Lane threads currently started (running or draining).
    live: usize,
    /// Lanes waiting for a thread slot, strict FIFO.
    waiters: VecDeque<PendingStart>,
}

fn budget() -> &'static Mutex<Budget> {
    static BUDGET: OnceLock<Mutex<Budget>> = OnceLock::new();
    BUDGET.get_or_init(|| Mutex::new(Budget { live: 0, waiters: VecDeque::new() }))
}

/// Returns the budget slot on thread exit — even on unwind, so a
/// panicking lane can never leak its slot.
struct SlotGuard;

impl Drop for SlotGuard {
    fn drop(&mut self) {
        let mut b = budget().lock().unwrap();
        loop {
            match b.waiters.pop_front() {
                // Killed while waiting: drain its queued jobs with a
                // cancelled post each — EVERY accepted job posts
                // exactly once, started lane or not. That post is the
                // host's memory-safe signal to free the job's buffers.
                // Then hand the slot to the next waiter.
                Some(p) if p.controller.cancel.load(Ordering::SeqCst) => {
                    while let Ok(job) = p.mailbox.try_recv() {
                        post_result(
                            job.result_port,
                            ResponseWriter::cancelled(),
                        );
                    }
                    p.controller.tickets.lock().unwrap().clear();
                    continue;
                }
                // Transfer the slot: `live` stays unchanged.
                Some(p) => {
                    start_thread(p.key, p.mailbox, p.controller);
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

fn start_thread(key: u64, mailbox: Receiver<Job>, controller: Arc<LaneController>) {
    thread::Builder::new()
        .name(format!("pdf-lane-{key}"))
        .spawn(move || {
            let _slot = SlotGuard;
            lane_main(mailbox, controller);
        })
        .expect("failed to spawn lane thread (under MAX_LANES_GLOBAL — OS misconfigured?)");
}

// ── Public operations (called by the FFI surface and by tests) ─────

/// Create a lane. Returns its key immediately; the thread starts now
/// (budget permitting) or when a slot frees (FIFO). Never blocks,
/// never errors.
pub fn spawn() -> u64 {
    let (sender, receiver) = unbounded::<Job>();
    let controller = Arc::new(LaneController {
        mailbox: Mutex::new(Some(sender)),
        cancel: Arc::new(AtomicBool::new(false)),
        channels: ChannelRegistry::new(),
        tickets: Mutex::new(HashMap::new()),
    });

    let key = NEXT_LANE_KEY.fetch_add(1, Ordering::Relaxed);
    table().lock().unwrap().insert(key, controller.clone());

    let mut b = budget().lock().unwrap();
    if b.live < MAX_LANES_GLOBAL {
        b.live += 1;
        start_thread(key, receiver, controller);
    } else {
        b.waiters.push_back(PendingStart { key, controller, mailbox: receiver });
    }

    key
}

/// Submit a job to a lane. Never blocks (unbounded mailbox). If the
/// lane is gone, posts a "lane disposed" error to the result port —
/// the caller always gets exactly one completion signal per job.
pub fn submit(lane_key: u64, job: Job) {
    let Some(controller) = table().lock().unwrap().get(&lane_key).cloned() else {
        post_result(job.result_port, ResponseWriter::cancelled());
        return;
    };

    // Ticket goes in BEFORE the enqueue so a cancel arriving right
    // after submit can always find its job.
    controller
        .tickets
        .lock()
        .unwrap()
        .insert(job.job_id, job.cancel.clone());

    let sender = controller.mailbox.lock().unwrap().clone();
    match sender {
        Some(sender) => {
            let result_port = job.result_port;
            let job_id = job.job_id;
            if sender.send(job).is_err() {
                // Receiver gone: lane was killed while waiting for a
                // budget slot and its mailbox was discarded.
                controller.tickets.lock().unwrap().remove(&job_id);
                post_result(result_port, ResponseWriter::cancelled());
            }
        }
        None => {
            // Killed between the table lookup and here.
            controller.tickets.lock().unwrap().remove(&job.job_id);
            post_result(job.result_port, ResponseWriter::cancelled());
        }
    }
}

/// Cancel one job. Instant, idempotent. Queued → it will be skipped
/// at dequeue. Running → its I/O wakes with FLAG_CANCELLED and the
/// op unwinds at the next I/O boundary. The lane and every other job
/// on it are untouched.
pub fn cancel_job(lane_key: u64, job_id: u64) {
    let Some(controller) = table().lock().unwrap().get(&lane_key).cloned() else {
        return;
    };

    if let Some(flag) = controller.tickets.lock().unwrap().get(&job_id) {
        flag.store(true, Ordering::SeqCst);
    }
    // Wake the job's parked I/O (no-op if it has none registered).
    controller.channels.cancel_job(job_id);
}

/// Kill a lane. Instant, idempotent, never joins. After this returns
/// no notify callback will ever fire for this lane (see the kill
/// protocol in `lane.rs`); the thread drains and frees its state in
/// the background; the budget slot is handed on when it exits.
pub fn kill(lane_key: u64) {
    let Some(controller) = table().lock().unwrap().remove(&lane_key) else {
        return;
    };

    // Order is load-bearing:
    // 1. Flag first — anything that wakes or dequeues next sees it.
    controller.cancel.store(true, Ordering::SeqCst);
    // 2. Wake every parked I/O channel (lock-ordered: after this, no
    //    notify callback can ever fire again for this lane).
    controller.channels.cancel_all();
    // 3. Close the mailbox — the thread drains and exits.
    *controller.mailbox.lock().unwrap() = None;
}

/// Number of started lane threads (running or draining). Diagnostics
/// and tests only — never used for decisions.
pub fn live_lane_count() -> usize {
    budget().lock().unwrap().live
}

#[cfg(test)]
pub(crate) fn controller_for(lane_key: u64) -> Option<Arc<LaneController>> {
    table().lock().unwrap().get(&lane_key).cloned()
}

// ── FFI surface — what the Dart Router calls ───────────────────────
//
// Dumb by design: every function is a direct translation of one
// Router verb. No logic lives here.

/// Create a lane. Returns its opaque key.
#[no_mangle]
pub extern "C" fn lane_spawn() -> u64 {
    spawn()
}

/// Submit one job to a lane.
///
/// Sources: parallel arrays of (buffer ptr, notify fn ptr, length,
/// keep flag), `source_count` entries each. A keep flag of 1 marks a
/// channel that outlives the job (handle-creating ops move the
/// source into the document) — it stays registered for kill/cancel
/// wakes until `lane_channel_release`. Sinks: parallel arrays of (buffer
/// ptr, notify fn ptr), `sink_count` entries each. The result is
/// posted to `result_port` exactly once.
///
/// # Safety
/// `request_ptr` must point to `request_len` readable bytes; the
/// array pointers must each point to `source_count`/`sink_count`
/// readable elements. Buffers must stay alive until the job's result
/// has been posted.
#[no_mangle]
pub unsafe extern "C" fn lane_submit(
    lane_key: u64,
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
        lane_key,
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

/// Cancel one job on a lane.
#[no_mangle]
pub extern "C" fn lane_job_cancel(lane_key: u64, job_id: u64) {
    cancel_job(lane_key, job_id);
}

/// Kill a lane and everything on it. Instant; never blocks.
#[no_mangle]
pub extern "C" fn lane_kill(lane_key: u64) {
    kill(lane_key);
}

/// Release one held channel (its handle was disposed). The caller
/// must free the buffer only AFTER this returns. No-op if the lane
/// is already gone (kill covers its channels).
#[no_mangle]
pub extern "C" fn lane_channel_release(lane_key: u64, buf: *mut u8) {
    if let Some(controller) = table().lock().unwrap().get(&lane_key).cloned() {
        controller.channels.release_one(buf as usize);
    }
}

/// Started lane threads, process-wide. Diagnostics only.
#[no_mangle]
pub extern "C" fn lane_live_count() -> u32 {
    live_lane_count() as u32
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
    /// parked source channel this makes the lane block on its first
    /// read — the canonical "stuck cook" every kill test needs.
    fn open_request() -> Vec<u8> {
        let mut bytes = vec![4u8];
        bytes.extend_from_slice(b"open");
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes
    }

    unsafe extern "C" fn noop_notify() {}

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

    fn tickets_empty(key_controller: &Arc<LaneController>) -> bool {
        key_controller.tickets.lock().unwrap().is_empty()
    }

    /// Wait until a submitted job's I/O channels are registered —
    /// i.e. the lane thread has genuinely started and is running the
    /// job. Needed because under a saturated global budget (e.g. the
    /// budget test running in parallel) a fresh lane may legitimately
    /// wait for a thread slot.
    fn wait_running(controller: &Arc<LaneController>) -> bool {
        wait_until(Duration::from_secs(20), || controller.channels.live_count() > 0)
    }

    #[test]
    fn spawn_and_kill_lane_with_no_jobs() {
        let key = spawn();
        assert!(controller_for(key).is_some());
        kill(key);
        assert!(controller_for(key).is_none());
        // Idempotent.
        kill(key);
    }

    #[test]
    fn kill_wakes_a_parked_job_instantly() {
        // THE test this architecture exists for: a job parked on I/O
        // that no host will ever answer must die promptly on kill —
        // not after a timeout, not never.
        let mut source = ParkedSource::new();
        let key = spawn();
        let controller = controller_for(key).unwrap();

        submit(key, parked_job(1, &mut source));

        // The job is genuinely running and parked on its I/O channel.
        assert!(wait_running(&controller), "lane never started the job");
        assert!(!tickets_empty(&controller));

        let killed_at = Instant::now();
        kill(key);
        assert!(
            wait_until(Duration::from_secs(5), || tickets_empty(&controller)),
            "parked job did not unwind after kill"
        );
        // Promptly — far inside the 30s heartbeat, proving the wake
        // came from the kill, not the safety-net timer.
        assert!(killed_at.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn job_cancel_kills_one_job_and_lane_survives() {
        let mut source = ParkedSource::new();
        let key = spawn();
        let controller = controller_for(key).unwrap();

        submit(key, parked_job(7, &mut source));
        assert!(wait_running(&controller), "lane never started the job");

        cancel_job(key, 7);
        assert!(
            wait_until(Duration::from_secs(5), || tickets_empty(&controller)),
            "cancelled job did not unwind"
        );

        // Lane is alive and accepts further work.
        assert!(controller_for(key).is_some());
        let mut source2 = ParkedSource::new();
        submit(key, parked_job(8, &mut source2));
        assert!(wait_running(&controller), "lane did not run the second job");
        cancel_job(key, 8);
        assert!(wait_until(Duration::from_secs(5), || tickets_empty(&controller)));

        kill(key);
    }

    #[test]
    fn queued_jobs_complete_as_cancelled_on_kill() {
        let mut parked = ParkedSource::new();
        let key = spawn();
        let controller = controller_for(key).unwrap();

        // Job 1 parks the lane; jobs 2 and 3 queue behind it.
        submit(key, parked_job(1, &mut parked));
        assert!(wait_running(&controller), "lane never started job 1");
        submit(
            key,
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
            Job {
                job_id: 3,
                request: open_request(),
                sources: vec![],
                sinks: vec![],
                result_port: 0,
                cancel: Arc::new(AtomicBool::new(false)),
            },
        );

        kill(key);
        // All three tickets drain: job 1 via the I/O wake, jobs 2+3
        // via the dequeue-time lane-cancel check.
        assert!(
            wait_until(Duration::from_secs(5), || tickets_empty(&controller)),
            "queued jobs were not drained after kill"
        );
    }

    #[test]
    fn submit_to_dead_lane_is_safe() {
        let key = spawn();
        kill(key);

        let mut source = ParkedSource::new();
        // Must not panic, must not leave a ticket anywhere.
        submit(key, parked_job(42, &mut source));
    }

    #[test]
    fn churn_spawn_kill_releases_all_threads() {
        // Rapid create/dispose cycles must never exhaust anything.
        let before = live_lane_count();
        for _ in 0..500 {
            let key = spawn();
            kill(key);
        }
        assert!(
            wait_until(Duration::from_secs(10), || live_lane_count() <= before),
            "lane threads leaked after churn: live={}",
            live_lane_count()
        );
    }

    #[test]
    fn budget_cap_holds_and_waiters_start_fifo() {
        // Fill the budget past the cap; waiting lanes must still
        // accept jobs and must start once slots free up.
        let keys: Vec<u64> = (0..MAX_LANES_GLOBAL + 8).map(|_| spawn()).collect();
        assert!(live_lane_count() <= MAX_LANES_GLOBAL);

        // A lane beyond the cap accepts a job (queued, not running).
        let last = *keys.last().unwrap();
        let controller = controller_for(last).unwrap();
        submit(
            last,
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
        // result — completion == ticket removal.)
        for &key in &keys[..MAX_LANES_GLOBAL] {
            kill(key);
        }
        assert!(
            wait_until(Duration::from_secs(10), || tickets_empty(&controller)),
            "waiting lane never started after slots freed"
        );

        for &key in &keys {
            kill(key);
        }
        assert!(wait_until(Duration::from_secs(10), || {
            controller_for(last).is_none()
        }));
    }

    #[test]
    fn lane_killed_while_waiting_for_budget_is_skipped() {
        let keys: Vec<u64> = (0..MAX_LANES_GLOBAL + 4).map(|_| spawn()).collect();

        // Kill the over-cap waiters before they ever start.
        for &key in &keys[MAX_LANES_GLOBAL..] {
            kill(key);
        }
        // Then everything else.
        for &key in &keys[..MAX_LANES_GLOBAL] {
            kill(key);
        }

        // No thread may remain for the skipped waiters (or anything
        // else from this batch). Other tests run in parallel, so the
        // assertion only requires convergence, not an exact count.
        assert!(
            wait_until(Duration::from_secs(10), || {
                keys.iter().all(|k| controller_for(*k).is_none())
            }),
            "killed lanes still present in table"
        );
    }
}
