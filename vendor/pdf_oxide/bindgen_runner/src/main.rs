// Post-processes the compiled wasm for the web build: wasm-bindgen and
// wasm-opt run as library calls instead of separately installed CLIs, so
// their versions resolve from this workspace's own Cargo.lock and can
// never drift from the wasm-bindgen crate the wasm was compiled with
// (pdf_manipulator#177).
//
//   bindgen_runner [--opt=<level>] [--keep-names] <input.wasm> <out-dir>
//
// Produces <out-dir>/pdf_oxide.js + <out-dir>/pdf_oxide_bg.wasm, the
// same outputs `wasm-bindgen --target web --out-name pdf_oxide` plus
// `wasm-opt -O<level>` produced.
//
//   bindgen_runner [--opt=<level>] [--keep-names] --asyncify <in.wasm> \
//       <out.wasm> <onlylist-file> <imports>
//
// Rewrites an optimized module so it can pause at the imports named in
// <imports> (a comma list of `module.name`) and resume later. Only the
// functions named in <onlylist-file> (comma-separated function indices, from
// lib/src/hook/asyncify_list.dart) are instrumented.
//
// <level> is wasm-opt's: 0 to 4, s or z, default 2. The engine's build mode
// picks it (lib/src/hook/engine_build.dart), and both commands take the same
// one, so the Asyncify wasm is optimized like the wasm it comes from.
// --keep-names keeps the name section, for a build that must symbolicate.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::exit;

use wasm_opt::{Feature, OptimizationOptions, Pass};

const USAGE: &str = "usage: bindgen_runner [--opt=<0-4|s|z>] [--keep-names] <input.wasm> <out-dir>\n       bindgen_runner [--opt=<0-4|s|z>] [--keep-names] --asyncify <in.wasm> <out.wasm> <onlylist-file> <imports>";

struct Level {
    opt: String,
    keep_names: bool,
}

impl Level {
    fn options(&self) -> OptimizationOptions {
        let mut opts = match self.opt.as_str() {
            "0" => OptimizationOptions::new_opt_level_0(),
            "1" => OptimizationOptions::new_opt_level_1(),
            "2" => OptimizationOptions::new_opt_level_2(),
            "3" => OptimizationOptions::new_opt_level_3(),
            "4" => OptimizationOptions::new_opt_level_4(),
            "s" => OptimizationOptions::new_optimize_for_size(),
            "z" => OptimizationOptions::new_optimize_for_size_aggressively(),
            other => {
                eprintln!("unknown --opt level {other:?}\n{USAGE}");
                exit(2);
            },
        };
        opts.debug_info(self.keep_names);
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
        opts
    }
}

// Asyncify first, on its own, and the optimizer after it: the pass emits
// code the default pipeline is meant to clean up, and binaryen's default
// pipeline runs before any pass added beside it. The pass itself carries the
// build's level: binaryen's Asyncify reads it and, above 0, cleans up each
// instrumented function's locals before it saves them (at level 0 the
// engine's Asyncify wasm is 3.7 MB larger than at 2).
fn asyncify(level: &Level, args: &[String]) -> anyhow::Result<()> {
    let (input, output, onlylist, imports) = (&args[0], &args[1], &args[2], &args[3]);
    let stage = PathBuf::from(format!("{output}.stage"));
    let mut pass = level.options();
    pass.add_default_passes(false)
        .add_pass(Pass::Asyncify)
        .set_pass_arg("asyncify-imports", imports)
        .set_pass_arg("asyncify-onlylist", &format!("@{onlylist}"));
    pass.run(input, &stage)?;
    let tmp = PathBuf::from(format!("{output}.opt"));
    level.options().run(&stage, &tmp)?;
    fs::remove_file(&stage)?;
    fs::rename(&tmp, output)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let mut level = Level {
        opt: "2".to_string(),
        keep_names: false,
    };
    let mut args: Vec<String> = Vec::new();
    for arg in env::args().skip(1) {
        if let Some(opt) = arg.strip_prefix("--opt=") {
            level.opt = opt.to_string();
        } else if arg == "--keep-names" {
            level.keep_names = true;
        } else {
            args.push(arg);
        }
    }
    if args.len() == 5 && args[0] == "--asyncify" {
        return asyncify(&level, &args[1..]);
    }
    if args.len() != 2 {
        eprintln!("{USAGE}");
        exit(2);
    }
    let input = PathBuf::from(&args[0]);
    let out_dir = PathBuf::from(&args[1]);
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
    level.options().run(&bg, &tmp)?;
    fs::rename(&tmp, &bg)?;
    Ok(())
}
