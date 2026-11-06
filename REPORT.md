# t-541 — Asyncify web mode: stopped at step 1, no sound list fits

Per the plan ("If a sound list cannot be computed, report the exact obstacle
before building anything else"), steps 2–7 are **not built**. The only thing in
`t-541-pkg.patch` is the analysis tool that found the obstacle.

## The obstacle

The functions that can be on the stack at a pause cannot be told apart from
almost the whole module by anything the wasm itself says.

Method (`tool/asyncify_list.dart`, parses the built `pdf_oxide_bg.wasm`; no
name section exists, so functions are identified by index):

1. Pausing imports: `__wbg_host_read_at_*` and `__wbg_host_write_chunk_*`
   (the two the worker wraps in `WebAssembly.Suspending`; `host_cancelled`
   never suspends).
2. Callers of those, transitively, through direct `call`s.
3. A `call_indirect` of type T reaches every function of type T named by an
   element segment or `ref.func`. The tool refuses a module with `table.set`
   / `table.init` / `table.copy` / `table.fill` / an imported table (none of
   those here), which is what makes this resolution sound.

Result on the wasm built here (`make build-wasm`, 20,870,567 bytes, 8,737
functions, 2,354 address-taken):

| Set | Functions |
|---|---|
| callers by direct calls only | 20 |
| callers with `call_indirect` resolved by signature (**sound list**) | 7,442 (85.2%) |

Where the set grows (from the tool's output):

| signature | address-taken ancestors | call-site functions | adds |
|---|---|---|---|
| (i32,i32,i32,i32)->() (`dyn Read::read` / `Write::write`) | 7 | 167 | 1,059 |
| (i32,i32,i32)->() | 88 | 89 | 273 |
| (i32)->() (drop glue, `FnOnce`) | 22 | 321 | 681 |
| (i32,i32)->() | 87 | 78 | **4,642** |
| (i32,i32,i32)->(i32) | 45 | 114 | 724 |

Three small functions (indices 5938, 8640, 8050 in this build, each only a
`call_indirect` of (i32,i32)->() behind 2, 484 and 143 direct callers) account
for most of the 4,642. Treating their indirect calls as non-reaching would give
3,217 functions (37%), but nothing in the wasm proves that: the targets of those
calls are decided by arguments from callers all over the engine. Cutting only
one of them leaves 5,016 / 7,392.

Two refinements that stay inside what the wasm shows did not help (exploratory,
not kept in the tool): reading the vtable slot (`i32.load offset=K` feeding the
`call_indirect`, candidates taken from the data segments at constant+K) leaves
7,436 functions, because 894 of 1,916 indirect call sites are not a vtable load;
and restricting to functions reachable from `execute` changes nothing (8,714 of
8,737 are reachable from it, since every op goes through it).

## What the sound list costs

`--asyncify` with that list (binaryen `asyncify-onlylist`, `asyncify-imports`
limited to the two pausing imports, then `-O2`) on the same wasm:

| | raw | gzip -9 |
|---|---|---|
| normal | 20,870,567 | 8,053,930 |
| asyncify, sound list | 32,449,093 (+55.5%) | 12,674,969 (+57.4%) |
| 25 MiB cap | 26,214,400 | |

6.2 MB over the cap. The plan's earlier prototype (all functions, 26,677,030 on
a 19.6 MB base, +36%) was on a smaller build; this machine builds 20.87 MB
(Rust 1.97, the pin is 1.92), so the number here is higher, but the list is
85% of the module either way, so the asyncify code is not the narrow list the
design assumed. No setting that keeps the list sound brings it under the cap
without the size build, which the design rules out.

The binaryen call took 4 min 5 s (4 cores) and was run through a
`bindgen_runner --asyncify` mode (reverted, below).

## What I did not do, and why

Steps 2–7 (second wasm asset and 25 MiB wall, `PdfIoMode.asyncify`, the worker
driver, `asyncify:` hook option, tests and the runner, the measurement against
OPFS, docs and CHANGELOG): each rests on a wasm that passes the release wall,
and none exists. A list that is guessed (e.g. cutting the three bridges) would
break the plan's rule that the list is computed, not guessed, and would fail in
the field as the `unreachable` the prototype hit. Nothing was built on it. The
wall of step 1 (refuse when the list is stale or misses a reachable function)
was not wired for the same reason; the tool is the analysis it would run.

## Options for the user (not decided here)

1. Make the engine's pause points reachable only through a few known call
   chains: remove the bridges at source (the three functions above sit behind
   generic helpers; the next level exists, so this is a larger engine change),
   then the list would be provable from direct calls plus a short, named
   indirect set.
2. Pause by another means than asyncify of the engine stack, e.g. run the
   engine on a worker thread that owns the I/O in a mode without
   `SharedArrayBuffer` — not available, which is why the design chose asyncify.
3. Accept the OPFS pre-copy for browsers with neither JSPI nor
   `SharedArrayBuffer` (today's behaviour).
4. Accept a wasm over 25 MiB for the asyncify asset only, and ship it where the
   host allows (not Cloudflare static assets) — this changes the design's
   release wall.

## Changes

- `tool/asyncify_list.dart` (new): the analysis above. Run
  `dart --packages=.dart_tool/package_config.json tool/asyncify_list.dart --out list.csv web_assets/pdf_oxide_bg.wasm`
  (`dart run` would first run the native build hook). Not part of the gate or
  `make`; no product code, test or doc changed. The fork is unchanged, so there
  is no `t-541-fork.patch`.

## Gate

| Line | Result |
|---|---|
| `dart analyze --fatal-infos lib test` | No issues |
| `dart analyze --fatal-infos tool/asyncify_list.dart` | No issues |
| `dart test test/guards test/runtime test/bridge test/keep test/hook -p vm` | 427 passed |
| `bash tool/check_changelog_shape.sh` | exit 0 |
| native runner, OPFS / JSPI / Atomics runners, Asyncify runner, size wall | **not run**: no product code changed, the Asyncify runner and wall were not built |
| cargo nextest, `check_patch_markers.sh`, `check_fork_tests.sh` | not run: the fork is unchanged |

## Choices the plan left open

- Taken the narrower reading of "report the exact obstacle before building
  anything else": stopped after step 1's analysis.
- binaryen: the `wasm-opt` 0.116 crate in the workspace (the repo pins it by
  `Cargo.lock`, no separate binaryen install); Rust 1.97 instead of the 1.92
  pin, which is why the normal wasm is 20.87 MB, not about 19.6 MB.
- The tool lives in `tool/`, not `lib/`, so it adds nothing to the barrel or to
  the naming guards.

## Reproducing the 32,449,093-byte figure

Add this to `vendor/pdf_oxide/bindgen_runner/src/main.rs` (reverted), build it
with `cargo build --release -p bindgen_runner`, and run
`bindgen_runner --asyncify web_assets/pdf_oxide_bg.wasm out.wasm list.csv './pdf_oxide_bg.js.__wbg_host_read_at_*,./pdf_oxide_bg.js.__wbg_host_write_chunk_*'`
with the list from `--out`:

```rust
fn asyncify(args: &[String]) -> anyhow::Result<()> {
    let (input, output, onlylist, imports) = (&args[0], &args[1], &args[2], &args[3]);
    let stage = PathBuf::from(format!("{output}.stage"));
    let mut pass = OptimizationOptions::new_opt_level_0();
    pass.add_default_passes(false)
        .add_pass(Pass::Asyncify)
        .set_pass_arg("asyncify-imports", imports)
        .set_pass_arg("asyncify-onlylist", &format!("@{onlylist}"));
    // the same seven Feature::* the O2 step enables
    pass.run(input, &stage)?;
    let mut opts = OptimizationOptions::new_opt_level_2();
    // the same features
    opts.run(&stage, output)?;
    Ok(())
}
```
