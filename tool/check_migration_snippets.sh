#!/usr/bin/env bash
# Migration snippet wall: every Before and After block of docs/MIGRATION.md
# must compile against its own version of the package.
#
# Each entry's Before block is compiled against pdf_manipulator 5.0.1 from
# pub.dev, and its After block against this checkout. A block becomes the body
# of one function, `entry<N>`, whose parameters are the free names the guide's
# intro promises (`pdf`, `doc`, `editor`, ...), typed per version below. Any
# analyzer error fails the wall and names the entry and the side; warnings and
# lints do not, since a snippet is read, not run.
#
# Nothing is executed and no native build runs: `dart pub get` and
# `dart analyze` only.
set -euo pipefail

cd "$(dirname "$0")/.."
root=$PWD
guide=docs/MIGRATION.md
released=5.0.1

# The free names a snippet may use, with the types the version gives them.
# Change a row only when an entry needs a new free name.
before_params='Pdf pdf, DataSource source, DataSink output, PdfDoc doc, PdfEditor editor, PdfBuilder builder, PdfPageBuilder page, PdfRect rect, Uint8List bytes, List<DataSource> sources, List<DataSource> images, PdfFallbackFontKind kind'
after_params='Pdf pdf, DataSource source, DataSink output, PdfDocument doc, PdfEditor editor, PdfBuilder builder, PdfPageBuilder page, PdfRect rect, Uint8List bytes, List<DataSource> sources, List<DataSource> images, PdfFallbackFontKind kind'

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Writes lib/entries.dart for one side ($1: Before or After, $2: parameters).
# Prints a title for each entry that lacks that side's block, and fails then.
generate() {
  awk -v side="$1" -v params="$2" '
    function flush() { if (title != "" && !done) missing = missing title "\n" }
    BEGIN {
      print "import \"dart:io\";"
      print "import \"dart:typed_data\";"
      print "import \"package:pdf_manipulator/pdf_manipulator.dart\";"
      print "import \"package:pdf_manipulator/io.dart\";"
    }
    /^\*\*/ {
      flush()
      title = $0; sub(/^\*\*/, "", title); sub(/\*\*.*/, "", title)
      n++; done = 0; label = ""; next
    }
    /^(Before|After):$/ { label = substr($0, 1, length($0) - 1); next }
    /^```dart$/ && label != "" { inblock = 1; body = ""; next }
    /^```$/ && inblock {
      inblock = 0
      if (label == side) {
        printf "\n// %s\nFuture<void> entry%d(%s) async {\n%s}\n", title, n, params, body
        done = 1
      }
      label = ""; next
    }
    inblock { body = body $0 "\n" }
    END {
      flush()
      if (missing != "") { printf "%s", missing > "/dev/stderr"; exit 1 }
      if (n == 0) { print "no entries" > "/dev/stderr"; exit 1 }
    }
  ' "$guide" > "$3" || {
    echo "$guide: these entries have no $1 block:"
    exit 1
  }
}

# Makes a project for one side ($1: before or after, $2: dependency lines).
project() {
  mkdir -p "$tmp/$1/lib"
  printf 'name: snippets_%s\nenvironment:\n  sdk: ">=3.10.0 <4.0.0"\ndependencies:\n  pdf_manipulator:%s\n' \
    "$1" "$2" > "$tmp/$1/pubspec.yaml"
}

project before " $released"
project after "
    path: $root"
generate Before "$before_params" "$tmp/before/lib/entries.dart"
generate After "$after_params" "$tmp/after/lib/entries.dart"

bad=0
for side in before after; do
  label=Before
  [ "$side" = after ] && label=After
  (cd "$tmp/$side" && dart pub get >/dev/null)
  # Machine format: SEVERITY|TYPE|CODE|FILE|LINE|COLUMN|LENGTH|MESSAGE
  found=$(cd "$tmp/$side" && dart analyze --format=machine 2>&1 || true)
  while IFS='|' read -r severity _ _ _ line _ _ message; do
    [ "$severity" = ERROR ] || continue
    title=$(awk -v at="$line" 'NR <= at && /^\/\/ /{t = substr($0, 4)} END{print t}' \
      "$tmp/$side/lib/entries.dart")
    code=$(sed -n "${line}p" "$tmp/$side/lib/entries.dart" | sed 's/^ *//')
    echo "$guide: $label of ${title:-?}: $message [$code]"
    bad=1
  done <<< "$found"
done

if [ "$bad" -ne 0 ]; then
  echo
  echo "Each Before block must compile against pdf_manipulator $released and each"
  echo "After block against this checkout. Fix the snippet, not the wall."
  exit 1
fi
echo "every migration snippet compiles against its version"
