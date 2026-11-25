---
title: One-shot build prompt
status: current
version: 0.2.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: ["AGENTS.md", "docs/product/roadmap.md", ".agent/workflows/research-prior-art.md"]
owner: "@rfluid"
tags: [prompt]
---

# One-shot build prompt

Paste everything below the line into a fresh agent session started in `~/development/Rust/detent`.

---

You are building **Detent**, a Rust userspace daemon that lets me control my Logitech MX Master 4 on Linux (Fedora, KDE Plasma on Wayland; the mouse is on Bluetooth as `046D:B042`; find its current `/dev/hidrawN` with `just ctl devices`, because the number changes). The repo is already scaffolded: architecture, specs, ADRs, agent skills and tested foundation code.

## The goal, and how you'll be judged
**I must be able to install Detent and use it to control my own mouse every day.** Compiling and passing tests isn't enough. Work through the roadmap (`docs/product/roadmap.md`): **Phase R (research), then phases 0 → 5**, each one finished and usable on my mouse before the next. The final gate is the "Daily-driver acceptance" checklist in that file, which I'll run myself using only `docs/user/quickstart.md`. Keep that page accurate as you go.

## Start here (mandatory reading, in order)
1. `AGENTS.md`: rules, commands, and the **decision policy**
2. `.agent/context/stack.md`, `conventions.md`, `glossary.md`
3. `.agent/memory/INDEX.md` and every entry it links
4. `docs/product/spec.md` and `docs/product/roadmap.md`
5. `docs/architecture/*.md`, then the specs in `docs/specs/` and `docs/plugins/` for the phase you're on

Use `.agent/workflows/` for process (`research-prior-art`, `implement-phase`, `change-protocol`) and `.agent/skills/` when a task matches (`hot-path-change`, `add-hidpp-feature`, `add-native-plugin`, `verify-on-device`).

## Phase R: research first
Before writing device code, follow `.agent/workflows/research-prior-art.md`. Study the existing open-source tools: **Solaar, logiops, the kernel's `hid-logitech-hidpp.c`, libratbag, input-remapper/keyd/kanata, existing Rust HID++ crates, Zed/GPUI layer-shell work, KWin scripting/kglobalaccel**, and MX Master 4 issues in their trackers (haptics, CIDs). Write `docs/research/prior-art.md`, cite sources, mark each finding adopt/adapt/reject, and feed what you settle back into the specs and memory. Use what these projects already figured out (function layouts, reconnect handling, gesture/tap-hold resolution, grab/uinput pitfalls) instead of rediscovering it. **License:** Detent is MIT, so learn facts and techniques from GPL code but don't copy it.

## Then build (summary; the roadmap has the details)
- **0:** timed hidraw recv, blocking Requester, `detentctl dump-features` / `dump-controls` on my mouse; haptics discovery (research, then read-only probes).
- **1:** feature modules + `detentctl get`.
- **2:** allocation-free data plane, diversion, uinput, gestures, chords with grab-while-armed, re-apply on reconnect, benches, zero-alloc test; `just install` + udev + systemd user service so it runs from login.
- **3:** config compiler + hot reload, reconciler, JSON-RPC IPC, `detentctl get/set/watch/check/reload`.
- **4:** plugin bus, supervisor, capabilities, leases + escape chord, SDK, native `detent-kde`, per-app profiles.
- **5:** GPUI layer-shell (upstream if it exists, else a minimal patch in `vendor/gpui`), native `detent-radial` (click default, hold-release option), pointer lease, haptic ticks.

## Decisions: don't ask me what the industry already answered
Follow the decision policy in `AGENTS.md`. If the answer is an industry standard, what the prior-art tools do, or follows from our docs, **decide it yourself** and record it in the relevant spec or ADR. That covers XDG paths, udev/uaccess, systemd units, uinput/evdev details, timeouts/retries/backoff, well-known crate choices, CLI naming, log levels, test layout and the like. Ask me only about things I'd notice as a user and that have no standard answer, anything risky for my device or system, or real conflicts between docs. Batch those questions with a recommendation each, and keep working while you wait.

## Hard rules
- **Performance first:** the hot-path rules in `docs/architecture/performance.md` are non-negotiable.
- **Never guess hardware or desktop facts:** settle ⚠ items from research, then verify on this machine; record them in specs and `.agent/memory/facts/`.
- **Safety on my mouse:** read-only probes before writes; no writes to unknown features; run anything that grabs or diverts under `timeout` until lease and grab release are proven. Stopping Detent must leave the mouse exactly as before.
- **Native plugins use only the public bus/SDK.**
- **Keep the lockfile** (seeded from a known-good GPUI resolution); no blanket `cargo update`.
- **No root at runtime.** Steps needing sudo (udev rules): ask me once, batched, and continue meanwhile.
- **Conventions:** no `unwrap`/`expect` outside tests, doc frontmatter current, `type(scope): summary` commits, `just verify` green before each commit.

## When you need my hands
Some checks need me to press buttons or watch the screen. At the end of each phase, give me one numbered list of exact steps with expected results, then continue with what doesn't depend on them.

## Done means
I can complete the daily-driver acceptance checklist from `docs/user/quickstart.md` alone. `just verify` and `just check` pass, docs, `.agent/context/stack.md` and memory reflect reality, and everything is committed. Finish with a short report: what I can do with my mouse now, what still needs my manual confirmation, benchmark numbers against the budget, and known issues.
