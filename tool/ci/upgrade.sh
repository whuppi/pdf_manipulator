#!/usr/bin/env bash
set -euo pipefail
#
# Upgrade radar — the ONE place that watches every pinned version this repo
# owns and can bump automatically. upgrade-check.yml runs `apply` daily and
# opens a single reviewed PR with whatever drifted.
#
# Watched here (this package's LOCAL pins — clean upstream, real churn):
#   binaryen      BINARYEN_VERSION + 3 sha256    WebAssembly/binaryen
#   pana          PANA_VERSION                   pub.dev
#
# Owned elsewhere by design (NOT here):
#   Flutter SDK (.fvmrc), lockfiles  the shared upgrade-check reusable workflow
#   fvm / Chrome / bore pins,        whuppi/ci (its self-upgrade bumps them;
#     zizmor / actionlint gate pins    they reach this repo via the whuppi-ci
#                                      Dependabot group after a whuppi/ci release)
#   pub deps, GitHub-action SHAs   Dependabot (.github/dependabot.yml)
#   build.json (baseTag, crate,    manual — pdf_oxide source facts; move only
#     features, outputs)             when the vendored submodule does
#
# JSON is parsed with jq — gh's embedded `--jq` for gh-api responses,
# ensure_jq + jq for curl'd manifests. In-place version bumps stay targeted
# sed (to preserve each file's formatting). No python, no dart.
#
# Usage:  tool/ci/upgrade.sh check              # report drift, write nothing
#         tool/ci/upgrade.sh apply              # rewrite the pinned files in place
#         tool/ci/upgrade.sh verify-pinned      # re-hash the pins, flag a repoint
#         tool/ci/upgrade.sh check-availability # HEAD the pins, flag a prune

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"  # tool/ci/ → repo root
VERSIONS="$ROOT/tool/versions.env"

# shellcheck source=/dev/null  # runtime path; not followed at lint time
source "$VERSIONS"
source "$ROOT/tool/lib.sh"

MODE="${1:-check}"
case "$MODE" in
  check|apply|verify-pinned|check-availability) ;;
  *) echo "usage: tool/ci/upgrade.sh [check|apply|verify-pinned|check-availability]" >&2; exit 2 ;;
esac

ensure_jq

drift=0
blocked=0   # a bump whose asset fetch failed sets this; the run then exits
            # nonzero at the end instead of passing silently.

gh_latest_tag() {  # owner/repo -> latest release tag, verbatim
  gh api "repos/$1/releases/latest" --jq '.tag_name' 2>/dev/null || true
}

# Hardened GET for the fetches below: fail-closed, capped redirects, and a
# few retries so a transient blip doesn't fail the daily run.
_fetch() { curl -fsSL --retry 3 --retry-delay 2 --max-redirs 5 --connect-timeout 10 --max-time 30 "$@"; }

# sha256 of a downloadable asset, or empty on failure. Downloads to a file
# (never a shell var) so binary content survives intact and a 404 can't
# masquerade as the empty-string hash.
sha256_of() {  # url
  local tmp; tmp="$(mktemp)"
  if _fetch "$1" -o "$tmp" 2>/dev/null && [ -s "$tmp" ]; then
    sha256_file "$tmp"
  fi
  rm -f "$tmp"
}

set_kv() {  # KEY value file — replace the KEY="old" line with KEY="new"
  # versions.env is sourced, so a value is executed on read. Validate the shape
  # before writing (version or 64-hex sha) so a malformed upstream string can't
  # be persisted and run.
  case "$1" in
    *SHA256*)  printf '%s' "$2" | grep -qE '^[0-9a-f]{64}$' \
                 || { echo "set_kv: refusing non-sha256 for $1: '$2'" >&2; return 1; } ;;
    *VERSION*) printf '%s' "$2" | grep -qE '^[A-Za-z0-9._+-]+$' \
                 || { echo "set_kv: refusing malformed version for $1: '$2'" >&2; return 1; } ;;
    *)         echo "set_kv: unknown key shape '$1' (expect *_VERSION or *_SHA256_*)" >&2; return 1 ;;
  esac
  # The value travels via the environment — not a sed replacement, not awk -v —
  # so a |, &, or backslash in it stays literal data (sed's replacement string
  # and awk's -v both interpret those). tmp+mv leaves the file intact if awk
  # ever fails mid-write.
  local tmp="$3.tmp"
  if sk_key="$1" sk_val="$2" awk '
        BEGIN { k = ENVIRON["sk_key"]; v = ENVIRON["sk_val"] }
        $0 ~ "^" k "=" { print k "=\"" v "\""; next }
        { print }
      ' "$3" > "$tmp"; then
    mv "$tmp" "$3"
  else
    rm -f "$tmp"
    return 1
  fi
}

# Every pinned asset as tool<TAB>platform<TAB>url<TAB>sha256. Single source for
# the checks below; mirrors the bump blocks, so keep the two in sync.
asset_urls() {
  local bu="https://github.com/WebAssembly/binaryen/releases/download/$BINARYEN_VERSION"
  printf 'binaryen\tlinux-x64\t%s\t%s\n'   "$bu/binaryen-$BINARYEN_VERSION-x86_64-linux.tar.gz"   "$BINARYEN_SHA256_LINUX_X64"
  printf 'binaryen\tmacos-arm64\t%s\t%s\n' "$bu/binaryen-$BINARYEN_VERSION-arm64-macos.tar.gz"    "$BINARYEN_SHA256_MACOS_ARM64"
  printf 'binaryen\twindows-x64\t%s\t%s\n' "$bu/binaryen-$BINARYEN_VERSION-x86_64-windows.tar.gz" "$BINARYEN_SHA256_WINDOWS_X64"
}

