#!/usr/bin/env bash
# Detent installer. No root, except one optional, explicitly confirmed step (udev rule).
#
# Two sources, detected automatically:
#   * source  : run from a git checkout (scripts/install.sh). Builds with
#               `cargo build --release --locked`, then installs target/release/*.
#   * release : run from an extracted release tarball (./install.sh next to the
#               binaries). Installs the prebuilt binaries as they are.
#
# What it does on Linux:
#   1. binaries detentd, detentctl, detent-kde, detent-radial, detent-tray -> ~/.local/bin
#      (detentd finds the native plugins next to its own executable)
#   2. systemd user units -> ~/.config/systemd/user/{detent,detent-tray}.service, and the
#      tray icons -> ~/.local/share/icons/hicolor/scalable/status/
#   3. udev rule -> /etc/udev/rules.d/70-detent.rules: the only step that needs root.
#      Prints the exact commands and asks before running them with sudo; never
#      runs sudo silently. Skipped when the installed rule is already identical.
#   4. systemctl --user enable --now detent (restart if it was already running), and the
#      same for detent-tray when a graphical session is running (else: enabled for next login)
#
# macOS (preview): only detentctl -> ~/.local/bin, with the quarantine flag removed.
# Steps 2-4 are Linux-only. Windows: scripts/install.ps1.
#
# Ends with platform notes and an opt-in sponsor prompt (see --help).
# Config in ~/.config/detent is never touched. Undo with scripts/uninstall.sh.
# The remote installer (install.sh at the repo root) downloads a release and runs this
# with DETENT_REMOTE_INSTALL=1; it then prints the closing notes itself.
set -euo pipefail

OS="$(uname -s)"
BIN_DIR="${DETENT_BIN_DIR:-$HOME/.local/bin}"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/status"
UDEV_DEST="/etc/udev/rules.d/70-detent.rules"
BINARIES=(detentd detentctl detent-kde detent-radial detent-tray)
PACKAGES=(-p detentd -p detentctl -p detent-plugin-kde -p detent-plugin-radial -p detent-tray)
if [[ "$OS" == Darwin ]]; then
    BINARIES=(detentctl)
    PACKAGES=(-p detentctl)
fi

usage() {
    cat <<'EOF'
Usage: install.sh [options]

Options:
  --no-build      Source checkout only: install the existing target/release binaries.
  --no-udev       Skip the udev rule (you can install it later; see the printed command).
  --yes           Answer yes to the udev prompt (the sudo command is still printed).
  --no-service    Install the unit files but don't enable or (re)start the services.
  -h, --help      Show this help.

On macOS only detentctl is installed (a preview); --no-udev, --yes and --no-service
are accepted and have nothing to do.

Environment:
  DETENT_BIN_DIR            Where binaries go (default ~/.local/bin). The systemd unit
                            expects ~/.local/bin; other locations need a unit override.
  DETENT_NO_SPONSOR_PROMPT  Skip the closing sponsor prompt (also skipped when CI is
                            set or no terminal is attached).
EOF
}

build=true
udev=ask
service=true
while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-build) build=false ;;
        --no-udev) udev=skip ;;
        --yes | -y) [[ "$udev" == skip ]] || udev=yes ;;
        --no-service) service=false ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "error: unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
    shift
done

say() { printf '\033[1m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[33mwarning:\033[0m %s\n' "$*" >&2; }
die() {
    printf '\033[31merror:\033[0m %s\n' "$*" >&2
    exit 1
}

case "$OS" in
    Linux | Darwin) ;;
    MINGW* | MSYS* | CYGWIN*)
        die "on Windows, run the PowerShell installer from PowerShell:
    irm https://raw.githubusercontent.com/Rfluid/detent/main/scripts/install.ps1 | iex"
        ;;
    *) die "no Detent build for $OS. Linux gets the full install, macOS and Windows a detentctl preview." ;;
