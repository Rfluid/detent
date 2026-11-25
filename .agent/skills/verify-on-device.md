---
title: Verify on the real device
status: current
version: 0.1.0
last_updated: 2026-11-24
last_verified: 2026-11-24
source_refs: ["docs/product/roadmap.md", "packaging/udev/70-detent.rules"]
owner: "@rfluid"
tags: [skill, testing, device]
---

# Verify on the real device

Device-facing work isn't done until it has been checked on the MX Master 4.

## Preconditions
- `just ctl devices` lists the mouse (find the current hidraw node; it changes).
- Solaar/logiops stopped (`detentd` warns otherwise).
- udev rules installed (`just install`, accept the udev step) if access is denied.
- Testing the installed service (not `just run-daemon`)? `just install` again first: `just verify`
  does not rebuild or reinstall the binaries the service runs from `~/.local/bin`.

## Safety
- Read-only probes first. Never send write functions to unknown features.
- Keep a way out: until leases/grab are proven, test chords and grabs with a timeout wrapper (`timeout 20 just run-daemon`) so a bug can't leave the mouse grabbed.
- Diversion is volatile: power-cycling the mouse (off/on switch) restores defaults.

## Steps
1. Run the phase's acceptance checks from `docs/product/roadmap.md`.
2. Paste the relevant output (feature dumps, CIDs, latencies) into the spec doc (`docs/specs/hidpp.md` or `docs/specs/devices/mx-master-4.md`).
3. Record surprises in `.agent/memory/facts/`.
4. If something can't be verified by an agent (it needs a human to press buttons), say so explicitly and list the exact manual steps for the user, with expected results.
