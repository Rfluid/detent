---
title: Native plugins
status: draft
version: 0.1.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: ["plugins/", "crates/config/src/native.rs", "crates/plugin-sdk/"]
owner: "@rfluid"
tags: [plugins, native]
---

# Native plugins

First-party plugins that live in this repo and ship with Detent.

| Dir | Binary | Namespace | What it does | Spec |
|---|---|---|---|---|
| `kde/` | `detent-kde` | `kde.*` | KDE Plasma integration: KWin actions from gestures and wheels, active-app context for profiles, OSD | [docs/plugins/kde.md](../docs/plugins/kde.md) |
| `radial/` | `detent-radial` | `radial.*` | On-screen radial menu (GPUI, Wayland layer-shell), selection by click or hold-release, haptic ticks | [docs/plugins/radial.md](../docs/plugins/radial.md) |

## The contract

Native plugins are not special at runtime:

1. **Same bus, same SDK.** They depend on `detent-plugin` and speak the public plugin bus protocol.
   No private hooks into the daemon. If a native plugin needs something, add it to the protocol so
   third-party plugins get it too.
2. **Registered in `crates/config/src/native.rs`.** This lets users enable them with just
   `[plugins.<name>]` (no `exec`) and gives them default capabilities.
3. **Built and installed with the workspace.** The binary is installed next to `detentd`.
4. **Own their settings.** Everything under `[plugins.<name>]` except `enable`, `exec` and `caps`
   is passed to the plugin, which validates it.
5. **Documented** in `docs/plugins/<name>.md` and covered by tests that run against the SDK's mock daemon.

Add a new one: [`.agent/skills/add-native-plugin.md`](../.agent/skills/add-native-plugin.md).
