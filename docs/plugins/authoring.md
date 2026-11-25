---
title: Writing a plugin
status: draft
version: 0.1.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["crates/plugin-sdk/src/lib.rs", "crates/bus/src/lib.rs"]
owner: "@rfluid"
tags: [plugins, guide]
---

# Writing a plugin

A plugin is a normal program that connects to `detentd` over the plugin bus. Read [architecture/plugins.md](../architecture/plugins.md) for the model and [specs/plugin-bus.md](../specs/plugin-bus.md) for the protocol.

Plugins run on Linux only for now, like the daemon (ADR 0010): on macOS and Windows the
`detent-plugin` crate compiles to an empty library, so gate a plugin's code with
`#[cfg(target_os = "linux")]` if it must build there (the native plugins do).

## Rust (SDK)

```rust
use detent_plugin::{ActionCall, Capability, Context, Event, Manifest, Plugin, Topic};

struct Clicker { count: u32 }

impl Plugin for Clicker {
    fn manifest(&self) -> Manifest {
        detent_plugin::manifest(
            "clicker",
            &[("count", "Count and buzz", false)],
            &[Capability::DeviceHaptics, Capability::EventsSubscribe],
        )
    }

    fn on_config(&mut self, _ctx: &mut Context, settings_json: &str) {
        // settings_json = your [plugins.clicker] table minus enable/exec/caps
    }

    fn on_action(&mut self, ctx: &mut Context, call: &ActionCall) {
        self.count += 1;
        ctx.haptic(1, 50);                       // answer the mouse
        if self.count % 10 == 0 {
            ctx.invoke("kde.overview", "{}");    // call another plugin's action
        }
    }

    fn on_event(&mut self, _ctx: &mut Context, _event: &Event) {}
}

fn main() -> Result<(), detent_plugin::PluginError> {
    detent_plugin::run(Clicker { count: 0 })
}
```

Config:

```toml
[plugins.clicker]
exec = "~/.local/bin/clicker"
caps = ["device.haptics", "events.subscribe"]

[[bind]]
when = "thumb.press"
do = { action = "clicker.count" }
```

## Rules of thumb

- **Callbacks must return quickly.** Do slow work (D-Bus, network) on your own threads. The daemon never waits for you, so slow plugins lose messages.
- **Coalesce continuous input.** Motion and rotation can arrive at hundreds of Hz. Batch to your output rate (e.g. 60 Hz).
- **Leases are temporary.** Request only what you need, release as soon as you're done, and handle `LeaseRevoked` at any time.
- **UI plugins** (GPUI, Qt...) call `detent_plugin::connect` on a background thread and forward messages into the UI loop. See `plugins/radial`.

## Other languages

Pick the JSON codec in `Hello` and speak the protocol directly over the `SOCK_SEQPACKET` socket in `$DETENT_BUS`. Every message is one JSON object per packet using serde's externally tagged enums, e.g. `{"Hello": {...}}`, `{"Device": {"HapticPulse": {"waveform": 1, "intensity": 50}}}`.

## Testing

The SDK will ship a mock daemon (phase 4): feed `ActionInvoked`/`Event`s, assert on `Context::take_outbox()`. Unit-test pure logic as you would anything else (see `plugins/radial/src/geometry.rs`).