esac

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Locate the payload: a release tarball has the binaries next to install.sh; a checkout
# has Cargo.toml one level up from scripts/.
if [[ -x "$HERE/${BINARIES[0]}" && ("$OS" == Darwin || -f "$HERE/detent.service") ]]; then
    mode=release
    BIN_SRC="$HERE"
    UNIT_SRC="$HERE/detent.service"
    TRAY_UNIT_SRC="$HERE/detent-tray.service"
    ICON_SRC="$HERE/icons"
    UDEV_SRC="$HERE/70-detent.rules"
elif [[ -f "$HERE/../Cargo.toml" && -d "$HERE/../bins/detentd" ]]; then
    mode=source
    ROOT="$(cd "$HERE/.." && pwd)"
    BIN_SRC="$ROOT/target/release"
    UNIT_SRC="$ROOT/packaging/systemd/detent.service"
    TRAY_UNIT_SRC="$ROOT/packaging/systemd/detent-tray.service"
    ICON_SRC="$ROOT/packaging/icons/hicolor/scalable/status"
    UDEV_SRC="$ROOT/packaging/udev/70-detent.rules"
else
    die "run this from a Detent checkout (scripts/install.sh) or an extracted release tarball,
    or use the remote installer: curl -fsSL https://raw.githubusercontent.com/Rfluid/detent/main/install.sh | bash"
fi
say "Installing Detent ($mode, $OS)"

if [[ "$mode" == source ]] && $build; then
    command -v cargo >/dev/null || die "cargo not found; install Rust from https://rustup.rs"
    if [[ "$OS" == Linux ]]; then
        say "Building release binaries (first build of the radial plugin takes a few minutes)"
    else
        say "Building detentctl"
    fi
    (cd "$ROOT" && cargo build --release --locked "${PACKAGES[@]}")
fi

for b in "${BINARIES[@]}"; do
    [[ -x "$BIN_SRC/$b" ]] || die "missing binary: $BIN_SRC/$b"
done
if [[ "$OS" == Linux ]]; then
    [[ -f "$UNIT_SRC" ]] || die "missing systemd unit: $UNIT_SRC"
    [[ -f "$TRAY_UNIT_SRC" ]] || die "missing systemd unit: $TRAY_UNIT_SRC"
    compgen -G "$ICON_SRC/detent-tray-*-symbolic.svg" >/dev/null || die "missing tray icons in $ICON_SRC"
    [[ -f "$UDEV_SRC" ]] || die "missing udev rule: $UDEV_SRC"
fi

# 1. Binaries. Install to a temp name and rename, so a running daemon keeps its inode.
say "Binaries -> $BIN_DIR"
mkdir -p "$BIN_DIR"
for b in "${BINARIES[@]}"; do
    install -m 0755 "$BIN_SRC/$b" "$BIN_DIR/.$b.new"
    mv -f "$BIN_DIR/.$b.new" "$BIN_DIR/$b"
    if [[ "$OS" == Darwin ]]; then
        # Release archives are unsigned: drop Gatekeeper's quarantine flag if a browser
        # download set it (curl doesn't).
        xattr -d com.apple.quarantine "$BIN_DIR/$b" 2>/dev/null || true
    fi
done
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
        warn "$BIN_DIR is not on your PATH; add it to use detentctl from a shell:"
        warn "    export PATH=\"$BIN_DIR:\$PATH\"   (in ~/.bashrc, ~/.zshrc, ...)"
        ;;
esac
for b in "${BINARIES[@]}"; do
    if [[ -e "$HOME/.cargo/bin/$b" ]]; then
        warn "an older copy exists at ~/.cargo/bin/$b (from \`cargo install\`); remove it with:"
        warn "    cargo uninstall detentd detentctl detent-plugin-kde detent-plugin-radial detent-tray"
        break
    fi
done

