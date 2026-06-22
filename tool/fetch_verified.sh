#!/usr/bin/env bash
# ────────────────────────────────────────────────────────────────────
# fetch_verified.sh — the ONLY door for downloading a third-party binary.
#
# Fail-closed: refuses unless the bytes match the pinned sha256. No
# "verify if a hash exists" path — a missing/placeholder hash is itself a
# refusal. TLS protects the wire; this protects against a repointed tag,
# a compromised release, or a CDN swap.
#
# Pinned hashes live in tool/versions.env (single source). Callers source
# that, then pass the right per-platform value in.
#
# Usage:  fetch_verified.sh <url> <sha256> <dest>
#         (<sha256> may carry an optional "sha256:" prefix)
# Exit:   0 = downloaded + verified   1 = download or hash failure
#         2 = no pinned hash supplied (fail-closed)
# ────────────────────────────────────────────────────────────────────
set -uo pipefail

URL="${1:?usage: fetch_verified.sh <url> <sha256> <dest>}"
DEST="${3:?usage: fetch_verified.sh <url> <sha256> <dest>}"
# An empty/unset hash must hit the fail-closed refusal below (exit 2),
# not a usage error — a missing pin is the exact case to refuse loudly.
WANT="${2-}"
WANT="${WANT#sha256:}"

case "$WANT" in
  ""|"TODO"|"unknown"|"none")
    echo "refusing to download $URL — no pinned sha256 (fail-closed)." >&2
    exit 2
    ;;
esac

curl -sSL --fail --retry 3 --retry-delay 2 --max-redirs 5 --connect-timeout 10 --max-time 180 "$URL" -o "$DEST" \
  || { echo "download failed: $URL" >&2; exit 1; }

if command -v sha256sum >/dev/null 2>&1; then
  GOT=$(sha256sum "$DEST" | awk '{print $1}')
else
  GOT=$(shasum -a 256 "$DEST" | awk '{print $1}')
fi

if [ "$GOT" != "$WANT" ]; then
  echo "HASH MISMATCH — refusing $URL" >&2
  echo "  expected $WANT" >&2
  echo "  got      $GOT" >&2
  rm -f "$DEST"
  exit 1
fi
echo "✓ verified $(basename "$DEST") ($GOT)"
