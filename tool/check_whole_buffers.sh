#!/usr/bin/env bash
# Whole-buffer lint (d-122, d-132): an engine site that reads a whole stream,
# file or page into memory fails the build unless it is a table by nature or the
# build can prove no lane reaches it.
#
# Memory is bounded for any number of pages, so a call that hands back every byte
# of a stream, a source or a page in one buffer is a hole in that promise unless
# what it reads is whole by nature: a font program, a sampled function, a
# PostScript calculator function, an ICC profile, the CMap table, a DER
# certificate or the DSS, the xref stream, one object-stream member, a
# progressive JPEG raster, a JPX recompressed at full size, a JBIG2 stream and
# its globals, a JPX codestream. Everything else reads in pieces.
#
# Walks the tracked and new .rs files of vendor/pdf_oxide/src (not
# src/host/engine_proofs/, not `#[cfg(test)]` or `#[test]` code) and finds each
# call matching a regex of tool/whole_buffer_patterns.txt (one per line, a `#`
# comment after it names what it matches). A call that matches several patterns
# is one site. Two things are not sites: a `read_to_end(` or `read_to_string(` on
# a chain with a `.take(` in its statement, which is a read of a bounded window,
# and a call inside a function that is itself one of the patterns, which is the
# helper's own body (its callers are the sites). It fails with file:line unless
# the site is a line of
# tool/whole_buffer_allow.txt:
#       <path under vendor/pdf_oxide>::<fn>#<n>  # <class>: why
# where <n> counts the flagged sites of that function, from 1, in source order.
# There are two classes:
#   table:      the why names one of the kinds above.
#   unreached:  no lane can reach the site, and the script proves it rather than
#               trusting the why. A site is unreached when one of these holds:
#                 gate    it sits in a `#[cfg(..)]` item, or in a module declared
#                         under one, that is off for both feature strings of
#                         build.json (and every feature they imply in the
#                         engine's Cargo.toml), or in a file no `mod` line
#                         declares. `not(..)` and target gates prove nothing.
#                 path    its function takes a `Path`, `&Path` or `PathBuf`: a
#                         lane holds a `DataSource`, never a path.
#                 callers the names its function is called through appear in
#                         no file under vendor/pdf_oxide/src/host and none under
#                         lib/, and the same holds for each function that calls
#                         it (one level). A free function is called through its
#                         name, a method through its type and its trait, since a
#                         method name is never unique. A `use`, `struct`, `enum`,
#                         `trait` or `impl` line is no call. A caller in
#                         `#[cfg(test)]` code or under src/host/engine_proofs/ is
#                         not a caller.
# A bound is no class: a `.take(N)` in the code is the window.
#
# An allow line that matches no flagged site fails too, so the file cannot
# outlive the code it excuses. `--sites` prints every flagged site and its key,
# for writing the allow list, with the proof that holds for it, and the reads it
# does not count as sites.
set -euo pipefail

cd "$(dirname "$0")/.."

[ -d vendor/pdf_oxide/src ] || exit 0

git -C vendor/pdf_oxide ls-files --cached --others --exclude-standard -z -- 'src/*.rs' \
  | python3 -I -c '
import json
import os
import re
import sys

ROOT = "vendor/pdf_oxide/"
PATTERNS_FILE = "tool/whole_buffer_patterns.txt"
ALLOW_FILE = "tool/whole_buffer_allow.txt"
# What an allow line may call a table.
KINDS = [
    "font program",
    "sampled function",
    "PostScript calculator function",
    "ICC profile",
    "CMap table",
    "DER certificate",
    "DSS",
    "xref stream",
    "object-stream member",
    "progressive JPEG raster",
    "JPX recompressed at full size",
    "JBIG2 stream and globals",
    "JPX codestream",
]
LIST_SITES = "--sites" in sys.argv[1:]


def read_lines(path):
    """The meaningful lines of a list file: (number, text, comment) triples."""
    out = []
    with open(path, encoding="utf-8") as f:
        for number, raw in enumerate(f, 1):
            raw = raw.rstrip("\n")
            if not raw.strip() or raw.lstrip().startswith("#"):
                continue
            text, _, comment = raw.partition("  #")
            out.append((number, text.strip(), comment.strip()))
    return out


