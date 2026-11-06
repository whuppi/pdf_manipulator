#!/usr/bin/env bash
# Whole-buffer lint (d-122): an engine site that reads a whole stream, file or
# page into memory fails the build unless it is one of the kinds that are tables
# by nature.
#
# Memory is bounded for any number of pages, so a call that hands back every byte
# of a stream, a source or a page in one buffer is a hole in that promise unless
# what it reads is whole by nature: a font program, a sampled function, an ICC
# profile, the CMap table, a DER certificate or the DSS, the xref stream, one
# object-stream member, a progressive JPEG raster, a JPX recompressed at full
# size. Everything else reads in pieces.
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
#       <path under vendor/pdf_oxide>::<fn>#<n>  # table: why
# where <n> counts the flagged sites of that function, from 1, in source order.
# `table:` is the only class an allow line may name, and its why names one of the
# kinds above; any other word fails. A site that reads something that is not a
# table is not allowed: it is a bug to fix with a bounded read.
#
# An allow line that matches no flagged site fails too, so the file cannot
# outlive the code it excuses. `--sites` prints every flagged site and its key,
# for writing the allow list, and the reads it does not count as sites.
set -euo pipefail

cd "$(dirname "$0")/.."

[ -d vendor/pdf_oxide/src ] || exit 0

git -C vendor/pdf_oxide ls-files --cached --others --exclude-standard -z -- 'src/*.rs' \
  | python3 -I -c '
import re
import sys

ROOT = "vendor/pdf_oxide/"
PATTERNS_FILE = "tool/whole_buffer_patterns.txt"
ALLOW_FILE = "tool/whole_buffer_allow.txt"
# What an allow line may call a table: the d-122 allow-list.
KINDS = [
    "font program",
    "sampled function",
    "ICC profile",
    "CMap table",
    "DER certificate",
    "DSS",
    "xref stream",
    "object-stream member",
    "progressive JPEG raster",
    "JPX recompressed at full size",
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


TEST_ATTR = re.compile(r"#\s*\[\s*(?:cfg\s*\(\s*(?:all\s*\(\s*|any\s*\(\s*)?test\b[^\]]*|test)\]")
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


def enclosing_fn(at, fns):
    best = None
    for start, name, body_start, body_end in fns:
        if body_start < at < body_end:
            best = (start, name, body_start, body_end)
    return best


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
        if not comment.startswith("table: "):
            print(f"{ALLOW_FILE}:{number}: an allowed site is `# table: <why>`; a read of anything that is not a table is bounded instead")
            sys.exit(1)
        if not any(kind in comment for kind in KINDS):
            print(f"{ALLOW_FILE}:{number}: the why names none of the kinds that are tables by nature: " + ", ".join(KINDS))
            sys.exit(1)
        allow[text] = [number, False]
else:
    for number, text, comment in read_lines(PATTERNS_FILE):
        patterns.append(re.compile(text))

bad = 0
for path in sys.stdin.buffer.read().split(b"\0"):
    if not path:
        continue
    path = path.decode()
    if path.startswith("src/host/engine_proofs/"):
        continue
    try:
        with open(ROOT + path, encoding="utf-8") as f:
            src = f.read()
    except (UnicodeDecodeError, FileNotFoundError, IsADirectoryError):
        continue
    text = blank(src)
    skip = test_ranges(text)
    fns = functions_of(text)
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
            print(f"{ROOT}{path}:{line}: {key}: {called.strip()}")
            continue
        if key in allow:
            allow[key][1] = True
            continue
        print(f"{ROOT}{path}:{line}: reads a whole stream, file or page into one buffer: {called.strip()}")
        print(f"    read it in bounded pieces, or allow it as {key}  # table: <kind>")
        bad += 1

for key, (number, used) in allow.items():
    if not used and not LIST_SITES:
        print(f"{ALLOW_FILE}:{number}: {key} matches no flagged site")
        bad += 1

if bad:
    print(f"{bad} site(s) read a whole buffer that is not a table")
    sys.exit(1)
if not LIST_SITES:
    print("every whole-buffer read is a table by nature")
' "$@"
