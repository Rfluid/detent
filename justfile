# Detent task runner. `just` lists recipes.

default:
    @just --list

# Type-check the whole workspace
check:
    cargo check --workspace --all-targets

# Check everything except the GPUI plugin (fast loop for core work)
check-core:
    cargo check --workspace --all-targets --exclude detent-plugin-radial

# Run all tests
test:
    cargo test --workspace

# Format + file-size ratchet + lint (CI gate)
lint:
    cargo fmt --all -- --check
    bash scripts/check-file-size.sh
    cargo clippy --workspace --all-targets -- -D warnings

# File-size ratchet only (.rs <= 600, .md <= 400 lines; allowlisted files may only shrink)
check-file-size *ARGS:
    bash scripts/check-file-size.sh {{ARGS}}

fmt:
    cargo fmt --all

# Hot-path benchmarks (criterion); compare against the stored baseline
bench:
    cargo bench -p detent-engine -p detent-hidpp-proto

# Full gate an agent must pass before declaring work done
verify: lint test

# Run the daemon in the foreground with debug logs
run-daemon *ARGS:
    DETENT_LOG=debug cargo run -p detentd -- {{ARGS}}

# CLI passthrough, e.g. `just ctl devices`, `just ctl settings list`
ctl *ARGS:
    cargo run -q -p detentctl -- {{ARGS}}

# Build (release) and install: binaries -> ~/.local/bin, systemd user unit, udev rule
# (prints the sudo command and asks first), then enables the service. Flags pass through:
# --no-build, --no-udev, --yes, --no-service (scripts/install.sh --help)
install *ARGS:
    bash scripts/install.sh {{ARGS}}

# detentctl man pages (roff) into target/man; view with `man -l target/man/detentctl.1`
man:
    cargo run -q -p detentctl -- man --out-dir target/man

# Shell completions into target/completions (bash, zsh, fish)
completions:
    mkdir -p target/completions
    cargo run -q -p detentctl -- completions bash > target/completions/detentctl.bash
    cargo run -q -p detentctl -- completions zsh > target/completions/_detentctl
    cargo run -q -p detentctl -- completions fish > target/completions/detentctl.fish

# Remove what `just install` added (config in ~/.config/detent is kept). --keep-udev, --yes
uninstall *ARGS:
    bash scripts/uninstall.sh {{ARGS}}

# Reinstall on top of an existing install (restarts the running service on the new build)
update *ARGS:
    bash scripts/install.sh {{ARGS}}

# Everything CI runs, locally (fmt auto-fix, ratchet, clippy, tests, linters, audit). --release adds a release build
pre-pr *ARGS:
    bash scripts/pre-pr.sh {{ARGS}}

# Release archive for this machine into dist/: Linux full set, macOS detentctl (asset naming: docs/dev/release.md)
[unix]
package VERSION="local":
    bash scripts/package-release.sh {{VERSION}}

# Release zip for this machine into dist/: detentctl preview (asset naming: docs/dev/release.md)
[windows]
package VERSION="local":
    pwsh -NoProfile -File scripts/package-release.ps1 -Version {{VERSION}}

# Remote installer dry run: detected platform, release URLs, closing notes. Installs nothing
install-dry-run *ARGS:
    bash install.sh --dry-run {{ARGS}}

# Bump the workspace version for a release: patch | minor | major | X.Y.Z [--dry-run]
release-prep *ARGS:
    bash scripts/prepare-release.sh {{ARGS}}