# HTTP status of a URL, following GitHub's asset redirect, or 000 if unreachable.
# Always exits 0 so a caller's $(...) never trips set -e.
http_status() {  # url -> code
  local code
  code="$(curl -sS -o /dev/null -IL --connect-timeout 10 --retry 2 --retry-delay 2 \
          --max-redirs 5 -w '%{http_code}' "$1" 2>/dev/null || true)"
  printf '%s' "${code:-000}"
}

# Daily integrity: a 404/410 means the pin was pruned, a 200 with a changed hash
# means a same-version repoint. A transient code warns rather than failing.
verify_pinned() {
  local t=0 tool plat url want code got
  while IFS=$'\t' read -r tool plat url want; do
    code="$(http_status "$url")"
    case "$code" in
      404|410) echo "::error::verify-pinned: $tool $plat pruned upstream ($code), re-pin: $url" >&2; t=1; continue ;;
      200) ;;
      *) echo "::warning::verify-pinned: $tool $plat HEAD returned $code (transient?): $url" >&2; continue ;;
    esac
    got="$(sha256_of "$url")"
    if [ -z "$got" ]; then
      echo "::warning::verify-pinned: $tool $plat 200 but empty body (transient?): $url" >&2
    elif [ "$got" != "$want" ]; then
      echo "::error::verify-pinned: $tool $plat REPOINT, pinned $want now $got: $url" >&2; t=1
    fi
  done < <(asset_urls)
  [ "$t" -eq 0 ] && { echo "verify-pinned: all pinned assets present and matching."; return 0; }
  echo "::error::verify-pinned: a pinned asset is gone or changed; do NOT bump, investigate" >&2
  return 1
}

# PR hot-path existence check, HEAD only (no download or hash). Fails on a
# definitive 404/410, warns on transient. Runs on PR activity, so a disabled
# daily cron can't hide a pruned pin until the build breaks.
check_availability() {
  local bad=0 tool plat url _sha code
  while IFS=$'\t' read -r tool plat url _sha; do
    code="$(http_status "$url")"
    case "$code" in
      200) ;;
      404|410) echo "::error::pin unavailable: $tool $plat gone upstream ($code): $url" >&2; bad=1 ;;
      *) echo "::warning::pin check: $tool $plat HEAD returned $code (transient?): $url" >&2 ;;
    esac
  done < <(asset_urls)
  [ "$bad" -eq 0 ] && echo "check-availability: all pinned assets reachable."
  return "$bad"
}

# Each check runs alone, then exits.
if [ "$MODE" = verify-pinned ]; then verify_pinned && exit 0; exit 1; fi
if [ "$MODE" = check-availability ]; then check_availability && exit 0; exit 1; fi

# ── pana (PANA_VERSION in versions.env; the `platforms` make target + CI gate).
# Track pub.dev's LATEST from its own API: the gate must run the same pana
# pub.dev runs, or it drifts from the platform verdict it exists to predict.
pana_latest="$(_fetch https://pub.dev/api/packages/pana 2>/dev/null | jq -r '.latest.version // empty' 2>/dev/null || true)"
if [ -n "$PANA_VERSION" ] && [ -n "$pana_latest" ] && [ "$PANA_VERSION" != "$pana_latest" ]; then
  drift=1
  echo "pana: $PANA_VERSION -> $pana_latest"
  [ "$MODE" = apply ] && set_kv PANA_VERSION "$pana_latest" "$VERSIONS"
fi

# ── binaryen (version + 3 verified sha256, all in versions.env) ──────
# Detection is a cheap tag compare; the downloads run only on apply, and any
# single 404 aborts the whole bump rather than writing a half-correct pin.
bin_latest="$(gh_latest_tag WebAssembly/binaryen)"
if [ -n "$bin_latest" ] && [ "$bin_latest" != "$BINARYEN_VERSION" ]; then
  if [ "$MODE" = apply ]; then
    u="https://github.com/WebAssembly/binaryen/releases/download/$bin_latest"
    lx="$(sha256_of "$u/binaryen-$bin_latest-x86_64-linux.tar.gz")"
    mac="$(sha256_of "$u/binaryen-$bin_latest-arm64-macos.tar.gz")"
    win="$(sha256_of "$u/binaryen-$bin_latest-x86_64-windows.tar.gz")"
    if [ -n "$lx" ] && [ -n "$mac" ] && [ -n "$win" ]; then
      drift=1; echo "binaryen: $BINARYEN_VERSION -> $bin_latest (+ 3 sha256)"
      set_kv BINARYEN_VERSION "$bin_latest" "$VERSIONS"
      set_kv BINARYEN_SHA256_LINUX_X64 "$lx" "$VERSIONS"
      set_kv BINARYEN_SHA256_MACOS_ARM64 "$mac" "$VERSIONS"
      set_kv BINARYEN_SHA256_WINDOWS_X64 "$win" "$VERSIONS"
    else
      echo "::error::binaryen: $bin_latest available but an asset download failed; bump by hand"; blocked=1
    fi
  else
    drift=1; echo "binaryen: $BINARYEN_VERSION -> $bin_latest (apply fetches + verifies 3 sha256)"
  fi
fi

[ "$drift" -eq 0 ] && echo "All watched pins are current."
if [ "$blocked" -ne 0 ]; then
  echo "::error::one or more pins are blocked (upstream had a new version but its asset fetch failed); fix by hand" >&2
  exit 1
fi
exit 0
