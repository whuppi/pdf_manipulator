#!/usr/bin/env bash
# Loop poll lint (d-126 step 6): a loop over a document-scale collection polls
# for a cancel.
#
# On web without SharedArrayBuffer the cancel check is the only way to stop a
# job, so a loop that walks pages, xref entries, content operators, annotations
# and the like without polling is a job that cannot be cancelled. The fuel
# probe (`make test-fuel`) is the proof; this is the cheap early warning.
#
# Walks the tracked and new .rs files of vendor/pdf_oxide/src (not
# src/host/engine_proofs/, not `#[cfg(test)]` or `#[test]` code) and finds each
# `for` / `while` / `loop` whose header matches a regex of
# tool/cancel_poll_patterns.txt (one per line, a `#` comment after it names
# what it matches; a `loop` has no header, so its header is its first
# statement). It fails with file:line when the loop's body, to its closing
# brace, holds none of
#   - `is_cancelled`
#   - `.check(`
#   - a call to a polling helper: `.tick(` (Stride), `.each(` (CancelCheck),
#     or a function named in HELPERS below (`poll`, the running job's check)
# unless
#   - a loop around it, in the same function, holds one (the outer loop polls
#     once per item, so the inner loop is one item's work), or
#   - its place is a line of tool/cancel_poll_allow.txt:
#       <path under vendor/pdf_oxide>::<fn>#<n>  # bounded: why
#     where <n> counts the loops of that function the patterns match, from 1.
#     `bounded:` is the only class an allow line may name; any other word,
#     `open:` included, fails.
#
# An allow line that matches no flagged loop fails too, so the file cannot
# outlive the code it excuses. `--headers` prints every loop header it sees,
# for writing patterns.
set -euo pipefail

cd "$(dirname "$0")/.."

[ -d vendor/pdf_oxide/src ] || exit 0

git -C vendor/pdf_oxide ls-files --cached --others --exclude-standard -z -- 'src/*.rs' \
  | python3 -I -c '
import re
import sys

ROOT = "vendor/pdf_oxide/"
PATTERNS_FILE = "tool/cancel_poll_patterns.txt"
ALLOW_FILE = "tool/cancel_poll_allow.txt"
# Functions that poll on the caller behalf, beside is_cancelled, .check(,
# .tick( and .each(.
HELPERS = ["poll", "poll_io"]
# Code outside the reach of the lanes: the language bindings, the command-line
# tools, OCR and the document builder, which no host op runs on an input.
SKIP = (
    "src/python.rs", "src/wasm.rs", "src/ffi.rs", "src/bin/", "src/ocr/", "src/api/", "src/writer/",
    # Primitives that work on the data of one page: the loop over the pages is
    # in their caller, which this lint does check.
    "src/extractors/", "src/structure/", "src/layout/", "src/elements/", "src/fonts/",
    "src/pipeline/", "src/converters/", "src/html_css/", "src/xfa/", "src/redaction/",
)
LIST_HEADERS = "--headers" in sys.argv[1:]


def read_lines(path):
    """The meaningful lines of a list file: (text, comment) pairs."""
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
    (newlines kept), so braces and keywords found in it are code."""
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


LOOP = re.compile(r"\b(for|while|loop)\b")
FN = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)")


def loops_of(text):
    """Every loop: (keyword offset, header, body start, body end)."""
    found = []
    for m in LOOP.finditer(text):
        a = m.start()
        before = text[:a].rstrip()
        prev = before[-1:] if before else ";"
        if prev == ">" and not before.endswith("=>"):
            continue
        if prev not in "{};:>,(" and before:
            # a label `\x27a: for` ends in `:`; anything else is not a statement start
            continue
        if m.group(1) == "for" and re.match(r"for\s*<", text[a:a + 8]):
            continue
        if m.group(1) == "loop" and not re.match(r"loop\s*\{", text[a:a + 12]):
            continue
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
        if end < 0:
            continue
        header = re.sub(r"\s+", " ", text[a:k]).strip()
        if m.group(1) == "loop":
            first = re.sub(r"\s+", " ", text[k + 1:end].split(";", 1)[0])[:160].strip()
            header = "loop " + first
        found.append((a, header, k, end))
    return found


def enclosing_fn(text, at, fns):
    best = None
    for start, name, body_start, body_end in fns:
        if body_start < at < body_end:
            best = (start, name, body_start, body_end)
    return best


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


if LIST_HEADERS:
    patterns, allow = [], {}
else:
    patterns = []
    for number, text, comment in read_lines(PATTERNS_FILE):
        if not comment:
            print(f"{PATTERNS_FILE}:{number}: pattern with no comment naming what it matches")
            sys.exit(1)
        patterns.append(re.compile(text))
    allow = {}
    for number, text, comment in read_lines(ALLOW_FILE):
        if not comment:
            print(f"{ALLOW_FILE}:{number}: allowed loop with no reason")
            sys.exit(1)
        if not comment.startswith("bounded: "):
            print(f"{ALLOW_FILE}:{number}: an allowed loop is `# bounded: <why>`; one that scales with the document polls")
            sys.exit(1)
        allow[text] = [number, False]

POLL = re.compile(
    r"is_cancelled|\.check\(|\.tick\(|\.each\(" + "".join(f"|\\b{h}\\(" for h in HELPERS)
)

bad = 0
for path in sys.stdin.buffer.read().split(b"\0"):
    if not path:
        continue
    path = path.decode()
    if path.startswith("src/host/engine_proofs/") or path.startswith(SKIP):
        continue
    try:
        with open(ROOT + path, encoding="utf-8") as f:
            src = f.read()
    except (UnicodeDecodeError, FileNotFoundError, IsADirectoryError):
        continue
    text = blank(src)
    skip = test_ranges(text)
    fns = functions_of(text)
    loops = [lp for lp in loops_of(text) if not any(a <= lp[0] < b for a, b in skip)]
    counts = {}
    for a, header, k, end in loops:
        line = text.count("\n", 0, a) + 1
        if LIST_HEADERS:
            print(f"{ROOT}{path}:{line}: {header}")
            continue
        if not any(p.search(header) for p in patterns):
            continue
        fn = enclosing_fn(text, a, fns)
        name = fn[1] if fn else "-"
        counts[name] = counts.get(name, 0) + 1
        key = f"{path}::{name}#{counts[name]}"
        if key in allow:
            allow[key][1] = True
            continue
        if POLL.search(text[k:end]):
            continue
        if any(oa < a and end < oe and POLL.search(text[ok:oe])
               for oa, _, ok, oe in loops if (oa, ok, oe) != (a, k, end) and fn and fn[2] < oa):
            continue
        print(f"{ROOT}{path}:{line}: loop over a document-scale collection never polls: {header[:100]}")
        print(f"    poll the cancel check at the top of the iteration, or allow it as {key}")
        bad += 1

for key, (number, used) in allow.items():
    if not used and not LIST_HEADERS:
        print(f"{ALLOW_FILE}:{number}: {key} matches no flagged loop")
        bad += 1

if bad:
    print(f"{bad} loop(s) without a cancel poll")
    sys.exit(1)
if not LIST_HEADERS:
    print("every loop over a document-scale collection polls for a cancel")
' "$@"
