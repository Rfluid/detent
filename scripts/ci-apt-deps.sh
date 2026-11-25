#!/usr/bin/env bash
# Install the Debian/Ubuntu build dependencies (CI runners).
#   ci-apt-deps.sh core   libudev only: everything except the GPUI plugin
#   ci-apt-deps.sh [all]  + gpui-pre's Wayland/X11, Vulkan, font and audio headers
# Fedora equivalents: docs/dev/setup.md.
set -euo pipefail

core=(build-essential pkg-config libudev-dev)
gpui=(
    libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev
    libx11-dev libx11-xcb-dev libxcb1-dev
    libfontconfig-dev libfreetype-dev
    libvulkan-dev libasound2-dev libssl-dev libzstd-dev
)

case "${1:-all}" in
    core) pkgs=("${core[@]}") ;;
    all) pkgs=("${core[@]}" "${gpui[@]}") ;;
    *)
        echo "usage: $0 [core|all]" >&2
        exit 2
        ;;
esac

sudo apt-get update -q
sudo apt-get install -y -q --no-install-recommends "${pkgs[@]}"
