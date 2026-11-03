//! Fuel probe host: runs every case of the guest in wasmtime with fuel
//! metering on and measures the work the engine does between two polls of its
//! cancel check.
//!
//! The guest (`tool/fuelprobe/guest`) calls `fuelprobe.poll` on every
//! `CancelCheck::is_cancelled`. The first poll of a run is the guest's mark
//! (the fixture is built and open); the last is its mark at the op's end. In
//! between, each poll records the fuel burnt since the one before. A loop that
//! never polls shows as one long gap, however many items it walks.
//!
//! One row per case: `op fixture max_gap total polls`. Fuel is wasmtime's:
//! about one unit per wasm instruction. Exit code 1 when any row's `max_gap`
//! exceeds `MAX_GAP`, any case does less than `MIN_TOTAL` work, or any case
//! fails to run.
//!
//! Usage: `fuelprobe-host <guest.wasm> [filter]`; a filter keeps the cases
//! whose `op fixture` contains it.

use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use wasmtime::{Caller, Config, Engine, Linker, Module, OptLevel, Store, UpdateDeadline, WasmBacktrace};
use wasmtime_wasi::p1::{self, WasiP1Ctx};
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::WasiCtxBuilder;

/// The widest gap between two polls any case may show, in fuel.
///
/// Derived from measurement on these fixtures, not from a model of the
/// engine: the largest gap of a run of the corpus after the loops it found were
/// polled was 273,182,144 (extractAll on a page of 1,000 lines, the layout
/// passes of one page), times 4, rounded up to the next 100,000,000. It is a
/// wall against regressions, not a statement of what a cancel costs.
const MAX_GAP: u64 = 1_100_000_000;

/// Each case must do at least this much work, or its fixture has shrunk
/// below what the wall is meant to see.
const MIN_TOTAL: u64 = 1_000_000_000;

/// A case that burns this much fuel is cut off and reported as too big: its
/// fixture has outgrown what a run of the wall should cost.
const MAX_TOTAL: u64 = 20_000_000_000;

#[derive(Default)]
struct Probe {
    /// Fuel burnt at the last poll; `None` before the mark.
    last: Option<u64>,
    max_gap: u64,
    total: u64,
    polls: u64,
}

struct State {
    wasi: WasiP1Ctx,
    probe: Arc<Mutex<Probe>>,
    /// Stacks sampled while a gap was longer than `FUELPROBE_SAMPLE` milliseconds.
    samples: Arc<Mutex<std::collections::HashMap<String, u64>>>,
    /// When the guest last polled; a sample is taken when it was long ago.
    polled_at: Arc<Mutex<std::time::Instant>>,
}

struct Row {
    op: String,
    fixture: String,
    outcome: Result<Probe, String>,
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(wasm) = args.next() else {
        eprintln!("usage: fuelprobe-host <guest.wasm> [filter]");
        return ExitCode::from(2);
    };
    let filter = args.next().unwrap_or_default();
    match run(&wasm, &filter) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("fuelprobe-host: {e}");
            ExitCode::FAILURE
        },
    }
}

fn run(wasm: &str, filter: &str) -> wasmtime::Result<bool> {
    let mut config = Config::new();
    config.consume_fuel(true);
    config.epoch_interruption(true);
    config.cranelift_opt_level(OptLevel::Speed);
    let engine = Engine::new(&config)?;
    let module = Module::from_file(&engine, wasm)?;
    // The sampler ticks the epoch every millisecond; it only acts when
    // FUELPROBE_SAMPLE names a gap length in fuel.
    let ticker = engine.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(1));
        ticker.increment_epoch();
    });

    let listing = guest(&engine, &module, &["--list"])?.map_err(wasmtime::Error::msg)?;
    let cases: Vec<(String, String)> = String::from_utf8_lossy(&listing.0)
        .lines()
        .filter_map(|line| line.split_once(' '))
        .filter(|(op, fixture)| format!("{op} {fixture}").contains(filter))
        .map(|(op, fixture)| (op.to_string(), fixture.to_string()))
        .collect();
    if cases.is_empty() {
        return Err(wasmtime::Error::msg("no case matches"));
    }

    println!("{:<20} {:<12} {:>14} {:>16} {:>9}", "op", "fixture", "max_gap", "total", "polls");
    let mut ok = true;
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).min(4);
    let next = Mutex::new(0usize);
    let rows: Mutex<Vec<Option<bool>>> = Mutex::new((0..cases.len()).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let index = {
                    let mut next = next.lock().unwrap();
                    let index = *next;
                    *next += 1;
                    index
                };
                let Some((op, fixture)) = cases.get(index) else { return };
                let outcome = guest(&engine, &module, &[op, fixture])
                    .map_err(|e| e.to_string())
                    .and_then(|run| run.map(|(_, probe)| probe));
                let row = Row { op: op.clone(), fixture: fixture.clone(), outcome };
                let passed = report(&row);
                rows.lock().unwrap()[index] = Some(passed);
            });
        }
    });
    for passed in rows.into_inner().unwrap() {
        ok &= passed.unwrap_or(false);
    }
    Ok(ok)
}