def blank(src):
    """`src` with comments, strings and char literals replaced by spaces
    (newlines kept), so calls found in it are code."""
    out = list(src)
    n = len(src)
    i = 0

    def wipe(a, b):
        for k in range(a, b):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            wipe(i, j)
            i = j
        elif src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif src.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            wipe(i, j)
            i = j
        elif c == "r" and re.match(r"r#*\"", src[i:i + 12]) and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            hashes = len(re.match(r"r(#*)\"", src[i:i + 12]).group(1))
            close = "\"" + "#" * hashes
            j = src.find(close, i + 2 + hashes)
            j = n if j < 0 else j + len(close)
            wipe(i, j)
            i = j
        elif c == "\"":
            j = i + 1
            while j < n and src[j] != "\"":
                j += 2 if src[j] == "\\" else 1
            wipe(i, min(j + 1, n))
            i = j + 1
        elif c == "\x27":
            m = re.match(r"\x27(\\\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|.)|[^\\\\\x27])\x27", src[i:i + 14])
            if m:
                wipe(i, i + m.end())
                i += m.end()
            else:
                i += 1
        else:
            i += 1
    return "".join(out)


def match_brace(text, at):
    """Index of the `}` closing the `{` at `at`, or -1."""
    depth = 0
    for k in range(at, len(text)):
        if text[k] == "{":
            depth += 1
        elif text[k] == "}":
            depth -= 1
            if depth == 0:
                return k
    return -1


TEST_ATTR = re.compile(r"#\s*\[\s*(?:cfg\s*\(\s*(?:all\s*\(\s*)?test\b[^\]]*|test)\]")
INNER_TEST = re.compile(r"#\s*!\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]")


def test_ranges(text):
    """Spans of `#[cfg(test)]` and `#[test]` items."""
    ranges = []
    if INNER_TEST.search(text):
        return [(0, len(text))]
    for m in TEST_ATTR.finditer(text):
        k = m.end()
        # further attributes, then the item: ends at its block or at a `;`
        while True:
            skip = re.match(r"\s*#\s*\[[^\]]*\]", text[k:])
            if not skip:
                break
            k += skip.end()
        brace = text.find("{", k)
        semi = text.find(";", k)
        if brace >= 0 and (semi < 0 or brace < semi):
            end = match_brace(text, brace)
            ranges.append((m.start(), len(text) if end < 0 else end + 1))
        elif semi >= 0:
            ranges.append((m.start(), semi + 1))
    return ranges


FN = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)")


def functions_of(text):
    out = []
    for m in FN.finditer(text):
        depth, k = 0, m.end()
        while k < len(text):
            c = text[k]
            if c in "([":
                depth += 1
            elif c in ")]":
                depth -= 1
            elif c == "{" and depth <= 0:
                break
            elif c == ";" and depth <= 0:
                k = -1
                break
            k += 1
        if k < 0 or k >= len(text):
            continue
        end = match_brace(text, k)
        if end >= 0:
            out.append((m.start(), m.group(1), k, end))
    return out


IMPL = re.compile(
    r"\bimpl\b\s*(?:<[^{]*?>)?\s*(?:([A-Za-z_][\w:]*)(?:<[^{]*?>)?\s+for\s+)?([A-Za-z_][\w:]*)[^{;]*\{"
    r"|\btrait\s+([A-Za-z_]\w*)[^{;]*\{"
)


def blocks_of(text):
    """(start, end, names) of each impl and trait block: the names a method in
    it is called through, the type and the trait, never the method by itself."""
    out = []
    for m in IMPL.finditer(text):
        end = match_brace(text, m.end() - 1)
        if end < 0:
            continue
        names = [n.split("::")[-1] for n in m.groups() if n]
        out.append((m.start(), end, names))
    return out


def enclosing_fn(at, fns):
    best = None
    for start, name, body_start, body_end in fns:
        if body_start < at < body_end:
            best = (start, name, body_start, body_end)
    return best


CFG_ATTR = re.compile(r"#\s*(!?)\s*\[\s*cfg\s*\(")


def cfg_items(src, text):
    """Spans of the items under a `#[cfg(..)]` (the whole file for `#![cfg(..)]`),
    as (start, end, predicate) triples. `src` supplies the feature names that
    `text` has blanked."""
    out = []
    for m in CFG_ATTR.finditer(src):
        if text[m.start()] != "#":
            continue
        depth, k = 1, m.end()
        while k < len(src) and depth:
            depth += {"(": 1, ")": -1}.get(src[k], 0)
            k += 1
        pred = src[m.end():k - 1]
        if m.group(1):
            out.append((0, len(src), pred))
            continue
        k = text.find("]", k) + 1
        while True:
            skip = re.match(r"\s*#\s*\[[^\]]*\]", text[k:])
            if not skip:
                break
            k += skip.end()
        brace = text.find("{", k)
        semi = text.find(";", k)
        if brace >= 0 and (semi < 0 or brace < semi):
            end = match_brace(text, brace)
            out.append((m.start(), len(text) if end < 0 else end + 1, pred))
        elif semi >= 0:
            out.append((m.start(), semi + 1, pred))
    return out


