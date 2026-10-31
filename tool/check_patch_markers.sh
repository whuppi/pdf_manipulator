#!/usr/bin/env bash
# Patch marker wall (docs/UPDATING.md): the markers are the full list of what
# the fork changed in upstream code, so they must pair.
#
# Walks every tracked or new file of vendor/pdf_oxide outside src/host/ (which
# is fork-only and carries no markers) and fails with file:line on:
#   - an end marker with no open marker before it
#   - an open marker never closed
#   - an open marker inside an open one (blocks do not nest)
#   - a line that mentions the marker phrase but is neither exact form (a
#     typo'd marker, a doc comment or a trailing comment that quotes it)
#
# An open marker is a `//` or `#` comment line that starts with
# `── pdf_manipulator patch`, followed by `:` or ` ──`; a close marker is the
# whole line `── end pdf_manipulator patch ──` behind the same prefix. In
# Markdown the same two lines sit inside an HTML comment, `<!-- … -->`.
set -euo pipefail

cd "$(dirname "$0")/.."

dir=vendor/pdf_oxide
[ -d "$dir" ] || exit 0

git -C "$dir" ls-files --cached --others --exclude-standard -z \
  | python3 -I -c '
import re
import sys

OPEN = re.compile(r"^\s*((//|#)[ \t]*── pdf_manipulator patch(:| ──|$)|<!--[ \t]*── pdf_manipulator patch(:[^>]*)? ──[ \t]*-->\s*$)")
CLOSE = re.compile(r"^\s*((//|#)[ \t]*── end pdf_manipulator patch ──|<!--[ \t]*── end pdf_manipulator patch ──[ \t]*-->)\s*$")
PHRASE = "pdf_manipulator patch"
ROOT = "vendor/pdf_oxide/"

bad = 0


def fail(path, line, why):
    global bad
    print(f"{ROOT}{path}:{line}: {why}")
    bad += 1


for path in sys.stdin.buffer.read().split(b"\0"):
    if not path:
        continue
    path = path.decode()
    if path.startswith("src/host/"):
        continue
    try:
        with open(ROOT + path, encoding="utf-8") as f:
            lines = f.read().split("\n")
    except (UnicodeDecodeError, FileNotFoundError, IsADirectoryError):
        continue
    stack = []
    for number, text in enumerate(lines, 1):
        if PHRASE not in text:
            continue
        if CLOSE.match(text):
            if not stack:
                fail(path, number, "end marker with no open marker")
            else:
                stack.pop()
        elif OPEN.match(text):
            if stack:
                fail(path, number, f"open marker inside the block opened at line {stack[-1]}")
            stack.append(number)
        else:
            fail(path, number, "line mentions the marker phrase but is neither marker form")
    for number in stack:
        fail(path, number, "open marker never closed")

if bad:
    print(f"{bad} patch marker violation(s)")
    sys.exit(1)
'
