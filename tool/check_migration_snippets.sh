#!/usr/bin/env bash
# Migration snippet wall: every Before and After block of docs/MIGRATION.md
# must compile against its own version of the package.
#
# The guide has one `## From X to Y` section per migration path. In each, an
# entry's Before block is compiled against pdf_manipulator X from pub.dev, and
# its After block against Y: this checkout when Y is the version the changelogs
# are on, else Y from pub.dev. A block becomes the body of one function,
# `entry<N>`, whose parameters are the free names the section's intro promises
# (`pdf`, `doc`, `editor`, ...), typed per version below. Any analyzer error
# fails the wall and names the section, the entry and the side; warnings and
# lints do not, since a snippet is read, not run. A block in another language
# (a pubspec, a file list) counts as the entry's side and is not compiled.
#
# Nothing is executed and no native build runs: `pub get` and `dart analyze`
# only. A version that needs Flutter (0.5.9 is a Flutter plugin) resolves with
# $FLUTTER instead of $DART.
set -euo pipefail

cd "$(dirname "$0")/.."
root=$PWD
guide=docs/MIGRATION.md
: "${DART:=dart}"
: "${FLUTTER:=flutter}"

# The version this checkout is: the newest `## X.Y.Z` heading of either
# changelog, a prerelease suffix dropped.
current=$(sed -nE 's/^## ([0-9]+\.[0-9]+\.[0-9]+)(-dev\.[0-9]+)?$/\1/p' CHANGELOG.pre.md CHANGELOG.md \
  | sort -t. -k1,1n -k2,2n -k3,3n | tail -1)