def parse_cfg(pred):
    toks = re.findall(r"[A-Za-z_][A-Za-z0-9_]*|\"[^\"]*\"|[(),=]", pred)
    pos = 0

    def expr():
        nonlocal pos
        name = toks[pos]
        pos += 1
        if toks[pos] == "(":
            pos += 1
            args = []
            while toks[pos] != ")":
                args.append(expr())
                if toks[pos] == ",":
                    pos += 1
            pos += 1
            return (name, args)
        if toks[pos] == "=":
            pos += 2
            return (name, toks[pos - 1].strip(chr(34)))
        return (name, None)

    return expr()


def provably_off(node, enabled):
    """True when the predicate is false for every feature set `enabled` allows.
    `not(..)` and every non-feature gate prove nothing."""
    name, arg = node
    if name == "feature":
        return arg not in enabled
    if name == "any":
        return all(provably_off(a, enabled) for a in arg)
    if name == "all":
        return any(provably_off(a, enabled) for a in arg)
    return False


def cfg_off(pred, enabled):
    try:
        return provably_off(parse_cfg(pred), enabled)
    except IndexError:
        return False


def enabled_features():
    """The features the package builds the engine with (both strings of
    build.json) and every feature those imply in the Cargo.toml of the engine."""
    with open("build.json", encoding="utf-8") as f:
        strings = json.load(f)["features"].values()
    with open(ROOT + "Cargo.toml", encoding="utf-8") as f:
        cargo = f.read()
    section = re.search(r"^\[features\]\s*$(.*?)(?=^\[|\Z)", cargo, re.S | re.M).group(1)
    section = re.sub(r"#[^\n]*", "", section)
    table = {
        m.group(1): re.findall(r"\"([^\"]*)\"", m.group(2))
        for m in re.finditer(r"^([A-Za-z0-9_-]+)\s*=\s*\[(.*?)\]", section, re.S | re.M)
    }
    todo = ["default"] + [name for s in strings for name in s.split(",")]
    enabled = set()
    while todo:
        name = todo.pop()
        if name in enabled:
            continue
        enabled.add(name)
        todo += [e for e in table.get(name, []) if not e.startswith("dep:") and "/" not in e]
    return enabled


class Source:
    def __init__(self, src):
        self.src = src
        self.text = blank(src)
        self.skip = test_ranges(self.text)
        self.fns = functions_of(self.text)
        self.blocks = blocks_of(self.text)
        self.cfgs = cfg_items(src, self.text)

    def in_test(self, at):
        return any(a <= at < b for a, b in self.skip)

    def names_of(self, fn):
        """A free function is found by its name; a method by its type and
        trait, since the bare name of a method is never unique."""
        inner = None
        for start, end, names in self.blocks:
            if start < fn[0] < end and (inner is None or start > inner[0]):
                inner = (start, end, names)
        return inner[2] if inner else [fn[1]]


def gate_proof(path, at, files, enabled):
    """The `#[cfg(..)]` that compiles the site out, or None."""
    rec = files[path]
    for a, b, pred in rec.cfgs:
        if a <= at < b and cfg_off(pred, enabled):
            return f"gate: inside #[cfg({pred})] in {path}"
    rel = path[len("src/"):]
    if rel.startswith("bin/") or rel in ("lib.rs", "main.rs"):
        return None
    parts = rel[:-len(".rs")].split("/")
    if parts[-1] == "mod":
        parts.pop()
    for depth in range(len(parts), 0, -1):
        name, parent = parts[depth - 1], "/".join(parts[:depth - 1])
        for cand in ([f"src/{parent}/mod.rs", f"src/{parent}.rs"] if parent else ["src/lib.rs"]):
            up = files.get(cand)
            if not up:
                continue
            decl = re.search(r"\bmod\s+" + re.escape(name) + r"\s*[;{]", up.text)
            if not decl:
                base = re.escape(path.rsplit("/", 1)[-1])
                if any(re.search(r"#\s*\[\s*path\s*=\s*\"[^\"]*\b" + base + "\"", f.src) for f in files.values()):
                    return None
                return f"gate: module {name} is declared nowhere ({cand} has no `mod {name}`), so no build compiles it"
            for a, b, pred in up.cfgs:
                if a <= decl.start() < b and cfg_off(pred, enabled):
                    return f"gate: module {name} is declared under #[cfg({pred})] in {cand}"
    return None


