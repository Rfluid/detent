#!/usr/bin/env bash
# Detent uninstaller: undoes scripts/install.sh.
#
#   1. systemctl --user disable --now detent detent-tray (detent's ExecStopPost gives the
#      buttons back)
#   2. removes ~/.config/systemd/user/{detent,detent-tray}.service
#   3. removes detentd, detentctl, detent-kde, detent-radial, detent-tray from ~/.local/bin
#      and the detent-tray-*-symbolic.svg icons from ~/.local/share/icons/hicolor
#   4. udev rule: prints the exact sudo commands and asks; never sudo silently
#
# Your config (~/.config/detent) is kept.
set -euo pipefail

BIN_DIR="${DETENT_BIN_DIR:-$HOME/.local/bin}"
UNIT="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/detent.service"
TRAY_UNIT="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/detent-tray.service"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/status"
UDEV_DEST="/etc/udev/rules.d/70-detent.rules"
BINARIES=(detentd detentctl detent-kde detent-radial detent-tray)

udev=ask
for arg in "$@"; do
    case "$arg" in
        --keep-udev) udev=skip ;;
        --yes | -y) [[ "$udev" == skip ]] || udev=yes ;;
        -h | --help)
            echo "Usage: uninstall.sh [--keep-udev] [--yes]"
            exit 0
            ;;
        *)
            echo "error: unknown option: $arg" >&2
            exit 2
            ;;
    esac
done

say() { printf '\033[1m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[33mwarning:\033[0m %s\n' "$*" >&2; }

# Only drive systemd when the unit lives where the user manager reads it. A relocated
# XDG_CONFIG_HOME (e.g. a test sandbox) must never stop the real service.
use_systemctl=false
if [[ "$UNIT" == "$HOME/.config/systemd/user/detent.service" ]] && command -v systemctl >/dev/null &&
    systemctl --user show-environment >/dev/null 2>&1; then
    use_systemctl=true
fi

if $use_systemctl; then
    say "Stopping and disabling the user services"
    systemctl --user disable --now detent-tray 2>/dev/null || true
    systemctl --user disable --now detent 2>/dev/null || true
fi
for u in "$UNIT" "$TRAY_UNIT"; do
    if [[ -e "$u" ]]; then
        rm -f "$u"
        say "Removed $u"
    fi
done
if $use_systemctl; then systemctl --user daemon-reload 2>/dev/null || true; fi

removed_icons=false
for f in "$ICON_DIR"/detent-tray-*-symbolic.svg; do
    [[ -e "$f" ]] || continue
    rm -f "$f"
    removed_icons=true
done
if $removed_icons; then say "Removed the tray icons from $ICON_DIR"; fi

for b in "${BINARIES[@]}"; do
    if [[ -e "$BIN_DIR/$b" ]]; then
        rm -f "$BIN_DIR/$b"
        say "Removed $BIN_DIR/$b"
    fi
done

if [[ -e "$UDEV_DEST" && "$udev" != skip ]]; then
    cmds=(
        "sudo rm -f $UDEV_DEST"
        "sudo udevadm control --reload-rules"
    )
    say "The udev rule needs root to remove. Commands:"
    printf '    %s\n' "${cmds[@]}"
    answer=n
    if [[ "$udev" == yes ]]; then
        answer=y
    elif [[ -t 0 ]]; then
        read -r -p "    Run them now with sudo? [y/N] " answer || answer=n
    fi
    if [[ "$answer" =~ ^[Yy] ]]; then
        sudo rm -f "$UDEV_DEST"
        sudo udevadm control --reload-rules
        say "Removed $UDEV_DEST"
    else
        warn "kept $UDEV_DEST; remove it later with the commands above."
    fi
fi

say "Detent uninstalled. Config in ~/.config/detent was kept."
