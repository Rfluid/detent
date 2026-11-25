#!/usr/bin/env bash
# prepare-release.sh: bump the workspace version for the next release (adapted from aura).
#
#   scripts/prepare-release.sh [patch|minor|major|X.Y.Z] [--dry-run] [--no-fetch]
#
# Detent versions every crate together ([workspace.package] version in Cargo.toml), so
# this bumps that one field from the latest vX.Y.Z tag (or the manifest when there is no
# tag yet), refreshes Cargo.lock, and prints the commit/tag/push steps. It never commits,
# tags or pushes by itself. Process: docs/dev/release.md.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

die() {
    echo "error: $*" >&2
    exit 1
}

bump="patch"
dry_run=false
fetch=true
for arg in "$@"; do
    case "$arg" in
        patch | minor | major) bump="$arg" ;;
        [0-9]*.[0-9]*.[0-9]*) bump="$arg" ;;
        --dry-run) dry_run=true ;;
        --no-fetch) fetch=false ;;
        -h | --help)
            sed -n '2,10p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) die "unknown argument: $arg" ;;
    esac
done

workspace_version() {
    sed -n '/^\[workspace\.package\]/,/^\[/{s/^version *= *"\(.*\)"/\1/p}' Cargo.toml
}

current="$(workspace_version)"
[[ "$current" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "can't read [workspace.package] version"

if $fetch && git remote get-url origin >/dev/null 2>&1; then
    git fetch --tags --quiet origin || echo "warning: could not fetch tags; using local tags" >&2
fi
latest_tag="$(git tag --list 'v[0-9]*.[0-9]*.[0-9]*' --sort=-version:refname | head -n1)"
base="${latest_tag#v}"
[[ -n "$base" ]] || base="$current"

if [[ "$bump" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    next="$bump"
else
    IFS=. read -r major minor patch <<<"$base"
    case "$bump" in
        patch) next="$major.$minor.$((patch + 1))" ;;
        minor) next="$major.$((minor + 1)).0" ;;
        major) next="$((major + 1)).0.0" ;;
    esac
fi

if git rev-parse -q --verify "refs/tags/v$next" >/dev/null; then
    die "tag v$next already exists"
fi

echo "Latest release tag: ${latest_tag:-<none>}"
echo "Workspace version:  $current -> $next"
if $dry_run; then
    echo "Dry run: nothing changed."
    exit 0
fi

# Only the version line inside [workspace.package].
sed -i "/^\[workspace\.package\]/,/^\[/{s/^version *= *\".*\"/version = \"$next\"/}" Cargo.toml
[[ "$(workspace_version)" == "$next" ]] || die "failed to update Cargo.toml"
cargo update --workspace --quiet

cat <<EOF

Updated Cargo.toml and Cargo.lock to $next. Next:
  just verify
  git commit -am "chore(release): v$next"
  git tag -a v$next -m "Detent v$next"
  git push origin main v$next      # the tag triggers .github/workflows/release.yml
EOF
