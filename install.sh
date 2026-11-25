#!/usr/bin/env bash
# Detent remote installer (Linux, macOS):
#
#   curl -fsSL https://raw.githubusercontent.com/Rfluid/detent/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/Rfluid/detent/main/install.sh | bash -s -- --version v0.1.0 --no-udev
#
# Detects the OS and CPU, resolves the latest GitHub release (or the one pinned with
# --version / DETENT_VERSION), downloads detent-<tag>-<target>.tar.gz and its .sha256,
# verifies the checksum, extracts it to a temporary directory and runs the bundled
# install.sh (scripts/install.sh in the repo):
#   Linux : the full install. Binaries in ~/.local/bin, systemd user units, and the udev
#           rule (asks before running sudo), then starts the service.
#   macOS : the detentctl preview in ~/.local/bin, quarantine flag removed.
# Windows: use scripts/install.ps1 from PowerShell.
#
# Running it again updates: the bundled installer replaces the binaries and restarts a
# running service. Release asset contract: docs/dev/release.md.
#
# The whole script is wrapped in main(), called on the last line, so a download cut
# short by the network can't run half a script.
set -euo pipefail

REPO="Rfluid/detent"
RELEASE_BASE_URL="${DETENT_RELEASE_URL:-https://github.com/${REPO}/releases}"
ISSUES_URL="https://github.com/${REPO}/issues?q=is%3Aissue+is%3Aopen+label%3A"
HELP_WANTED_URL="${ISSUES_URL}%22help+wanted%22"
SPONSOR_URL="https://github.com/sponsors/Rfluid"
PIX_URL="https://livepix.gg/rfluid"

usage() {
    cat <<'EOF'
Usage: install.sh [options]          (curl ... | bash -s -- [options])

Downloads the Detent release for this machine, verifies it and runs its installer.
Run it again to update.

Options:
  --version <tag>   Install this release (e.g. v0.1.0) instead of the latest.
  --dry-run         Print the detected platform, the URLs and the command; download
                    and install nothing.
  --no-udev         Linux: skip the udev rule (passed to the bundled installer).
  --yes, -y         Linux: answer yes to the udev prompt (the sudo commands are
                    still printed).
  --no-service      Linux: install the units but don't enable or (re)start them.
  -h, --help        Show this help.

Environment:
  DETENT_VERSION            Same as --version.
  DETENT_BIN_DIR            Where binaries go (default ~/.local/bin).
  DETENT_NO_SPONSOR_PROMPT  Skip the closing sponsor prompt (also skipped when CI is
                            set or no terminal is attached).

Testing:
  DETENT_OS, DETENT_ARCH    Override `uname -s` / `uname -m` (use with --dry-run).
  DETENT_RELEASE_URL        Releases base URL instead of GitHub's; archives are read
                            from <url>/download/<tag>/ (file:// works; needs --version).

Windows: irm https://raw.githubusercontent.com/Rfluid/detent/main/scripts/install.ps1 | iex
EOF
}

say() { printf '\033[1m==>\033[0m %s\n' "$*"; }
die() {
    printf '\033[31merror:\033[0m %s\n' "$*" >&2
    exit 1
}

# `uname -s`-`uname -m` → release target triple. Same table as crates/update (assets.rs).
target_for() {
    case "$1-$2" in
        Linux-x86_64 | Linux-amd64) echo x86_64-unknown-linux-gnu ;;
        Linux-aarch64 | Linux-arm64) echo aarch64-unknown-linux-gnu ;;
        Darwin-x86_64) echo x86_64-apple-darwin ;;
        Darwin-arm64 | Darwin-aarch64) echo aarch64-apple-darwin ;;
        *) return 1 ;;
    esac
}

# The tag /releases/latest redirects to (no API call, so no API rate limit).
resolve_latest() {
    local url
    url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "${RELEASE_BASE_URL}/latest")" || return 1
    url="${url##*/}"
    case "$url" in
        v[0-9]*) echo "$url" ;;
        *) return 1 ;; # no release: GitHub redirects to /releases
    esac
}

verify_checksum() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum -c "$1"
    else
        shasum -a 256 -c "$1"
    fi
}

# Platform note: what this desktop or OS can't do yet, and where to help.
# Same text and rules as scripts/install.sh and scripts/install.ps1; keep in step.
platform_note() {
    local os="$1" label="" name=""
    case "$os" in
        Darwin) label=platform%3Amacos ;;
        Linux)
            local desktop="${XDG_CURRENT_DESKTOP:-}" session="${XDG_SESSION_TYPE:-}" lower
            lower="$(printf '%s' "$desktop" | tr '[:upper:]' '[:lower:]')"
            case "$lower" in
                "") return 0 ;;    # no desktop session (ssh, tty): nothing to say
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
    if [[ "$os" == Linux ]]; then
        echo "Detent's desktop actions don't support $name yet: settings and remapping work,"
        echo "but swipes can't switch workspaces. Help build it: $url"
    else
        echo "This is a preview: detentctl can read and change your mouse's settings. The"
        echo "background service, remapping and gestures are open issues: $url"
    fi
}