install_linux_integration() {
    # 2. systemd user units, tray icons.
    say "systemd user units -> $UNIT_DIR/{detent,detent-tray}.service"
    install -Dm 0644 "$UNIT_SRC" "$UNIT_DIR/detent.service"
    install -Dm 0644 "$TRAY_UNIT_SRC" "$UNIT_DIR/detent-tray.service"
    say "Tray icons -> $ICON_DIR"
    mkdir -p "$ICON_DIR"
    install -m 0644 "$ICON_SRC"/detent-tray-*-symbolic.svg "$ICON_DIR/"
    # Icon caches (non-fatal): refresh GTK's cache only if one already exists there; bumping
    # the theme dir's mtime makes KDE's icon loader rescan.
    local hicolor="${ICON_DIR%/scalable/status}"
    touch "$hicolor" 2>/dev/null || true
    if [[ -f "$hicolor/icon-theme.cache" ]] && command -v gtk-update-icon-cache >/dev/null; then
        gtk-update-icon-cache -q -t -f "$hicolor" 2>/dev/null || true
    fi

    # 3. udev rule (root). Ask, show the commands, never sudo silently.
    local udev_cmds=(
        "sudo install -m 0644 '$UDEV_SRC' $UDEV_DEST"
        "sudo udevadm control --reload-rules"
        "sudo udevadm trigger --subsystem-match=hidraw --subsystem-match=input --subsystem-match=misc"
    )
    if cmp -s "$UDEV_SRC" "$UDEV_DEST"; then
        say "udev rule already installed and up to date"
    elif [[ "$udev" == skip ]]; then
        warn "udev rule skipped. Without it detentd cannot open the mouse. Install it later with:"
        printf '    %s\n' "${udev_cmds[@]}"
    else
        say "The udev rule gives your login session access to Logitech hidraw/evdev nodes and uinput."
        echo "    It is the only step that needs root. Commands:"
        printf '    %s\n' "${udev_cmds[@]}"
        local answer=n
        if [[ "$udev" == yes ]]; then
            answer=y
        elif [[ -t 0 ]]; then
            read -r -p "    Run them now with sudo? [y/N] " answer || answer=n
        else
            warn "no terminal to ask on; run the commands above yourself."
        fi
        if [[ "$answer" =~ ^[Yy] ]]; then
            sudo install -m 0644 "$UDEV_SRC" "$UDEV_DEST"
            sudo udevadm control --reload-rules
            sudo udevadm trigger --subsystem-match=hidraw --subsystem-match=input --subsystem-match=misc
            say "udev rule installed"
        else
            warn "udev rule not installed; detentd won't see the mouse until you run the commands above."
        fi
    fi

    # 4. Service.
    if [[ "$UNIT_DIR" != "$HOME/.config/systemd/user" ]]; then
        # Relocated XDG_CONFIG_HOME (e.g. a test sandbox): the user manager won't read this
        # unit, and the real service must not be touched.
        warn "unit installed outside ~/.config/systemd/user; not touching the systemd service."
    elif ! command -v systemctl >/dev/null || ! systemctl --user show-environment >/dev/null 2>&1; then
        warn "no systemd user session; start detentd yourself ($BIN_DIR/detentd)."
    elif $service; then
        systemctl --user daemon-reload
        if systemctl --user is-active --quiet detent; then
            say "Restarting the running service on the new binaries"
            systemctl --user restart detent
        else
            say "Enabling and starting the user service"
            systemctl --user enable --now detent ||
                warn "could not start it now (no graphical session?); it starts at your next login."
        fi
        systemctl --user enable detent >/dev/null 2>&1 || true
        # Tray: only start it now inside a running graphical session; otherwise enable it so
        # it comes up with the next login.
        if systemctl --user is-active --quiet graphical-session.target; then
            if systemctl --user is-active --quiet detent-tray; then
                say "Restarting the tray icon on the new binary"
                systemctl --user restart detent-tray || warn "could not restart detent-tray"
            else
                say "Enabling and starting the tray icon"
                systemctl --user enable --now detent-tray || warn "could not start detent-tray"
            fi
        else
            systemctl --user enable detent-tray >/dev/null 2>&1 || true
            say "Tray icon enabled; it starts with your next graphical login."
        fi
        say "Tray icon: if it landed in the '^' overflow group, right-click the tray → Configure System Tray → Entries → Detent → set Visibility to 'Always shown'."
    else
        systemctl --user daemon-reload
        say "Services not enabled (--no-service). Start them with: systemctl --user enable --now detent detent-tray"
    fi
}

