---
title: Device state and reconciler
status: draft
version: 0.1.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: ["crates/device/", "crates/hidpp-features/"]
owner: "@rfluid"
tags: [architecture, state]
---

# Device state and reconciler

Goal: **everything about the device is visible and editable**, and stays correct across sleep, reconnect and host switches.

## Model

`DeviceState` (crates/device/src/state.rs) holds identity, link, the full feature table (including unknown features), and one `Section<T>` per supported feature:

```rust
struct Section<T> { desired: Option<T>, reported: Option<T>, generation: u64 }
```

- **desired**: from config plus live edits (IPC `device.set`, plugin `SetState`)
- **reported**: last value read back from the device. `None` means unknown (e.g. after disconnect)
- **generation**: bumped on every change, so subscribers get diffs (`StateDiff { path, value, generation }`)

## Reconciler

```
 desired ──diff──► Feature::apply ──► device ──► Feature::read ──► reported ──► publish diff
   ▲                                                                   │
   └── config reload · detentctl set · plugin SetState     reconnect ──┘ (FullResync)
```

| Trigger | Plan |
|---|---|
| Config reload / live edit | `ApplyDirty`: apply only out-of-sync sections, read them back |
| Link up after sleep / reconnect / host switch | `FullResync`: re-enumerate if firmware changed, read all, apply all desired (incl. diversion) |
| Periodic verify (e.g. every 5 min, cheap reads) | `ApplyDirty` if drift is found |

The reconciler runs on the control plane. Its requests go through the data plane's request ring, so the hidraw fd keeps a single owner.

## Feature modules

Each feature implements `Feature` (crates/hidpp-features/src/lib.rs): `read`, `apply` (only what differs), `on_notification`, and `schema()`. The schema (`Bool`, `Range`, `Choice`, `ReadOnly`) lets `detentctl`, plugins and a future GUI build editors with no per-feature UI code.

## Edits and persistence

- `detentctl set dpi 1600` → desired changes for this session.
- `--persist` also writes the change into the config file (format-preserving edit planned with `toml_edit`). The watcher sees the file change, and the reload is a no-op because desired already matches.
- There's one write path, so the config and the device can't drift apart.

## Identity

Devices are keyed by `(product, serial)`, **never by hidraw path**: the node number changes between connections (observed: hidraw9 → hidraw10 for the same mouse).
