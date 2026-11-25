---
title: Implement a roadmap phase
status: current
version: 0.1.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: ["docs/product/roadmap.md"]
owner: "@rfluid"
tags: [workflow]
---

# Implement a roadmap phase

1. **Load context:** AGENTS.md → `.agent/context/*` → the phase in `docs/product/roadmap.md` → the architecture/spec docs it touches → `.agent/memory/INDEX.md`.
2. **Plan:** list deliverables as small, independently testable steps in dependency order. Identify hot-path steps (use `.agent/skills/hot-path-change.md`) and ⚠ facts that need device verification first.
3. **Settle unknowns first:** check `docs/research/prior-art.md` (and extend it if this phase raises new questions), then verify device facts (feature IDs, CIDs, divertability) and desktop facts (D-Bus names) on the machine before code depends on them. Apply the decision policy in AGENTS.md: decide standard questions yourself and record them.
4. **Build step by step:** for each step: code, tests, `just check-core`/`just test`, commit (`type(scope): ...`).
5. **Docs as you go:** update specs, remove ⚠ where verified, bump `last_verified`.
6. **Acceptance:** the phase's result must be *usable by the owner on their mouse*. Run the acceptance checks on the device (`.agent/skills/verify-on-device.md`). Where a human is needed, write exact manual steps and expected results, and keep `docs/user/quickstart.md` current so the owner can do it without you.
7. **Close out:** `just verify` green; roadmap marks the phase done and points to the next; `.agent/context/stack.md` "Current state" updated; memory entries added.
