//! The wall around state shared between executors.
//!
//! An executor owns its engine caches (caches.rs). A `static` that holds mutable
//! or lazily built state would be shared by every executor of a process, so each
//! one in the engine outside test code is named in `shared_state.txt`, one
//! `<file>::<name>  <class>  <reason>` a line, with a class that says why it
//! may be shared:
//!
//! - `executor`: the executor's own state, installed per thread (caches.rs's
//!   `INSTALLED`, heap.rs's thread-locals);
//! - `machine`: the same for every executor and never document data (the system
//!   font database, the machine font caches, bundled fonts, the gray
//!   fallback, counters of the process);
//! - `registry`: the native executor table;
//! - `upstream-binding`: set only by upstream's python, ffi or wasm.rs
//!   bindings, which the host never calls (the crypto provider and policy,
//!   the inventory bitset, `MAX_OPERATORS_OVERRIDE`,
//!   `PRESERVE_UNMAPPED_GLYPHS`) or reached only when no executor is installed;
//! - `thread`: scratch of the one thread an executor is, empty between calls (a
//!   recursion guard, a warning sink, a reused buffer); the executor's state
//!   starts clean on its thread (`caches::install`);
//! - `test`: a seam the engine's own tests set.
//!
//! The scan reads the source text: a `static` of type Mutex, RwLock, Atomic*,
//! a `static mut`, every `static` in a `thread_local!`, and a OnceLock or
//! LazyLock of anything but a compiled regex. An unlisted one fails the proof,
//! and so does a listed one the scan no longer finds.
//!
//! No Dart battery sees it: it reads the engine's own source text.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const CLASSES: [&str; 6] =
    ["executor", "machine", "registry", "upstream-binding", "thread", "test"];

/// The files a class may name: `executor` only the executor's own thread-locals, `registry` only
/// the lane table; `thread` only a `thread_local!`.
fn class_fits(class: &str, item: &str, thread_local: bool) -> bool {
    match class {
        "executor" => item.starts_with("src/host/caches.rs::") || item.starts_with("src/host/heap.rs::"),
        "registry" => item.starts_with("src/host/native/executor_table.rs::"),
        "thread" => thread_local,
        _ => true,
    }
}

/// The first line of a `#[cfg(test)]` module: everything after it is test code.
fn code_end(lines: &[&str]) -> usize {
    for (i, line) in lines.iter().enumerate() {
        if line.trim() != "#[cfg(test)]" {
            continue;
        }
        let next = lines[i + 1..].iter().map(|l| l.trim()).find(|l| !l.starts_with("#["));
        if next.is_some_and(|l| {
            l.trim_start_matches("pub ")
                .trim_start_matches("pub(crate) ")
                .starts_with("mod ")
        }) {
            return i;
        }
    }
    lines.len()
}

/// Files that are `#[cfg(test)]` modules: `#[cfg(test)] mod name;` in the file
/// at `path` names `name.rs` or `name/mod.rs` beside it.
fn test_modules_declared_in(path: &Path, lines: &[&str], out: &mut BTreeSet<std::path::PathBuf>) {
    let dir = match path.file_name().and_then(|n| n.to_str()) {
        Some("mod.rs" | "lib.rs" | "main.rs") => path.parent().unwrap().to_path_buf(),
        _ => path.with_extension(""),
    };
    for (at, line) in lines.iter().enumerate() {
        let rest = line.trim();
        let rest = rest.trim_start_matches("pub(crate) ").trim_start_matches("pub ");
        if let Some(name) = rest.strip_prefix("mod ").and_then(|r| r.strip_suffix(';')) {
            if test_only(lines, at) {
                out.insert(dir.join(format!("{name}.rs")));
                out.insert(dir.join(name).join("mod.rs"));
            }
        }
    }
}

/// True when the item at line `at` has `#[cfg(test)]` among the attributes
/// directly above it.
fn test_only(lines: &[&str], at: usize) -> bool {
    lines[..at]
        .iter()
        .rev()
        .map(|l| l.trim())
        .take_while(|l| l.starts_with("#[") || l.starts_with("///") || l.starts_with("//"))
        .any(|l| l == "#[cfg(test)]")
}

