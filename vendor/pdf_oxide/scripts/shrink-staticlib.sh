#!/usr/bin/env bash
# Shrink a Rust-produced staticlib (.a / .lib) by removing sections that are
# useless to non-Rust downstream consumers.
#
# Rust staticlibs embed per-object `.llvmbc` + `.llvmcmd` (LLVM bitcode for
# cross-crate LTO) and DWARF `.debug_*` sections. Neither is used by:
#   - CGo's linker (Go staticlib consumer)
#   - node-gyp / gyp-ng (Node.js addon staticlib consumer)
#   - MSVC's LINK.EXE in default mode (C# NuGet path uses cdylib anyway)
#
# Empirically on this repo: 35 MB `.llvmbc` + 4 MB DWARF per Linux x64 .a out
# of 71 MB total. Removing both halves the committed Go lib payload.
#
# Usage: shrink-staticlib.sh <path-to-staticlib>
# Platform detection is automatic; on macOS we use `strip -S` (DWARF-only) and
# skip bitcode since Mach-O uses `__LLVM,__bitcode` which `strip` ignores.

set -euo pipefail

LIB="${1:?path to .a / .lib required}"

if [[ ! -f "$LIB" ]]; then
  echo "shrink-staticlib: file not found: $LIB" >&2
  exit 1
fi

before=$(wc -c < "$LIB")

case "$(uname -s)" in
  Linux|MINGW*|MSYS*|CYGWIN*)
    # GNU binutils path. objcopy handles archives: it iterates members and
    # applies the operation to each, then rewrites the archive in place.
    if command -v objcopy >/dev/null 2>&1; then
      # Split-debug `.dwo` archive members (emitted by the mingw cross-compile
      # toolchain on x86_64-pc-windows-gnu) contain *only* DWARF sections.
      # `objcopy --strip-debug` removes their only sections and then aborts
      # the whole archive with "has no sections". Drop these members first so
      # objcopy has nothing debug-only left to process.
      if command -v ar >/dev/null 2>&1; then
        mapfile -t dwo_members < <(ar t "$LIB" 2>/dev/null | grep -E '\.dwo$' || true)
        if [[ ${#dwo_members[@]} -gt 0 ]]; then
          for m in "${dwo_members[@]}"; do
            ar d "$LIB" "$m" || true
          done
        fi
      fi
      # llvm-objcopy rejects "same input and output" on some distros; write to
      # a sibling tmp file and move it into place atomically.
      tmp="${LIB}.shrink.tmp"
      objcopy \
        --remove-section=.llvmbc \
        --remove-section=.llvmcmd \
        --strip-debug \
        "$LIB" "$tmp"
      mv "$tmp" "$LIB"
    else
      echo "shrink-staticlib: objcopy not available; skipping $LIB" >&2
    fi
    ;;
  Darwin)
    # macOS `strip -S` removes DWARF only. Bitcode in Mach-O lives in the
    # `__LLVM,__bitcode` segment; on non-bitcode-framework archives `strip`
    # leaves it alone, but modern Rust builds (1.74+) don't emit Mach-O
    # bitcode segments by default for staticlib outputs, so the debug-only
    # strip is the expected full win here.
    strip -S "$LIB"
    ;;
  *)
    echo "shrink-staticlib: unknown OS $(uname -s); skipping" >&2
    ;;
esac

after=$(wc -c < "$LIB")
saved=$((before - after))
pct=$(awk -v b="$before" -v a="$after" 'BEGIN{ if(b>0) printf "%.1f", (b-a)*100/b; else print "0.0" }')
echo "shrink-staticlib: $LIB  ${before} -> ${after} bytes  (saved ${saved}, ${pct}%)"
