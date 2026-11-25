#!/usr/bin/env bash
# check-file-size.sh: hard line caps per file, with a ratchet for existing offenders.
#
#   *.rs  <= 600 lines      *.md  <= 400 lines
#
# Files already over the cap are listed in scripts/file-size-allowlist.txt with their
# line count at the time they were listed. That count is their personal cap: they may
# shrink, never grow. When an allowlisted file drops to the default cap or below, the
# script asks you to delete its entry, so the list only ever gets shorter.
#
# Scope: tracked + untracked-but-not-ignored files (git ls-files), minus vendor/,
# target/ and Cargo.lock.
#
# Usage:
#   scripts/check-file-size.sh            check (CI, `just lint`)
#   scripts/check-file-size.sh --update   rewrite the allowlist to the current counts of
#                                         files over the cap (never raises an existing cap)
# Policy: .agent/context/conventions.md ("File size").
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

RS_CAP=600
MD_CAP=400
ALLOWLIST="scripts/file-size-allowlist.txt"

update=false
case "${1:-}" in
    "") ;;
    --update) update=true ;;
    -h | --help)
        sed -n '2,19p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *)
        echo "error: unknown option: $1" >&2
        exit 2
        ;;
esac

cap_for() {
    case "$1" in
        *.rs) echo "$RS_CAP" ;;
        *.md) echo "$MD_CAP" ;;
        *) echo "" ;;
    esac
}

# Allowlist: "<cap> <path>" per line; '#' comments and blank lines ignored.
declare -A allowed=()
if [[ -f "$ALLOWLIST" ]]; then
    while read -r cap path _; do
        [[ -z "${cap:-}" || "$cap" == \#* ]] && continue
        allowed["$path"]="$cap"
    done <"$ALLOWLIST"
fi

declare -A seen=()
declare -a over=() stale=() grown=() new_entries=()

while IFS= read -r -d '' file; do
    [[ -f "$file" ]] || continue
    cap="$(cap_for "$file")"
    [[ -n "$cap" ]] || continue
    lines="$(wc -l <"$file")"
    seen["$file"]=1
    if [[ -n "${allowed[$file]:-}" ]]; then
        limit="${allowed[$file]}"
        if ((lines > limit)); then
            grown+=("$file: $lines lines (allowlisted cap $limit, default $cap)")
        elif ((lines <= cap)); then
            stale+=("$file: $lines lines, now within the default cap $cap")
        fi
        # --update keeps a shrunken file at its new, lower count.
        if ((lines > cap)); then
            new_entries+=("$((lines < limit ? lines : limit)) $file")
        fi
    elif ((lines > cap)); then
        over+=("$file: $lines lines (cap $cap)")
        new_entries+=("$lines $file")
    fi
done < <(git ls-files -z --cached --others --exclude-standard -- \
    '*.rs' '*.md' ':!:vendor/**' ':!:target/**' ':!:Cargo.lock')

for path in "${!allowed[@]}"; do
    [[ -n "${seen[$path]:-}" ]] || stale+=("$path: listed but no longer exists")
done

if $update; then
    if ((${#grown[@]} > 0)); then
        echo "error: --update never raises a cap. Split these files instead:" >&2
        printf '  %s\n' "${grown[@]}" >&2
        exit 1
    fi
    {
        echo "# Files allowed over the size cap (scripts/check-file-size.sh)."
        echo "# Format: <max lines> <path>. Caps only go down: shrink a file, then lower or"
        echo "# delete its entry. Never add new files here; split them instead."
        printf '%s\n' "${new_entries[@]}" | sort -k2
    } >"$ALLOWLIST"
    echo "wrote $ALLOWLIST (${#new_entries[@]} entries)"
    exit 0
fi

status=0
if ((${#over[@]} > 0)); then
    echo "Files over the size cap (split them; see .agent/context/conventions.md):"
    printf '  %s\n' "${over[@]}"
    status=1
fi
if ((${#grown[@]} > 0)); then
    echo "Allowlisted files that grew past their recorded size (they may only shrink):"
    printf '  %s\n' "${grown[@]}"
    status=1
fi
if ((${#stale[@]} > 0)); then
    echo "Stale entries in $ALLOWLIST (delete or lower them):"
    printf '  %s\n' "${stale[@]}"
    status=1
fi
if ((status == 0)); then
    echo "ok: every .rs <= $RS_CAP and .md <= $MD_CAP lines (or within its allowlisted cap)"
fi
exit "$status"