/// The name and type of a `static` declared on line `at`, the type read up to
/// the `=` that starts the value (a type may run over several lines).
fn declaration(lines: &[&str], at: usize) -> Option<(String, String, bool)> {
    let mut rest = lines[at].trim();
    for prefix in ["pub(crate) ", "pub(super) ", "pub "] {
        rest = rest.trim_start_matches(prefix);
    }
    let after = rest.strip_prefix("static ")?;
    let (after, is_mut) = match after.strip_prefix("mut ") {
        Some(a) => (a, true),
        None => (after, false),
    };
    let colon = after.find(':')?;
    let name = after[..colon].trim();
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let mut ty = after[colon + 1..].to_string();
    let mut next = at + 1;
    while !ty.contains('=') && !ty.contains(';') && next < lines.len() {
        ty.push(' ');
        ty.push_str(lines[next].trim());
        next += 1;
    }
    let ty = ty.split(|c| c == '=' || c == ';').next().unwrap_or("");
    Some((name.to_string(), ty.split_whitespace().collect::<Vec<_>>().join(" "), is_mut))
}

/// Whether a static of type `ty` holds state a lane could share.
fn shares_state(ty: &str, is_mut: bool, in_thread_local: bool) -> bool {
    let lazy = ["OnceLock<", "LazyLock<", "Lazy<", "OnceCell<"].iter().any(|k| ty.contains(k));
    is_mut
        || in_thread_local
        || ty.contains("Mutex<")
        || ty.contains("RwLock<")
        || ty.contains("Atomic")
        || (lazy && !ty.contains("Regex"))
}

/// Every `.rs` file under `dir`, the proofs' own directory left out.
fn sources_under(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            if !path.ends_with("host/engine_proofs") {
                sources_under(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Every shared static of the engine as `src/<file>::<name>`, with whether it
/// lives in a `thread_local!`.
fn shared_statics(src: &Path) -> BTreeMap<String, bool> {
    let mut files = Vec::new();
    sources_under(src, &mut files);
    let texts: Vec<String> = files.iter().map(|f| std::fs::read_to_string(f).unwrap()).collect();
    let mut test_files = BTreeSet::new();
    for (path, text) in files.iter().zip(&texts) {
        let lines: Vec<&str> = text.split('\n').collect();
        test_modules_declared_in(path, &lines, &mut test_files);
    }
    let mut out = BTreeMap::new();
    for (path, text) in files.iter().zip(&texts) {
        if test_files.contains(path) {
            continue;
        }
        let lines: Vec<&str> = text.split('\n').collect();
        let file = path.strip_prefix(src).unwrap().to_string_lossy().replace('\\', "/");
        let mut thread_local: Option<(i32, bool)> = None;
        for at in 0..code_end(&lines) {
            let line = lines[at];
            if thread_local.is_none() && line.contains("thread_local!") {
                thread_local = Some((0, test_only(&lines, at)));
            }
            if let Some((name, ty, is_mut)) = declaration(&lines, at) {
                let skipped = test_only(&lines, at) || thread_local.is_some_and(|(_, test)| test);
                if shares_state(&ty, is_mut, thread_local.is_some()) && !skipped {
                    out.insert(format!("src/{file}::{name}"), thread_local.is_some());
                }
            }
            if let Some((depth, test)) = thread_local {
                let opened = line.matches('{').count() as i32;
                let closed = line.matches('}').count() as i32;
                let now = depth + opened - closed;
                thread_local = (now > 0 || (opened == 0 && depth == 0)).then_some((now, test));
            }
        }
    }
    out
}

fn listed() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in include_str!("shared_state.txt").lines().filter(|l| !l.trim().is_empty()) {
        let mut parts = line.splitn(3, "  ");
        let item = parts.next().unwrap().trim().to_string();
        let class = parts.next().unwrap_or("").trim().to_string();
        let reason = parts.next().unwrap_or("").trim();
        assert!(CLASSES.contains(&class.as_str()), "{item}: unknown class {class:?}");
        assert!(!reason.is_empty(), "{item}: no reason");
        assert!(out.insert(item.clone(), class).is_none(), "{item} is listed twice");
    }
    out
}

/// Every shared static of the engine is named in `shared_state.txt` with the
/// reason it may be shared, and no listed one is gone.
#[test]
fn every_shared_static_is_listed_with_a_class() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let found = shared_statics(&src);
    let listed = listed();
    let new: Vec<_> = found.keys().filter(|f| !listed.contains_key(*f)).collect();
    let gone: Vec<_> = listed.keys().filter(|l| !found.contains_key(*l)).collect();
    let misfiled: Vec<_> = listed
        .iter()
        .filter(|(item, class)| found.get(*item).is_some_and(|tl| !class_fits(class, item, *tl)))
        .map(|(item, class)| format!("{item} is `{class}`"))
        .collect();
    assert!(
        new.is_empty() && gone.is_empty() && misfiled.is_empty(),
        "statics not on the list: {new:#?}; listed statics the scan no longer finds: {gone:#?}; \
         classes that do not fit their static: {misfiled:#?}"
    );
}
