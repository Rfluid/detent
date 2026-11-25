---
title: Documentation index
status: current
version: 0.1.0
last_updated: 2026-11-25
last_verified: 2026-11-23
source_refs: []
owner: "@rfluid"
tags: [index]
---

# Detent documentation

| Area | Doc | What's in it |
|---|---|---|
| **Product** | [product/spec.md](product/spec.md) | Vision, users, use cases, requirements, non-goals |
| | [product/roadmap.md](product/roadmap.md) | Phases, deliverables, acceptance criteria |
| **Architecture** | [architecture/overview.md](architecture/overview.md) | Planes, threads, crates, data flow |
| | [architecture/data-plane.md](architecture/data-plane.md) | The hot path, frame by frame |
| | [architecture/device-state.md](architecture/device-state.md) | Desired vs reported state, reconciler |
| | [architecture/bindings.md](architecture/bindings.md) | Binding table, chords, layers, gestures, grab-while-armed |
| | [architecture/plugins.md](architecture/plugins.md) | Plugin model, native vs third-party, leases |
| | [architecture/performance.md](architecture/performance.md) | Latency budget, rules, benchmarks |
| **Specs** | [specs/hidpp.md](specs/hidpp.md) | HID++ 2.0 notes, features used, things to verify |
| | [specs/config.md](specs/config.md) | Config file reference + trigger grammar |
| | [specs/plugin-bus.md](specs/plugin-bus.md) | Plugin bus protocol (normative) |
| | [specs/ipc.md](specs/ipc.md) | Client JSON-RPC API |
| | [specs/transport.md](specs/transport.md) | HID++ transport and discovery per OS (hidraw, hidapi) |
| **Plugins** | [plugins/authoring.md](plugins/authoring.md) | Writing a plugin |
| | [plugins/kde.md](plugins/kde.md) | Native: KDE integration |
| | [plugins/radial.md](plugins/radial.md) | Native: radial menu |
| **Decisions** | [adr/](adr/) | Architecture decision records |
| **Development** | [dev/setup.md](dev/setup.md), [dev/testing.md](dev/testing.md) | Machine setup, test strategy |
| | [dev/ci-and-release.md](dev/ci-and-release.md) | CI jobs, file-size ratchet, bots, GitHub repo settings |
| | [dev/release.md](dev/release.md) | Cutting a release, release asset naming, install script contract |

Every doc carries YAML frontmatter. `source_refs` lists the code it describes. Change that code, update the doc.
