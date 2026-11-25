#!/usr/bin/env bash
# Build and package a Detent release archive for this machine (Linux or macOS).
#
#   scripts/package-release.sh <version> [--no-build]
#
# <version> is the release tag (v0.1.0) or any label (main-abc1234). Produces:
#   dist/detent-<version>-<target>.tar.gz
#   dist/detent-<version>-<target>.tar.gz.sha256      (sha256sum format)
# The archive holds one directory, detent-<version>-<target>/. Its contents depend on
# the OS (bundle lists: docs/dev/release.md):
#   Linux : detentd, detentctl, detent-kde, detent-radial, detent-tray, install.sh,
#           uninstall.sh, detent.service, detent-tray.service, icons/, 70-detent.rules,
#           detent.example.toml, README.md, LICENSE
#   macOS : detentctl (preview), install.sh, uninstall.sh, README.md, LICENSE
# Windows has its own script: scripts/package-release.ps1 (a .zip).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

version="${1:-}"
build=true
[[ "${2:-}" == --no-build ]] && build=false
if [[ -z "$version" || "$version" == -* ]]; then
    echo "usage: $0 <version> [--no-build]" >&2
    exit 2
fi

os="$(uname -s)"
case "$os-$(uname -m)" in
    Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
    Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-gnu ;;
    Darwin-x86_64) target=x86_64-apple-darwin ;;
    Darwin-arm64 | Darwin-aarch64) target=aarch64-apple-darwin ;;
    *)
        echo "error: unsupported platform $os $(uname -m) (Windows: scripts/package-release.ps1)" >&2
        exit 1
        ;;
esac

if [[ "$os" == Linux ]]; then
    binaries=(detentd detentctl detent-kde detent-radial detent-tray)
    packages=(-p detentd -p detentctl -p detent-plugin-kde -p detent-plugin-radial -p detent-tray)
else
    # macOS preview: the CLI only (reads and changes device settings). The daemon,
    # remapping and gestures are Linux-only today.
    binaries=(detentctl)
    packages=(-p detentctl)
fi

name="detent-${version}-${target}"
stage="dist/${name}"

if $build; then
    cargo build --release --locked "${packages[@]}"
fi

rm -rf "$stage" "dist/${name}.tar.gz" "dist/${name}.tar.gz.sha256"
mkdir -p "$stage"

for b in "${binaries[@]}"; do
    install -m 0755 "target/release/$b" "$stage/$b"
    if command -v strip >/dev/null; then
        if [[ "$os" == Linux ]]; then strip --strip-unneeded "$stage/$b"; else strip -x "$stage/$b"; fi
    fi
done
# The in-bundle installer handles both OSes (macOS: detentctl only).
install -m 0755 scripts/install.sh "$stage/install.sh"
install -m 0755 scripts/uninstall.sh "$stage/uninstall.sh"
if [[ "$os" == Linux ]]; then
    install -m 0644 packaging/systemd/detent.service "$stage/detent.service"
    install -m 0644 packaging/systemd/detent-tray.service "$stage/detent-tray.service"
    install -d "$stage/icons"
    install -m 0644 packaging/icons/hicolor/scalable/status/detent-tray-*-symbolic.svg "$stage/icons/"
    install -m 0644 packaging/udev/70-detent.rules "$stage/70-detent.rules"
    install -m 0644 config/detent.example.toml "$stage/detent.example.toml"
fi
for f in README.md LICENSE; do
    if [[ -f "$f" ]]; then install -m 0644 "$f" "$stage/$f"; fi
done

if [[ "$os" == Linux ]]; then
    tar -C dist --owner=0 --group=0 --numeric-owner -czf "dist/${name}.tar.gz" "$name"
else
    # bsdtar: no --owner; --uid/--gid do the same. COPYFILE_DISABLE keeps ._* files out.
    COPYFILE_DISABLE=1 tar -C dist --uid 0 --gid 0 -czf "dist/${name}.tar.gz" "$name"
fi
(
    cd dist
    if command -v sha256sum >/dev/null; then
        sha256sum "${name}.tar.gz" >"${name}.tar.gz.sha256"
    else
        shasum -a 256 "${name}.tar.gz" >"${name}.tar.gz.sha256"
    fi
)
rm -rf "$stage"

echo "dist/${name}.tar.gz"
echo "dist/${name}.tar.gz.sha256"