# What a snippet sees for one version: its imports, its free names, and
# whether resolving it needs Flutter. Add a row when a section names a new
# version; change a row only when an entry needs a new free name.
config() {
  local io='import "package:pdf_manipulator/io.dart";'
  local api='import "package:pdf_manipulator/pdf_manipulator.dart";'
  case "$1" in
    1.0.6|2.0.0)
      imports="$api"
      params='Pdf pdf, DataSource source, DataSink output, PdfDoc doc, PdfEditor editor, Uint8List bytes'
      flutter=0 ;;
    2.1.4|3.0.0|4.0.0|4.2.0)
      imports="$api"$'\n'"$io"
      params='Pdf pdf, DataSource source, DataSink output, PdfDoc doc, PdfEditor editor, Uint8List bytes'
      flutter=0 ;;
    0.5.9)
      imports="$api"$'\n''import "package:flutter/services.dart";'
      params='PdfManipulator plugin, String path, String pathA, String pathB, List<String> imagePaths'
      flutter=1 ;;
    5.0.0|5.0.1)
      imports="$api"$'\n'"$io"
      params='Pdf pdf, DataSource source, DataSink output, PdfDoc doc, PdfEditor editor, PdfBuilder builder, PdfPageBuilder page, PdfRect rect, Uint8List bytes, List<DataSource> sources, List<DataSource> images, PdfFallbackFontKind kind'
      flutter=0 ;;
    6.0.0)
      imports="$api"$'\n'"$io"
      params='Pdf pdf, DataSource source, DataSink output, PdfDocument doc, PdfEditor editor, PdfBuilder builder, PdfPageBuilder page, PdfRect rect, Uint8List bytes, List<DataSource> sources, List<DataSource> images, PdfFallbackFontKind kind, String path, String pathA, String pathB, List<String> imagePaths'
      flutter=0 ;;
    *)
      echo "$guide: no snippet configuration for version $1; add a row to config() in $0"
      exit 1 ;;
  esac
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Writes $4/lib/entries.dart: the $1 (Before or After) blocks of the section
# headed $2, with parameters $3 and imports $5. Fails, naming them, when entries lack that
# side's block.
generate() {
  IMPORTS="$5" awk -v side="$1" -v heading="$2" -v params="$3" '
    function flush() { if (title != "" && !done) missing = missing title "\n" }
    BEGIN {
      print "import \"dart:io\";"
      print "import \"dart:typed_data\";"
      print ENVIRON["IMPORTS"]
    }
    /^## / { flush(); title = ""; on = ($0 == heading); next }
    !on { next }
    /^\*\*/ {
      flush()
      title = $0; sub(/^\*\*/, "", title); sub(/\*\*.*/, "", title)
      n++; done = 0; label = ""; next
    }
    /^(Before|After):$/ { label = substr($0, 1, length($0) - 1); next }
    /^```[a-z]+$/ && label != "" { inblock = 1; lang = substr($0, 4); body = ""; next }
    /^```$/ && inblock {
      inblock = 0
      if (label == side) {
        if (lang == "dart") printf "\n// %s\nFuture<void> entry%d(%s) async {\n%s}\n", title, n, params, body
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
  ' "$guide" > "$4/lib/entries.dart" || {
    echo "$guide ($2): these entries have no $1 block:"
    exit 1
  }
}

# Makes a project in $1 that depends on pdf_manipulator $2 (a version, or
# `path` for this checkout), with Flutter when $3 is 1.
project() {
  mkdir -p "$1/lib"
  local dep=" $2"
  [ "$2" = path ] && dep=$'\n'"    path: $root"
  local sdk='">=3.10.0 <4.0.0"'
  if [ "$3" = 1 ]; then
    printf 'name: snippets\nenvironment:\n  sdk: %s\ndependencies:\n  flutter:\n    sdk: flutter\n  pdf_manipulator:%s\n' "$sdk" "$dep" > "$1/pubspec.yaml"
  else
    printf 'name: snippets\nenvironment:\n  sdk: %s\ndependencies:\n  pdf_manipulator:%s\n' "$sdk" "$dep" > "$1/pubspec.yaml"
  fi
}

bad=0
sections=0
while IFS= read -r heading; do
  from=$(printf '%s' "$heading" | sed -nE 's/^## From ([0-9.]+) to [0-9.]+$/\1/p')
  to=$(printf '%s' "$heading" | sed -nE 's/^## From [0-9.]+ to ([0-9.]+)$/\1/p')
  if [ -z "$from" ] || [ -z "$to" ]; then
    echo "$guide: '$heading' is not a 'From X to Y' section"
    bad=1
    continue
  fi
  sections=$((sections + 1))
  for side in Before After; do
    version=$from
    [ "$side" = After ] && version=$to
    config "$version"
    dir="$tmp/$sections-$side"
    dep=$version
    [ "$version" = "$current" ] && dep=path
    project "$dir" "$dep" "$flutter"
    generate "$side" "$heading" "$params" "$dir" "$imports"
    if [ "$flutter" = 1 ]; then
      (cd "$dir" && $FLUTTER pub get >/dev/null)
    else
      (cd "$dir" && $DART pub get >/dev/null)
    fi
    # Machine format: SEVERITY|TYPE|CODE|FILE|LINE|COLUMN|LENGTH|MESSAGE
    found=$(cd "$dir" && $DART analyze --format=machine 2>&1 || true)
    while IFS='|' read -r severity _ _ _ line _ _ message; do
      [ "$severity" = ERROR ] || continue
      title=$(awk -v at="$line" 'NR <= at && /^\/\/ /{t = substr($0, 4)} END{print t}' "$dir/lib/entries.dart")
      code=$(sed -n "${line}p" "$dir/lib/entries.dart" | sed 's/^ *//')
      echo "$guide (${heading#\#\# }): $side of ${title:-?}, against $version: $message [$code]"
      bad=1
    done <<< "$found"
  done
done < <(grep '^## ' "$guide")

if [ "$sections" -eq 0 ]; then
  echo "$guide: no 'From X to Y' section"
  bad=1
fi
if [ "$bad" -ne 0 ]; then
  echo
  echo "Each Before block must compile against the version its section migrates"
  echo "from, and each After block against the version it migrates to. Fix the"
  echo "snippet, not the wall."
  exit 1
fi
echo "every migration snippet compiles against its version"