def path_proof(fn, text):
    """The Path parameter of the enclosing function, or None."""
    start, name, body_start, _ = fn
    open_at = None
    angle = 0
    for k in range(start, body_start):
        c = text[k]
        if c == "<":
            angle += 1
        elif c == ">" and text[k - 1] != "-":
            angle -= 1
        elif c == "(" and angle <= 0:
            open_at = k
            break
    if open_at is None:
        return None
    depth, k = 0, open_at
    while k < body_start:
        depth += {"(": 1, ")": -1}.get(text[k], 0)
        k += 1
        if depth == 0:
            break
    m = re.search(r"\b(?:Path|PathBuf)\b", text[start:k])
    return f"path: {name} takes a {m.group(0)}" if m else None


class Mentions:
    """Where a name appears under src/host (not engine_proofs, not test code)
    and under lib/."""

    def __init__(self, files):
        self.texts = []
        for base in (ROOT + "src/host", "lib"):
            for dirpath, dirs, names in os.walk(base):
                dirs[:] = [d for d in dirs if d != "engine_proofs"]
                for n in names:
                    full = os.path.join(dirpath, n)
                    rel = full[len(ROOT):] if full.startswith(ROOT) else full
                    rec = files.get(rel)
                    try:
                        if rec:
                            text = "".join(" " if any(a <= k < b for a, b in rec.skip) and c != "\n" else c for k, c in enumerate(rec.src))
                        else:
                            with open(full, encoding="utf-8") as f:
                                text = f.read()
                    except (UnicodeDecodeError, OSError):
                        continue
                    self.texts.append((full, text))

    def find(self, name):
        pattern = re.compile(r"\b" + re.escape(name) + r"\b")
        for full, text in self.texts:
            for m in pattern.finditer(text):
                if not declares(text, m.start()):
                    return f"{full}:{text.count(chr(10), 0, m.start()) + 1}"
        return None


DECLARATION = re.compile(r"\s*(?:pub(?:\([^)]*\))?\s+)?(?:use|struct|enum|trait|impl)\b")


def declares(text, at):
    """A `use`, `struct`, `enum`, `trait` or `impl` line names a type without calling it."""
    return bool(DECLARATION.match(text, text.rfind("\n", 0, at) + 1))


def callers_proof(fn, path, files, mentions):
    """None-or-reason: the names the function is called through, and those of
    each function that calls it, must appear in no file under src/host or lib/."""
    names = files[path].names_of(fn)
    for name in names:
        hit = mentions.find(name)
        if hit:
            return f"callers: {name} appears at {hit}"
    seen = set()
    for name in names:
        pattern = re.compile(r"\b" + re.escape(name) + r"\b")
        for other, rec in files.items():
            if other.startswith("src/host/engine_proofs/"):
                continue
            for m in pattern.finditer(rec.text):
                if rec.in_test(m.start()) or declares(rec.text, m.start()) or re.search(r"\bfn\s+$", rec.text[:m.start()]):
                    continue
                caller = enclosing_fn(m.start(), rec.fns)
                if caller is None:
                    return f"callers: {name} is used outside a function at {other}:{rec.text.count(chr(10), 0, m.start()) + 1}"
                if other == path and caller[0] == fn[0]:
                    continue
                for outer in rec.names_of(caller):
                    if outer in seen or outer in names:
                        continue
                    seen.add(outer)
                    hit = mentions.find(outer)
                    if hit:
                        return f"callers: {name} is used by {caller[1]} ({other}:{rec.text.count(chr(10), 0, m.start()) + 1}), found through {outer} at {hit}"
    return None


def prove(path, at, fn, files, enabled, mentions):
    """(proof that holds, or None; why each failed)."""
    failed = []
    gate = gate_proof(path, at, files, enabled)
    if gate:
        return gate, failed
    failed.append("no cfg gate that is off for the build")
    if fn:
        proof = path_proof(fn, files[path].text)
        if proof:
            return proof, failed
        failed.append(f"{fn[1]} takes no Path")
        reason = callers_proof(fn, path, files, mentions)
        if not reason:
            return f"callers: {" and ".join(files[path].names_of(fn))} and its callers appear in no file under src/host or lib/", failed
        failed.append(reason)
    return None, failed


