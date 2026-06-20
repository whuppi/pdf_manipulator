#!/bin/bash
# ============================================================================
# pdf_manipulator secrets manager
#
# Manages secrets for whuppi/pdf_manipulator ONLY.
# Bitwarden Secrets Manager is the source of truth.
# GitHub Environments are CI-accessible copies.
#
# Usage:
#   ./secrets.sh set   <env>/<KEY> <value>   Store in Bitwarden + push to GitHub
#   ./secrets.sh get   <env>/<KEY>           Read from Bitwarden
#   ./secrets.sh list  <env>                 List secret names for an environment
#   ./secrets.sh dump  <env>                 Show values (careful!)
#   ./secrets.sh upload <env|all>            Push all from Bitwarden → GitHub
#   ./secrets.sh rm    <env>/<KEY>           Delete from Bitwarden + GitHub
# ============================================================================
set -e

REPO="whuppi/pdf_manipulator"
BWS="${HOME}/bin/bws"
BW_PREFIX="pdf_manipulator"
export BWS_SERVER_URL="${BWS_SERVER_URL:-https://vault.bitwarden.eu}"

# ── Auth ─────────────────────────────────────────────────────────────────────

if [ -z "$BWS_ACCESS_TOKEN" ]; then
    BWS_ACCESS_TOKEN=$(security find-generic-password -a "whuppi" -s "BWS_ACCESS_TOKEN" -w 2>/dev/null || true)
    export BWS_ACCESS_TOKEN
fi

if [ -z "$BWS_ACCESS_TOKEN" ]; then
    echo "Not authenticated. Run: bws-auth"
    exit 1
fi

if ! $BWS project list > /dev/null 2>&1; then
    echo "Bitwarden auth failed. Run: bws-auth"
    exit 1
fi

# ── Project ID ───────────────────────────────────────────────────────────────

PROJECT_ID=$($BWS project list 2>/dev/null | python3 -c "
import sys,json
for p in json.load(sys.stdin):
    if p['name'] == 'whuppi-infra':
        print(p['id'])
        break
")

# ── Helpers ──────────────────────────────────────────────────────────────────

bws_get() {
    $BWS secret list 2>/dev/null | python3 -c "
import sys,json
for s in json.load(sys.stdin):
    if s['key'] == '$1':
        print(s['value'])
        sys.exit(0)
" 2>/dev/null
}

bws_get_id() {
    $BWS secret list 2>/dev/null | python3 -c "
import sys,json
for s in json.load(sys.stdin):
    if s['key'] == '$1':
        print(s['id'])
        sys.exit(0)
" 2>/dev/null
}

bws_set() {
    local key="$1" value="$2"
    local existing_id
    existing_id=$(bws_get_id "$key")
    if [ -n "$existing_id" ]; then
        $BWS secret edit "$existing_id" --key "$key" --value "$value" > /dev/null 2>&1
    else
        $BWS secret create "$key" "$value" "$PROJECT_ID" > /dev/null 2>&1
    fi
}

validate_env() {
    case "$1" in
        dev|prod) return 0 ;;
        *) echo "Invalid env: $1 (use dev or prod)"; exit 1 ;;
    esac
}

# ── Commands ─────────────────────────────────────────────────────────────────

cmd_set() {
    local path="$1" value="$2"
    if [ -z "$path" ] || [ -z "$value" ]; then
        echo "Usage: ./secrets.sh set <env>/<KEY> <value>"
        echo "Example: ./secrets.sh set prod/PUB_CREDENTIALS '{...}'"
        exit 1
    fi

    local env key
    env=$(echo "$path" | cut -d/ -f1)
    key=$(echo "$path" | cut -d/ -f2-)
    validate_env "$env"

    local bw_key="$BW_PREFIX/$env/$key"
    bws_set "$bw_key" "$value"
    echo "✓ Bitwarden: $bw_key"

    local ghn
    ghn="$key"
    gh secret set "$ghn" --env "$env" --body "$value" --repo "$REPO"
    echo "✓ GitHub:    $REPO → $env → $ghn"
}

# Deletes from both backends, scoped to THIS repo only: GitHub by --repo/--env,
# Bitwarden by the "$BW_PREFIX/$env/" key prefix — it can't reach another repo's
# secrets. Idempotent: a missing secret is reported, not an error.
cmd_rm() {
    local path="$1"
    if [ -z "$path" ]; then
        echo "Usage: ./secrets.sh rm <env>/<KEY>"
        echo "Deletes the secret from Bitwarden AND this repo's GitHub environment."
        exit 1
    fi

    local env key
    env=$(echo "$path" | cut -d/ -f1)
    key=$(echo "$path" | cut -d/ -f2-)
    validate_env "$env"

    local ghn
    ghn="$key"
    if gh secret list --env "$env" --repo "$REPO" 2>/dev/null | grep -qE "^${ghn}[[:space:]]"; then
        if gh secret delete "$ghn" --env "$env" --repo "$REPO" 2>/dev/null; then
            echo "✓ GitHub:    $REPO → $env → $ghn (deleted)"
        else
            echo "✗ GitHub:    $REPO → $env → $ghn (delete FAILED — still present)"
            exit 1
        fi
    else
        echo "· GitHub:    $REPO → $env → $ghn (not present)"
    fi

    local bw_key="$BW_PREFIX/$env/$key"
    local id
    id=$(bws_get_id "$bw_key")
    if [ -n "$id" ]; then
        if $BWS secret delete "$id" > /dev/null 2>&1; then
            echo "✓ Bitwarden: $bw_key (deleted)"
        else
            echo "✗ Bitwarden: $bw_key (delete failed)"
            exit 1
        fi
    else
        echo "· Bitwarden: $bw_key (not present)"
    fi
}

