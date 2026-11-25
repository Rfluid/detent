---
title: CI, repository settings and bots
status: current
version: 0.2.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: [".github/", ".cargo/audit.toml", ".gitleaksignore", "clippy.toml", "scripts/check-file-size.sh", "scripts/file-size-allowlist.txt", "scripts/pre-pr.sh", "scripts/ci-apt-deps.sh", "justfile"]
owner: "@rfluid"
tags: [dev, ci, github]
---

# CI, repository settings and bots

Repository: <https://github.com/Rfluid/detent> (public from v0.1.0). The setup follows the owner's
`Rfluid/aura` repo, adapted to a daemon that is complete on Linux and a `detentctl` preview on macOS and Windows. Releases: [release.md](release.md).

## Workflows (`.github/workflows/`)

| Workflow | Trigger | Jobs (check names) | Local equivalent |
|---|---|---|---|
| `ci.yml` (CI) | push to `main`, PRs, manual | **Core - Format, Clippy, Tests & File Size**: `cargo fmt --check`, `scripts/check-file-size.sh`, clippy `-D warnings` and tests for the whole workspace except `detent-plugin-radial`. Only needs `libudev-dev` | `just lint && just test` (`just check-core` for a quick check) |
| | | **GPUI Plugin - Clippy & Tests**: the same for `detent-plugin-radial`, with the GPUI system packages | `cargo clippy -p detent-plugin-radial --all-targets` |
| | | **Portable - Check, Clippy & Tests (macos-14 / windows-latest)**: `cargo check`, clippy `-D warnings` and tests for the whole workspace on macOS and Windows; Linux-only members build as stubs (ADR 0010) | cross-check from Linux, below |
| | | **Workflow & Script Lint**: `actionlint` + `shellcheck install.sh scripts/*.sh` | `just pre-pr` |
| | | **Dependency Security Scan (cargo-audit)** | `cargo audit` |
| | | **Release Build & Package** (not on PRs, after core + gpui): runs `scripts/package-release.sh main-<sha>` and uploads the tarball as a 7-day artifact | `just package` |
| `secret-scanning.yml` | push to `main`, PRs | **Secret Detection (gitleaks)** over the full history | `gitleaks git --redact .` |
| `sbom.yml` | push to `main`, weekly, manual | CycloneDX SBOM (Syft), uploaded as an artifact | none |
| `release.yml` | tag `v*.*.*`; manual (dry run) | **Build <target>** for Linux x86_64/aarch64 (everything), macOS aarch64/x86_64 and Windows x86_64 (the `detentctl` preview): test, package, check the asset name. **Publish GitHub release** (tags only): verify checksums, publish. A manual run builds and uploads the archives as artifacts and publishes nothing | `just package` on each OS; [release.md](release.md) |

CI is split like `just check-core` and `just check`: the core job gives fast feedback and the
GPUI job carries the slow first build and the system packages. Each job has its own
`Swatinem/rust-cache` key (`core`, `gpui`, `portable-<os>`, `release-<target>`), so they don't evict each other.
`scripts/ci-apt-deps.sh core|all` is the single list of Ubuntu packages, used by every job.