patterns = []
allow = {}
if not LIST_SITES:
    for number, text, comment in read_lines(PATTERNS_FILE):
        if not comment:
            print(f"{PATTERNS_FILE}:{number}: pattern with no comment naming what it matches")
            sys.exit(1)
        patterns.append(re.compile(text))
    for number, text, comment in read_lines(ALLOW_FILE):
        if not comment:
            print(f"{ALLOW_FILE}:{number}: allowed site with no reason")
            sys.exit(1)
        if comment.startswith("table: "):
            if not any(kind in comment for kind in KINDS):
                print(f"{ALLOW_FILE}:{number}: the why names none of the kinds that are tables by nature: " + ", ".join(KINDS))
                sys.exit(1)
        elif not comment.startswith("unreached: ") or not comment[len("unreached: "):].strip():
            print(f"{ALLOW_FILE}:{number}: an allowed site is `# table: <kind>` or `# unreached: <why>`; a read of anything else is bounded instead")
            sys.exit(1)
        allow[text] = [number, False, comment.startswith("unreached: ")]
else:
    for number, text, comment in read_lines(PATTERNS_FILE):
        patterns.append(re.compile(text))

files = {}
for path in sys.stdin.buffer.read().split(b"\0"):
    if not path:
        continue
    path = path.decode()
    if path.startswith("src/host/engine_proofs/"):
        continue
    try:
        with open(ROOT + path, encoding="utf-8") as f:
            files[path] = Source(f.read())
    except (UnicodeDecodeError, FileNotFoundError, IsADirectoryError):
        continue

enabled = enabled_features()
mentions = Mentions(files)

bad = 0
for path, rec in files.items():
    text, skip, fns = rec.text, rec.skip, rec.fns
    # One site per call: the offset just past the name, which patterns that
    # name the same call (`std::fs::read(` and `fs::read(`) share.
    sites = {}
    for pattern in patterns:
        for m in pattern.finditer(text):
            if any(a <= m.start() < b for a, b in skip):
                continue
            if re.search(r"\bfn\s+$", text[:m.start()]):
                continue
            sites.setdefault(m.end(), (m.start(), m.group(0)))
    # What is not a site: the helper bodies and the windowed reads.
    helper_bodies = [(b, e) for _, name, b, e in fns if any(p.search(name + "(") for p in patterns)]
    for end in list(sites):
        start, called = sites[end]
        statement = max(text.rfind(c, 0, start) for c in ";{}")
        windowed = re.match(r"\.?(read_to_end|read_to_string)\(", called.strip()) and ".take(" in text[statement + 1:start]
        inside = any(b < start < e for b, e in helper_bodies)
        if windowed or inside:
            if LIST_SITES:
                line = text.count("\n", 0, start) + 1
                why = "bounded by take" if windowed else "body of a whole-read helper"
                print(f"{ROOT}{path}:{line}: not a site ({why}): {called.strip()}")
            del sites[end]
    counts = {}
    for end in sorted(sites):
        start, called = sites[end]
        line = text.count("\n", 0, start) + 1
        fn = enclosing_fn(start, fns)
        name = fn[1] if fn else "-"
        counts[name] = counts.get(name, 0) + 1
        key = f"{path}::{name}#{counts[name]}"
        if LIST_SITES:
            proof, _ = prove(path, start, fn, files, enabled, mentions)
            print(f"{ROOT}{path}:{line}: {key}: {called.strip()}" + (f"  [would pass as unreached: {proof}]" if proof else ""))
            continue
        if key not in allow:
            print(f"{ROOT}{path}:{line}: reads a whole stream, file or page into one buffer: {called.strip()}")
            print(f"    read it in bounded pieces, or allow it as {key}  # table: <kind>  (or `# unreached: <why>` when no lane reaches it)")
            bad += 1
            continue
        allow[key][1] = True
        if allow[key][2]:
            proof, failed = prove(path, start, fn, files, enabled, mentions)
            if not proof:
                print(f"{ALLOW_FILE}:{allow[key][0]}: {key} is not unreached ({ROOT}{path}:{line}): " + "; ".join(failed))
                bad += 1

for key, (number, used, _) in allow.items():
    if not used and not LIST_SITES:
        print(f"{ALLOW_FILE}:{number}: {key} matches no flagged site")
        bad += 1

if bad:
    print(f"{bad} site(s) read a whole buffer that is not a table and that no proof shows unreached")
    sys.exit(1)
if not LIST_SITES:
    print("every whole-buffer read is a table by nature or unreached by proof")
' "$@"
