---
title: Glossary
status: current
version: 0.1.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: []
owner: "@rfluid"
tags: [context]
---

# Glossary

| Term | Meaning |
|---|---|
| **HID++ 2.0** | Logitech's vendor protocol over HID reports; capabilities are *features* identified by 16-bit IDs |
| **Feature index** | Per-device slot a feature ID maps to; resolved via Root/FeatureSet |
| **swid** | 4-bit software ID echoed in responses; 0 = notification; kernel = 1; Detent = 0xD |
| **Device index** | 0xFF for direct (BT/USB), 1–6 behind a receiver |
| **CID** | Control ID of a button/control in `0x1B04` |
| **Divert** | Ask the firmware to send a control's events to software (HID++ notification) instead of as normal HID input |
| **Raw XY** | Diverting pointer motion of a held control as dx/dy notifications (cursor stops) |
| **Data plane** | Per-device thread running the event path; allocation/lock/async-free |
| **Control plane** | Async runtime for config, reconciler, IPC, plugin supervision |
| **Binding table** | Compiled `[layer][slot][edge] → Action` array |
| **Slot** | Dense index of a bindable control in the binding table |
| **Modifier / layer** | Held controls form a mask; the mask selects a layer of bindings |
| **Chord** | Modifier(s) held + another control, e.g. `gesture + left` |
| **Grab-while-armed** | `EVIOCGRAB` on the mouse's evdev node only while a chord modifier is held, to swallow non-divertable clicks |
| **Desired / reported** | What config + edits want vs what the device last reported |
| **Reconciler** | Brings reported to desired; full resync after reconnect |
| **Plugin bus** | SOCK_SEQPACKET protocol between `detentd` and plugin processes |
| **Native plugin** | First-party plugin in `plugins/`, registered in `config/src/native.rs`, uses the public bus |
| **Action namespace** | `<plugin>.<action>` names any binding, menu item or plugin can invoke |
| **Lease** | Time-bounded exclusive routing of controls to one plugin |
| **Escape chord** | left + right held 2 s: revokes every lease; not rebindable |
| **Context** | Key/values published by plugins (e.g. `app`) used to match profiles |
| **Layer-shell** | Wayland protocol (`zwlr_layer_shell_v1`) for overlays/panels; needed by the radial menu |