/// Prints the row; false when it breaks the wall.
fn report(row: &Row) -> bool {
    match &row.outcome {
        Err(why) => {
            println!("{:<20} {:<12} FAILED TO RUN: {why}", row.op, row.fixture);
            false
        },
        Ok(probe) => {
            let verdict = if probe.total >= MAX_TOTAL {
                format!("  FAIL: cut off at {MAX_TOTAL}, fixture too big")
            } else if probe.max_gap > MAX_GAP {
                format!("  FAIL: gap above {MAX_GAP}")
            } else if probe.total < MIN_TOTAL {
                format!("  FAIL: total below {MIN_TOTAL}")
            } else {
                String::new()
            };
            println!(
                "{:<20} {:<12} {:>14} {:>16} {:>9}{verdict}",
                row.op, row.fixture, probe.max_gap, probe.total, probe.polls
            );
            verdict.is_empty()
        },
    }
}

/// Runs the guest once with `args`; its stdout and what the probe measured,
/// or why it did not finish.
fn guest(
    engine: &Engine,
    module: &Module,
    args: &[&str],
) -> wasmtime::Result<Result<(Vec<u8>, Probe), String>> {
    let probe = Arc::new(Mutex::new(Probe::default()));
    let stdout = MemoryOutputPipe::new(1 << 20);
    let stderr = MemoryOutputPipe::new(1 << 20);
    let mut builder = WasiCtxBuilder::new();
    builder.arg("fuelprobe-guest").args(args).stdout(stdout.clone()).stderr(stderr.clone());
    let samples = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let mut store = Store::new(
        engine,
        State {
            wasi: builder.build_p1(),
            probe: probe.clone(),
            samples: samples.clone(),
            polled_at: Arc::new(Mutex::new(std::time::Instant::now())),
        },
    );
    store.set_fuel(u64::MAX)?;
    store.set_epoch_deadline(1);
    let threshold: Option<u64> = std::env::var("FUELPROBE_SAMPLE").ok().and_then(|v| v.parse().ok());
    store.epoch_deadline_callback(move |ctx| {
        if let Some(threshold) = threshold {
            let state = ctx.data();
            let marked = state.probe.lock().unwrap().last.is_some();
            let quiet = state.polled_at.lock().unwrap().elapsed().as_millis() as u64;
            if marked && quiet > threshold {
                let trace = WasmBacktrace::capture(&ctx);
                let names: Vec<String> = trace
                    .frames()
                    .iter()
                    .filter_map(|f| f.func_name().map(|n| format!("{:#}", rustc_demangle::demangle(n))))
                    .filter(|n| n.contains("pdf_oxide::") && !n.contains("lexer::") && !n.contains("parser::parse_"))
                    .take(6)
                    .collect();
                *state.samples.lock().unwrap().entry(names.join("\n    <- ")).or_insert(0) += 1;
            }
        }
        Ok(UpdateDeadline::Continue(1))
    });

    let mut linker: Linker<State> = Linker::new(engine);
    p1::add_to_linker_sync(&mut linker, |state: &mut State| &mut state.wasi)?;
    linker.func_wrap("fuelprobe", "poll", |mut caller: Caller<'_, State>| -> wasmtime::Result<()> {
        *caller.data().polled_at.lock().unwrap() = std::time::Instant::now();
        let marked = caller.data().probe.lock().unwrap().last.is_some();
        if !marked {
            // The mark: what the fixture cost is not the op's, so the meter restarts.
            caller.set_fuel(MAX_TOTAL)?;
            caller.data().probe.lock().unwrap().last = Some(0);
            return Ok(());
        }
        let burnt = MAX_TOTAL - caller.get_fuel()?;
        let mut probe = caller.data().probe.lock().unwrap();
        let gap = burnt - probe.last.unwrap_or(0);
        probe.max_gap = probe.max_gap.max(gap);
        probe.total += gap;
        probe.polls += 1;
        probe.last = Some(burnt);
        Ok(())
    })?;
    // The wasm-bindgen imports of the browser edge stay unlinked: nothing
    // on a probe's path calls them.
    linker.define_unknown_imports_as_traps(module)?;

    let instance = linker.instantiate(&mut store, module)?;
    let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
    let result = start.call(&mut store, ());
    let exit = match result {
        Ok(()) => 0,
        Err(e) if probe.lock().unwrap().last.is_some() && store.get_fuel().is_ok_and(|left| left == 0) => {
            let mut measured = std::mem::take(&mut *probe.lock().unwrap());
            let tail = MAX_TOTAL - measured.last.unwrap_or(0);
            measured.max_gap = measured.max_gap.max(tail);
            measured.total = MAX_TOTAL;
            return Ok(Ok((Vec::new(), measured)));
        },
        Err(e) => match e.downcast_ref::<wasmtime_wasi::I32Exit>() {
            Some(code) => code.0,
            None => {
                return Ok(Err(format!(
                    "trap: {e}; stderr: {}",
                    String::from_utf8_lossy(&stderr.contents())
                )))
            },
        },
    };
    if exit != 0 {
        return Ok(Err(format!(
            "exit {exit}: {}",
            String::from_utf8_lossy(&stderr.contents()).trim()
        )));
    }
    if threshold.is_some() {
        let mut top: Vec<_> = samples.lock().unwrap().drain().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1));
        let mut block = format!("== {}\n", args.join(" "));
        for (stack, count) in top.into_iter().take(5) {
            block.push_str(&format!("  [{count} samples]\n    {stack}\n"));
        }
        eprint!("{block}");
    }
    let mut measured = std::mem::take(&mut *probe.lock().unwrap());
    // The count is the polls between the two marks, the marks excluded.
    measured.polls = measured.polls.saturating_sub(1);
    Ok(Ok((stdout.contents().to_vec(), measured)))
}
