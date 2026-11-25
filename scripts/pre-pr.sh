#!/usr/bin/env bash
# pre-pr.sh: run everything CI runs, locally, before pushing (adapted from aura).
# Mirrors .github/workflows/ci.yml and secret-scanning.yml.
#
#   rustfmt          AUTO-FIXES (cargo fmt --all); everything else only checks
#   file_size        scripts/check-file-size.sh
#   clippy, test     whole workspace (core + GPUI plugin), --locked
#   actionlint, shellcheck, gitleaks, cargo_audit   skipped with a hint if not installed
#   build_release    only with --release (slow): cargo build --release --locked
#
# Usage: scripts/pre-pr.sh [--release]
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT" || exit 1

with_release=false
[[ "${1:-}" == --release ]] && with_release=true

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

declare -a ORDER=()
declare -A RESULT=() HINT=()
HINT[rustfmt]="cargo fmt --all changed files; review and stage them"
HINT[file_size]="split the file (see .agent/context/conventions.md, File size)"
HINT[clippy]="cargo clippy --workspace --all-targets --locked -- -D warnings"
HINT[test]="cargo test --workspace --locked"
HINT[actionlint]="fix the workflow errors above"
HINT[shellcheck]="shellcheck install.sh scripts/*.sh"
HINT[gitleaks]="remove the secret from the commit (and history)"
HINT[cargo_audit]="upgrade the dependency, or ignore an accepted advisory in .cargo/audit.toml with a reason"
HINT[build_release]="cargo build --release --locked --workspace"

run() {
    local name="$1"
    shift
    ORDER+=("$name")
    echo -e "\n${BOLD}── $name${RESET} ${DIM}$*${RESET}"
    if "$@"; then RESULT[$name]=pass; else RESULT[$name]=fail; fi
}

skip() {
    ORDER+=("$1")
    RESULT[$1]=skip
    HINT[$1]="$2"
    echo -e "\n${YELLOW}── $1 skipped: $2${RESET}"
}

has() { command -v "$1" >/dev/null 2>&1; }

fmt_fix() {
    cargo fmt --all
    if ! git diff --quiet -- '*.rs'; then
        echo "cargo fmt changed files:"
        git diff --name-only -- '*.rs'
        return 1
    fi
}

run rustfmt fmt_fix
run file_size bash scripts/check-file-size.sh
run clippy cargo clippy --workspace --all-targets --locked -- -D warnings
run test cargo test --workspace --locked

if has actionlint; then run actionlint actionlint; else
    skip actionlint "not installed: go install github.com/rhysd/actionlint/cmd/actionlint@latest"
fi
if has shellcheck; then run shellcheck shellcheck install.sh scripts/*.sh; else
    skip shellcheck "not installed: sudo dnf install ShellCheck"
fi
if has gitleaks; then run gitleaks gitleaks git --redact .; else
    skip gitleaks "not installed: https://github.com/gitleaks/gitleaks#installing"
fi
if cargo audit --version >/dev/null 2>&1; then run cargo_audit cargo audit; else
    skip cargo_audit "not installed: cargo install cargo-audit --locked"
fi
if $with_release; then
    run build_release cargo build --release --locked --workspace
fi

echo -e "\n${BOLD}Pre-PR summary${RESET}"
failed=false
for name in "${ORDER[@]}"; do
    case "${RESULT[$name]}" in
        pass) printf "  ${GREEN}PASS${RESET}  %s\n" "$name" ;;
        skip) printf "  ${YELLOW}SKIP${RESET}  %s ${DIM}(%s)${RESET}\n" "$name" "${HINT[$name]}" ;;
        *)
            printf "  ${RED}FAIL${RESET}  %s ${DIM}hint: %s${RESET}\n" "$name" "${HINT[$name]}"
            failed=true
            ;;
    esac
done

if $failed; then
    echo -e "\n${RED}${BOLD}Fix the failures above before opening a PR.${RESET}"
    exit 1
fi
echo -e "\n${GREEN}${BOLD}All checks passed.${RESET}"
