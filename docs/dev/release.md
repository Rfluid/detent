---
title: Releases, release assets and install contract
status: current
version: 0.2.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: [".github/workflows/release.yml", "scripts/package-release.sh", "scripts/package-release.ps1", "scripts/prepare-release.sh", "install.sh", "scripts/install.sh", "scripts/uninstall.sh", "scripts/install.ps1", "scripts/uninstall.ps1", "packaging/", "crates/update/"]
owner: "@rfluid"
tags: [dev, release, install]
---

# Releases

Same approach as aura: you bump the version by hand and push a tag. The tag triggers a
GitHub Actions build, which publishes a GitHub Release with generated notes and the
archives attached. CI, bots and repo settings: [ci-and-release.md](ci-and-release.md).

## Cutting a release

```sh
just release-prep patch          # or minor | major | 1.2.3; --dry-run to preview
just verify
git commit -am "chore(release): vX.Y.Z"
git tag -a vX.Y.Z -m "Detent vX.Y.Z"
git push origin main vX.Y.Z      # triggers .github/workflows/release.yml
```

`scripts/prepare-release.sh` (adapted from aura) bumps `[workspace.package] version` in
`Cargo.toml`. All crates share that version. The bump starts from the latest `vX.Y.Z` tag,
or from the manifest when there is no tag yet. It runs `cargo update --workspace` (which
touches workspace entries only) and never commits, tags or pushes.

`release.yml` refuses a tag that doesn't match the workspace version. Each matrix row then
runs the tests for what it builds, builds with `--locked`, packages, and checks the archive
name against this contract. The publish job verifies every `.sha256` and publishes. Tags
`v0.0.*` and tags with a suffix (`v1.0.0-rc1`) are published as pre-releases; `v0.1.0` and
later plain tags are full releases (and become `/releases/latest`).

**Dry run:** run the workflow by hand (Actions → Release → Run workflow, or `gh workflow run
release.yml`). It builds, tests and packages every row under the label
`v<version>-dryrun.<sha7>`, uploads the archives as 7-day workflow artifacts, and publishes
nothing.

| Target | Runner | Ships | Tests |
|---|---|---|---|
| `x86_64-unknown-linux-gnu` | `ubuntu-24.04` (glibc 2.39) | everything | `cargo test --workspace` |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | everything | `cargo test --workspace` |
| `aarch64-apple-darwin` | `macos-14` | `detentctl` preview | detentctl and the crates it's built from (`PREVIEW_TEST_PACKAGES` in release.yml) |
| `x86_64-apple-darwin` | `macos-15-intel` | `detentctl` preview | same |
| `x86_64-pc-windows-msvc` | `windows-latest` | `detentctl` preview | same |

`macos-13` is retired on GitHub-hosted runners; `macos-15-intel` is the current Intel image.
Every row builds natively, so binaries are in `target/release/`.

## Release assets (contract)

The update check (`crates/update`: `detentctl update check` and the tray), the remote
installers (`install.sh`, `scripts/install.ps1`) and the README's install steps depend on
these names. Keep them stable. If one has to change, update this section and the consumers
in the same commit.

| Asset | Name |
|---|---|
| Archive, Linux and macOS | `detent-<tag>-<target>.tar.gz` |
| Archive, Windows | `detent-<tag>-<target>.zip` |
| Checksum | archive name + `.sha256` (`sha256sum` format: `<hex>  <file name>`, LF) |

- `<tag>` is the git tag including the `v`, e.g. `v0.1.0`. It equals `v` +
  `[workspace.package] version`, and `detentctl --version` prints that version.
- `<target>` is the Rust target triple of the matrix row above.
- Examples: `detent-v0.1.0-x86_64-unknown-linux-gnu.tar.gz`,
  `detent-v0.1.0-aarch64-apple-darwin.tar.gz.sha256`,
  `detent-v0.1.0-x86_64-pc-windows-msvc.zip`.
- `crates/update` (`assets.rs`) maps `std::env::consts::{OS, ARCH}` to the target and the
  target to the name. Its tests pin every name above; the remote installers build the same
  names and say so in a comment.