if [[ "$OS" == Linux ]]; then
    install_linux_integration
    echo
    say "Done. Check it with: detentctl status   (logs: journalctl --user -u detent -f)"
else
    echo
    say "Done. Try it with: detentctl devices   (then: detentctl settings list)"
    say "If detentctl can't open the mouse, allow your terminal under System Settings → Privacy & Security → Input Monitoring."
fi

# ── Closing notes and sponsor prompt ─────────────────────────────────────────────
# Same text and rules in install.sh (repo root) and scripts/install.ps1; keep in step.
# The remote installer prints these itself after running this script.

ISSUES_URL="https://github.com/Rfluid/detent/issues?q=is%3Aissue+is%3Aopen+label%3A"
HELP_WANTED_URL="${ISSUES_URL}%22help+wanted%22"
SPONSOR_URL="https://github.com/sponsors/Rfluid"
PIX_URL="https://livepix.gg/rfluid"

# Platform note: what this desktop or OS can't do yet, and where to help.
platform_note() {
    local label="" name=""
    case "$OS" in
        Darwin) label=platform%3Amacos ;;
        Linux)
            local desktop="${XDG_CURRENT_DESKTOP:-}" session="${XDG_SESSION_TYPE:-}" lower
            lower="$(printf '%s' "$desktop" | tr '[:upper:]' '[:lower:]')"
            case "$lower" in
                "") return 0 ;; # no desktop session (ssh, tty): nothing to say
                *kde*) return 0 ;; # supported: plugins/kde
                *gnome*) label=platform%3Alinux-gnome name=GNOME ;;
                *hyprland*) label=platform%3Alinux-wlroots name=Hyprland ;;
                *sway*) label=platform%3Alinux-wlroots name=Sway ;;
                *river* | *wayfire* | *labwc* | *wlroots*) label=platform%3Alinux-wlroots name="$desktop" ;;
                *)
                    name="$desktop"
                    if [[ "$session" == x11 ]]; then label=platform%3Alinux-x11; fi
                    ;;
            esac
            ;;
        *) return 0 ;;
    esac
    local url="$HELP_WANTED_URL"
    [[ -z "$label" ]] || url="${ISSUES_URL}${label}"
    echo
    if [[ "$OS" == Linux ]]; then
        echo "Detent's desktop actions don't support $name yet: settings and remapping work,"
        echo "but swipes can't switch workspaces. Help build it: $url"
    else
        echo "This is a preview: detentctl can read and change your mouse's settings. The"
        echo "background service, remapping and gestures are open issues: $url"
    fi
}

prompt_sponsor() {
    [[ -z "${DETENT_NO_SPONSOR_PROMPT:-}" ]] || return 0
    [[ -z "${CI:-}" ]] || return 0

    echo
    echo "Detent is free and built by one person. If it helps you, please consider"
    echo "sponsoring: ${SPONSOR_URL}"
    echo "Prefer Pix (BRL)? ${PIX_URL}"

    # `[ -r /dev/tty ]` passes even without a controlling terminal, so try to actually
    # open it (in a subshell, so a failure can't trip `set -e`).
    (exec </dev/tty) 2>/dev/null || return 0

    local answer=""
    printf 'Open the sponsor page now? [y/N] ' >/dev/tty
    read -r answer </dev/tty || answer=""
    case "$answer" in
        [yY] | [yY][eE][sS]) ;;
        *) return 0 ;;
    esac

    if [[ "$OS" == Darwin ]] && command -v open >/dev/null 2>&1; then
        open "$SPONSOR_URL" >/dev/null 2>&1 &
    elif [[ "$OS" != Darwin ]] && command -v xdg-open >/dev/null 2>&1; then
        xdg-open "$SPONSOR_URL" >/dev/null 2>&1 &
    else
        echo "Open this link in your browser: ${SPONSOR_URL}"
    fi
}

if [[ -z "${DETENT_REMOTE_INSTALL:-}" ]]; then
    platform_note
    echo
    echo "Detent is early and community-built. Open issues come with ready-to-run agent prompts: $HELP_WANTED_URL"
    prompt_sponsor
fi