community_note() {
    echo
    echo "Detent is early and community-built. Open issues come with ready-to-run agent prompts: $HELP_WANTED_URL"
}

# Opt-in, default No. `curl | bash` leaves stdin on the pipe, so the answer is read
# from /dev/tty; with no terminal the links are printed and that's it.
prompt_sponsor() {
    local os="$1"
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

    if [[ "$os" == Darwin ]] && command -v open >/dev/null 2>&1; then
        open "$SPONSOR_URL" >/dev/null 2>&1 &
    elif [[ "$os" != Darwin ]] && command -v xdg-open >/dev/null 2>&1; then
        xdg-open "$SPONSOR_URL" >/dev/null 2>&1 &
    else
        echo "Open this link in your browser: ${SPONSOR_URL}"
    fi
}

main() {
    local version="${DETENT_VERSION:-}" dry_run=false
    local passthrough=()
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --version)
                [[ $# -ge 2 ]] || die "--version needs a tag, e.g. --version v0.1.0"
                version="$2"
                shift
                ;;
            --version=*) version="${1#--version=}" ;;
            --dry-run) dry_run=true ;;
            --no-udev | --yes | -y | --no-service) passthrough+=("$1") ;;
            -h | --help)
                usage
                return 0
                ;;
            *)
                echo "error: unknown option: $1" >&2
                usage >&2
                exit 2
                ;;
        esac
        shift
    done

    local os arch target
    os="${DETENT_OS:-$(uname -s)}"
    arch="${DETENT_ARCH:-$(uname -m)}"
    case "$os" in
        MINGW* | MSYS* | CYGWIN* | Windows*)
            die "this script is for Linux and macOS. On Windows, run in PowerShell:
    irm https://raw.githubusercontent.com/Rfluid/detent/main/scripts/install.ps1 | iex"
            ;;
    esac
    target="$(target_for "$os" "$arch")" ||
        die "no Detent release for $os $arch. Build from source: https://github.com/${REPO}"

    command -v curl >/dev/null 2>&1 || die "curl not found"
    command -v tar >/dev/null 2>&1 || die "tar not found"

    if [[ -n "$version" ]]; then
        [[ "$version" == v* ]] || version="v$version"
    else
        say "Resolving the latest release"
        if ! version="$(resolve_latest)"; then
            $dry_run || die "could not find a published release at ${RELEASE_BASE_URL}"
            version="<latest>"
        fi
    fi

    local base="detent-${version}-${target}"
    local archive="${base}.tar.gz"
    local url="${RELEASE_BASE_URL}/download/${version}/${archive}"

    if $dry_run; then
        echo "os:        $os"
        echo "arch:      $arch"
        echo "target:    $target"
        echo "version:   $version"
        echo "archive:   $url"
        echo "checksum:  ${url}.sha256"
        echo "would run: DETENT_REMOTE_INSTALL=1 bash <tmp>/${base}/install.sh${passthrough[*]+ ${passthrough[*]}}"
        platform_note "$os"
        community_note
        return 0
    fi

    say "Installing Detent ${version} (${target})"
    local tmp
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/detent-install.XXXXXX")"
    # shellcheck disable=SC2064 # expand now: $tmp is local to main
    trap "rm -rf '$tmp'" EXIT

    curl -fsSL --retry 3 -o "$tmp/$archive" "$url" ||
        die "download failed: $url"
    curl -fsSL --retry 3 -o "$tmp/$archive.sha256" "$url.sha256" ||
        die "download failed: $url.sha256"
    (cd "$tmp" && verify_checksum "$archive.sha256") >/dev/null ||
        die "checksum mismatch for $archive; nothing was installed"
    say "Checksum verified"
    tar -xzf "$tmp/$archive" -C "$tmp"
    [[ -f "$tmp/$base/install.sh" ]] || die "$archive has no $base/install.sh"
    if [[ "$os" == Darwin ]]; then
        # Unsigned archive: make sure nothing in it carries Gatekeeper's quarantine flag.
        xattr -dr com.apple.quarantine "$tmp/$base" 2>/dev/null || true
    fi

    # The bundled installer asks before sudo (udev). Give it the terminal, never the
    # pipe this script is being read from.
    local stdin=/dev/null
    if (exec </dev/tty) 2>/dev/null; then stdin=/dev/tty; fi
    DETENT_REMOTE_INSTALL=1 bash "$tmp/$base/install.sh" ${passthrough[@]+"${passthrough[@]}"} <"$stdin"

    platform_note "$os"
    community_note
    prompt_sponsor "$os"
}

main "$@"
