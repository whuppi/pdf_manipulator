// Post-processes the compiled wasm for the web build: wasm-bindgen and
// wasm-opt run as library calls instead of separately installed CLIs, so
// their versions resolve from this workspace's own Cargo.lock and can
// never drift from the wasm-bindgen crate the wasm was compiled with
// (pdf_manipulator#177).
//
//   bindgen_runner <input.wasm> <out-dir>
//
// Produces <out-dir>/pdf_oxide.js + <out-dir>/pdf_oxide_bg.wasm, the
// same outputs `wasm-bindgen --target web --out-name pdf_oxide` plus
// `wasm-opt -O2` produced.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::exit;

use wasm_opt::{Feature, OptimizationOptions};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: bindgen_runner <input.wasm> <out-dir>");
        exit(2);
    }
    let input = PathBuf::from(&args[1]);
    let out_dir = PathBuf::from(&args[2]);
    fs::create_dir_all(&out_dir)?;

    // omit_default_module_path defaults to TRUE in the library (the CLI
    // forces it false) — keep the no-arg `init()` sibling-URL fallback
    // the glue always shipped with, or downstream loaders break.
    let mut bindgen = wasm_bindgen_cli_support::Bindgen::new();
    bindgen
        .input_path(&input)
        .web(true)?
        .typescript(false)
        .omit_default_module_path(false)
        .out_name("pdf_oxide")
        .generate(&out_dir)?;

    // Optimize to a temp file and rename on success: writing the input
    // path directly would truncate the only copy on a mid-write crash.
    let bg = out_dir.join("pdf_oxide_bg.wasm");
    let tmp = out_dir.join("pdf_oxide_bg.wasm.opt");
    let mut opts = OptimizationOptions::new_opt_level_2();
    for feature in [
        Feature::BulkMemory,
        Feature::Multivalue,
        Feature::MutableGlobals,
        Feature::TruncSat,
        Feature::ReferenceTypes,
        Feature::SignExt,
        Feature::Simd,
    ] {
        opts.enable_feature(feature);
    }
    opts.run(&bg, &tmp)?;
    fs::rename(&tmp, &bg)?;
    Ok(())
}