Each archive holds one top-level directory with the archive's name, without the extension.

**Linux** (`scripts/package-release.sh`):

```
detent-v0.1.0-x86_64-unknown-linux-gnu/
  detentd  detentctl  detent-kde  detent-radial  detent-tray   (stripped, 0755)
  install.sh  uninstall.sh                          (copies of scripts/*.sh)
  detent.service  detent-tray.service               (packaging/systemd/)
  icons/detent-tray-*-symbolic.svg                  (packaging/icons/hicolor/scalable/status/)
  70-detent.rules                                   (packaging/udev/)
  detent.example.toml                               (config/)
  README.md  LICENSE
```

**macOS** (`scripts/package-release.sh`; unsigned, not notarized):

```
detent-v0.1.0-aarch64-apple-darwin/
  detentctl                 (stripped, 0755)
  install.sh  uninstall.sh  (the same scripts; on macOS they handle detentctl only)
  README.md  LICENSE
```

**Windows** (`scripts/package-release.ps1`; unsigned):

```
detent-v0.1.0-x86_64-pc-windows-msvc/
  detentctl.exe
  install.ps1  uninstall.ps1   (copies of scripts/*.ps1)
  README.md  LICENSE
```

### Update check (`crates/update`)

Implemented in `detent-update`, shared by `detentctl update check` and `detent-tray`:

- Latest release: anonymous `GET https://api.github.com/repos/Rfluid/detent/releases/latest`.
  It compares `tag_name` (the `v` stripped) with the binary's `CARGO_PKG_VERSION` using
  semver; only a strictly newer release counts. `/releases/latest` skips pre-releases.
- Rate limit: GitHub allows 60 anonymous API requests an hour per IP. Only when that limit
  answers (HTTP 403 or 429) does the check retry once with a token: `gh auth token`, else
  the file named by `token_file` under `[update]` in `~/.config/detent/tray.toml`. With no
  token the error says so and how to add one. A refused token is `Unauthorized` (exit 77).
- `Release::asset_for(target)` / `checksum_for(target)` pick this platform's archive and
  `.sha256` from the release's `assets` (`browser_download_url`); `host_target()` is the
  running platform's target. The check itself never downloads anything.

## Install contract

### Remote installers

```sh
curl -fsSL https://raw.githubusercontent.com/Rfluid/detent/main/install.sh | bash        # Linux, macOS
curl -fsSL .../install.sh | bash -s -- --version v0.1.0 --no-udev                         # with flags
irm https://raw.githubusercontent.com/Rfluid/detent/main/scripts/install.ps1 | iex        # Windows
```

`install.sh` (repo root) detects the OS and CPU (`uname -s`/`-m`; Git Bash and MSYS are sent
to the PowerShell installer), resolves the latest tag from the `/releases/latest` redirect
(no API call) or takes `--version` / `DETENT_VERSION`, downloads the archive and its
`.sha256`, verifies it (`sha256sum`, else `shasum -a 256`), extracts to a temp directory and
runs the bundled `install.sh` with `DETENT_REMOTE_INSTALL=1`, the pass-through flags
(`--no-udev`, `--yes`, `--no-service`) and stdin from `/dev/tty` (never the pipe), so the
udev `sudo` question still reaches the user. The whole script is one `main` called on its
last line, so a truncated download runs nothing. `--dry-run` prints the platform, URLs and
command and installs nothing; `DETENT_OS`, `DETENT_ARCH` and `DETENT_RELEASE_URL` (a
`file://` mirror works) exist for testing.

`scripts/install.ps1` does the same on Windows: target from `PROCESSOR_ARCHITECTURE` (ARM64
gets the x64 build under emulation, with a warning), latest tag from the API, `-Version` /
`DETENT_VERSION`, `-DryRun`, SHA256 check, then `detentctl.exe` (and `uninstall.ps1`) into
`%LOCALAPPDATA%\Programs\Detent`, which is added to the user PATH. Run from an extracted zip
it installs that zip's binary; from a checkout with cargo, it builds `-p detentctl`.
`scripts/uninstall.ps1` removes the directory and the PATH entry and keeps the config.

