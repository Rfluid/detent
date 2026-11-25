---
title: GitHub features missing on the private Rfluid/detent repo (free plan)
date: 2026-11-24
source_task: create the GitHub repo, CI and release setup
tags: [github, ci, repo-settings]
status: superseded
superseded_on: 2026-11-25
---

> **Superseded (2026-11-25):** the repo is public from v0.1.0, so the 403/422 limits below no
> longer apply. Branch protection, secret scanning, push protection and private
> vulnerability reporting are switched on after going public with the commands in
> `docs/dev/ci-and-release.md` ("Repository settings"); CodeQL can be added. gitleaks keeps
> running either way. The `gh repo view` TTY note at the end still holds. Kept as history.

Verified with `gh api` on 2026-11-24:

- `PUT /repos/Rfluid/detent/branches/main/protection` and `POST .../rulesets` return 403:
  "Upgrade to GitHub Pro or make this repository public". So `main` has **no** branch
  protection. The intended rules are in `.github/settings.yml`; the command to apply them
  is in `docs/dev/ci-and-release.md`.
- Enabling secret scanning returns 422 "not available for this repository". gitleaks runs
  in CI instead (`secret-scanning.yml`).
- CodeQL code scanning on a private repo needs GitHub Advanced Security, so it's not set up.
- These work: Dependabot alerts and security updates, a read-only default `GITHUB_TOKEN`,
  merge-method settings, deleting branches after merge.
- `gh repo view` hangs in a non-TTY agent shell (it sends terminal queries). Use
  `gh api ... </dev/null | cat` instead.