cmd_get() {
    local path="$1"
    if [ -z "$path" ]; then
        echo "Usage: ./secrets.sh get <env>/<KEY>"
        exit 1
    fi

    local env key
    env=$(echo "$path" | cut -d/ -f1)
    key=$(echo "$path" | cut -d/ -f2-)
    validate_env "$env"

    local value
    value=$(bws_get "$BW_PREFIX/$env/$key")
    if [ -n "$value" ]; then
        echo "$value"
    else
        echo "(not found: $BW_PREFIX/$env/$key)"
        exit 1
    fi
}

cmd_list() {
    local env="$1"
    if [ -z "$env" ]; then
        echo "Usage: ./secrets.sh list <env>"
        exit 1
    fi
    validate_env "$env"

    $BWS secret list 2>/dev/null | python3 -c "
import sys,json
secrets = json.load(sys.stdin)
prefix = '$BW_PREFIX/$env/'
matched = sorted([s['key'].replace(prefix, '') for s in secrets if s['key'].startswith(prefix)])
if not matched:
    print('No secrets found for: $BW_PREFIX/$env')
else:
    for name in matched:
        print(f'  {name}')
    print(f'\n  {len(matched)} secrets')
"
}

cmd_dump() {
    local env="$1"
    if [ -z "$env" ]; then
        echo "Usage: ./secrets.sh dump <env>"
        echo "⚠️  Shows secret VALUES."
        exit 1
    fi
    validate_env "$env"

    $BWS secret list 2>/dev/null | python3 -c "
import sys,json
secrets = json.load(sys.stdin)
prefix = '$BW_PREFIX/$env/'
matched = sorted([(s['key'].replace(prefix, ''), s['value']) for s in secrets if s['key'].startswith(prefix)])
if not matched:
    print('No secrets found for: $BW_PREFIX/$env')
else:
    for name, value in matched:
        display = value if len(value) < 80 else value[:40] + '...[truncated]'
        print(f'  {name}={display}')
"
}

cmd_upload() {
    local target="$1"
    if [ -z "$target" ]; then
        echo "Usage: ./secrets.sh upload <env|all>"
        exit 1
    fi

    if [ "$target" = "all" ]; then
        upload_env "dev"
        upload_env "prod"
    else
        validate_env "$target"
        upload_env "$target"
    fi

    echo ""
    echo "=== Upload complete ==="
}

upload_env() {
    local env="$1"
    echo ""
    echo "=== $BW_PREFIX/$env → GitHub ==="

    gh api "repos/$REPO/environments/$env" -X PUT > /dev/null 2>&1

    $BWS secret list 2>/dev/null | python3 -c "
import sys,json
secrets = json.load(sys.stdin)
prefix = '$BW_PREFIX/$env/'
for s in secrets:
    if s['key'].startswith(prefix):
        name = s['key'].replace(prefix, '')
        print(f'{name}\t{s[\"value\"]}')
" | while IFS=$'\t' read -r key value; do
        local ghn
        ghn="$key"
        if gh secret set "$ghn" --env "$env" --body "$value" --repo "$REPO" 2>/dev/null; then
            echo "  ✓ $ghn"
        else
            echo "  ✗ $ghn (failed)"
        fi
    done

    echo "Done: $env"
}

# ── Dispatch ─────────────────────────────────────────────────────────────────

case "${1:-}" in
    set)    cmd_set "$2" "$3" ;;
    get)    cmd_get "$2" ;;
    list)   cmd_list "$2" ;;
    dump)   cmd_dump "$2" ;;
    upload) cmd_upload "$2" ;;
    rm)     cmd_rm "$2" ;;
    *)
        echo "pdf_manipulator secrets manager"
        echo ""
        echo "Usage:"
        echo "  ./secrets.sh set   <env>/<KEY> <value>   Store in Bitwarden + GitHub"
        echo "  ./secrets.sh get   <env>/<KEY>           Read from Bitwarden"
        echo "  ./secrets.sh list  <env>                 List secret names"
        echo "  ./secrets.sh dump  <env>                 Show values (careful!)"
        echo "  ./secrets.sh upload <env|all>            Push Bitwarden → GitHub"
        echo "  ./secrets.sh rm    <env>/<KEY>           Delete from Bitwarden + GitHub"
        echo ""
        echo "Examples:"
        echo "  ./secrets.sh set prod/PUB_CREDENTIALS '{\"accessToken\":\"...\",\"refreshToken\":\"...\"}'"
        echo "  ./secrets.sh get prod/PUB_CREDENTIALS"
        echo "  ./secrets.sh list prod"
        echo "  ./secrets.sh upload all"
        echo "  ./secrets.sh rm prod/OLD_KEY"
        ;;
esac