**macOS / Windows from Linux.** `rustup target add aarch64-apple-darwin
x86_64-pc-windows-msvc`, then `cargo clippy --workspace --all-targets --target <t>`. Two C
build scripts need the real OS (`ring` for both, hidapi's macOS backend); `cargo check`
doesn't link, so override them: `--config "target.<t>.ring_core_0_17_14_.rustc-link-lib=[]"`
and, for macOS only, `--config 'target.<t>.hidapi.rustc-cfg=["hidapi"]'`. This type-checks
every line; building and running tests is what the portable CI jobs are for. Details:
`.agent/memory/facts/2026-11-25-cross-check-c-build-scripts.md`.

Accepted advisories live in `.cargo/audit.toml`, each with its reason. All of them come
from GPUI's dependencies, as in aura. gitleaks false positives go in `.gitleaksignore`
(fingerprint plus reason).

`just pre-pr` (`scripts/pre-pr.sh`, ported from aura) runs everything CI runs, locally. It
formats with `cargo fmt` automatically and only checks the rest. Linters that aren't
installed are skipped with an install hint. `just verify` stays the minimum gate for agents.

### Ported from aura for the public release

- **Release matrix** (Linux, macOS, Windows), the remote `install.sh` (`curl | bash`) and
  `scripts/install.ps1` / `scripts/uninstall.ps1`, with aura's closing sponsor prompt. macOS
  and Windows ship only the `detentctl` preview. Contract: [release.md](release.md).
- `just pre-pr` and CI's lint job shellcheck the root `install.sh` as well as `scripts/*.sh`.

### Not ported from aura

- **CodeQL**: wasn't available while the repo was private (needs GitHub Advanced Security).
  Now that it is public, aura's `codeql-analysis.yml` can be added.
- **`.app` bundling, launchd agent, Windows shortcuts**: the preview ships a CLI only.

`.github/FUNDING.yml` (as in aura) points the Sponsor button at GitHub Sponsors, Pix and
the crypto addresses in `SPONSOR.md`.

## File-size ratchet

`scripts/check-file-size.sh` fails when a `.rs` file has more than 600 lines or a `.md` file
more than 400 lines. It checks tracked and untracked, non-ignored files and skips `vendor/`,
`target/` and `Cargo.lock`. Files that were already over the cap are listed in
`scripts/file-size-allowlist.txt` as `<max lines> <path>`, using their line count when they
were added. They may shrink but never grow. An entry that is no longer needed (the file is
back under the default cap, or deleted) also fails, so the list only gets shorter.
`--update` rewrites the list with current counts. It lowers caps but never raises one. Runs
in `just lint`, `just pre-pr` and the CI core job. The list has been empty since
2026-11-24 (ADR 0008). The policy is in `.agent/context/conventions.md` ("File size").

Functions are capped at 100 lines by `clippy::too_many_lines` (threshold in `clippy.toml`,
enabled in `[workspace.lints.clippy]`), so the clippy step of both CI jobs enforces it.
Functions that predate the rule carry `#[expect(clippy::too_many_lines, ...)]`.

## Bots

- **Dependabot** (`.github/dependabot.yml`): weekly on Monday, 06:00 America/Sao_Paulo.
  Cargo minor and patch updates come as one grouped PR, labelled `dependencies` +
  `security`. GitHub Actions updates come as one grouped PR, labelled `dependencies` + `ci`.
  `gpui-pre` and `gpui-pre-platform` are ignored on purpose: they are pinned exactly (ADR
  0005) and are bumped by hand after checking the radial menu on the real desktop.
- **Dependabot alerts + security updates**: enabled.
- **CODEOWNERS**: `* @Rfluid`.
- Issue forms: bug report (asks for version, device, connection, kernel, desktop, install
  method and logs), feature/device request. Blank issues are off. PR template:
  `.github/pull_request_template.md` (the definition-of-done checklist).

## Repository settings

Declared in `.github/settings.yml` (aura's Probot format) and applied with `gh api`:

```sh
R=repos/Rfluid/detent
gh api -X PATCH $R -F allow_merge_commit=false -F allow_squash_merge=true \
  -F allow_rebase_merge=true -F delete_branch_on_merge=true -F allow_update_branch=true \
  -f squash_merge_commit_title=PR_TITLE -f squash_merge_commit_message=PR_BODY -F has_wiki=false
gh api -X PUT $R/vulnerability-alerts
gh api -X PUT $R/automated-security-fixes
gh api -X PUT $R/actions/permissions/workflow -f default_workflow_permissions=read
```

What's live today:

- PR merges: squash (PR title and body) or rebase. No merge commits. Head branches are
  deleted after merge, and the "update branch" button is on.
- Labels: GitHub's defaults plus `security`, `ci` and `dependencies` (as in aura), and the
  platform/area labels in `.github/settings.yml` (`platform:*`, `area:*`, `help wanted`,
  `good first issue`). The README's platform table links to their issue filters.
- The default `GITHUB_TOKEN` is read-only. Only `release.yml` asks for `contents: write`.
- Community files: `SECURITY.md` (reports go through private vulnerability reporting),
  `CODE_OF_CONDUCT.md`, `SPONSOR.md`, issue forms including a platform-port form.
- **Turn on after going public** (they were refused while the repo was private on a free
  plan): private vulnerability reporting, secret scanning and push protection, and branch
  protection. gitleaks keeps running in CI either way.

```sh
gh api -X PUT repos/Rfluid/detent/private-vulnerability-reporting
gh api -X PATCH repos/Rfluid/detent --input - <<'EOF'
{"security_and_analysis":{"secret_scanning":{"status":"enabled"},
 "secret_scanning_push_protection":{"status":"enabled"}}}
EOF
```

The intended protection for `main` is in `.github/settings.yml`: the four required checks
above (strict), 1 approving review with stale reviews dismissed, conversations resolved,
linear history, no force pushes or deletions, and **`enforce_admins: false`** so the owner
can still push locally merged work straight to `main`. Apply it with:

```sh
gh api -X PUT repos/Rfluid/detent/branches/main/protection --input - <<'EOF'
{"required_status_checks":{"strict":true,"contexts":["Core - Format, Clippy, Tests & File Size",
 "GPUI Plugin - Clippy & Tests","Dependency Security Scan (cargo-audit)","Secret Detection (gitleaks)"]},
 "enforce_admins":false,"required_pull_request_reviews":{"required_approving_review_count":1,
 "dismiss_stale_reviews":true},"restrictions":null,"required_linear_history":true,
 "allow_force_pushes":false,"allow_deletions":false,"required_conversation_resolution":true}
EOF
```

If you rename a CI job, update the check names here, in `.github/settings.yml` and in the
live protection.