Updating is running the installer again, on every OS.

### Closing notes and sponsor prompt (all installers)

At the end, in this order:

1. A platform note, when there is something missing. Linux, from `XDG_CURRENT_DESKTOP` /
   `XDG_SESSION_TYPE`: KDE and no desktop (ssh, tty) say nothing; otherwise "Detent's
   desktop actions don't support <desktop> yet: settings and remapping work, but swipes
   can't switch workspaces. Help build it: <URL>". macOS and Windows: "This is a preview:
   detentctl can read and change your mouse's settings. The background service, remapping
   and gestures are open issues: <URL>".
2. Always: "Detent is early and community-built. Open issues come with ready-to-run agent
   prompts: <help wanted URL>".
3. The sponsor prompt (ported from aura): the GitHub Sponsors and Pix links, then an opt-in
   `[y/N]` read from `/dev/tty` (PowerShell: `Read-Host`); yes opens the page with
   `xdg-open` / `open` / `Start-Process`. Skipped when `DETENT_NO_SPONSOR_PROMPT` is set or
   `CI` is set; with no terminal the links are printed without the question. It runs on
   every install and update.

URLs are label filters, never issue numbers:
`https://github.com/Rfluid/detent/issues?q=is%3Aissue+is%3Aopen+label%3A<label>`, with
`platform%3Alinux-gnome`, `platform%3Alinux-wlroots` (Hyprland, Sway, river, Wayfire,
labwc), `platform%3Alinux-x11` (other desktops on an X11 session), `platform%3Amacos`,
`platform%3Awindows`, and `%22help+wanted%22` (also the fallback for other Wayland desktops).
The text lives in three places (`install.sh`, `scripts/install.sh`, `scripts/install.ps1`);
keep them in step. Under the remote installer the bundled script skips its own copy.

### In-bundle installer (`scripts/install.sh`)

The same script runs from a checkout (`just install`, `scripts/install.sh`) and from an
extracted archive (`./install.sh`). It detects which one: binaries next to the script mean
an archive, a `Cargo.toml` one level up means a checkout.

1. Checkout only: `cargo build --release --locked` for the five binaries (macOS: detentctl).
   Skip it with `--no-build`.
2. Binaries go to `~/.local/bin` (`DETENT_BIN_DIR` overrides). Each file is replaced
   atomically by a rename. `detentd` starts the native plugins from its own directory.
   The script warns about leftover `cargo install` copies in `~/.cargo/bin`. On macOS the
   `com.apple.quarantine` flag is removed and steps 3 to 5 don't apply.
3. Units: `~/.config/systemd/user/detent.service` (`ExecStart=%h/.local/bin/detentd`) and
   `detent-tray.service`. Tray icons: `~/.local/share/icons/hicolor/scalable/status/`
   (`$XDG_DATA_HOME` respected); an existing GTK `icon-theme.cache` there is refreshed.
4. udev rule: `/etc/udev/rules.d/70-detent.rules`. This is the only root step. The script
   prints the exact `sudo` commands, then asks `[y/N]`. `--yes` answers for you; the
   commands are still printed. `--no-udev` skips the step. With no terminal it only
   prints the commands. If the installed rule is already identical, the step is skipped.
5. `systemctl --user daemon-reload`, then `enable --now detent`. If the service was
   already running it is restarted instead, so an update takes effect. The tray is
   started (or restarted) the same way only when `graphical-session.target` is active;
   otherwise it is just enabled for the next login. `--no-service` skips this step. When
   the unit directory isn't `~/.config/systemd/user` (a test sandbox with a relocated
   `XDG_CONFIG_HOME`), systemd is never touched.
6. The closing notes and sponsor prompt above.

The config in `~/.config/detent` is never touched. `scripts/uninstall.sh` (`just
uninstall`) disables both services, then removes the units, the binaries and the tray
icons. It asks the same way before removing the udev rule (`--keep-udev` skips that step)
and keeps the config.

`just package [VERSION]` builds the same archive locally into `dist/` (ignored by git):
`scripts/package-release.sh` on Linux and macOS, `scripts/package-release.ps1` on Windows.
