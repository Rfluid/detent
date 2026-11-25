---
title: "KDE integration and GPUI layer-shell (research appendix)"
status: current
version: 0.1.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: []
owner: "@rfluid"
tags: [research, prior-art]
---

# Prior art: KDE integration and GPUI layer-shell for Detent

Researched 2026-11-23. Owner machine (checked live, read-only): **Plasma 6.8.3 / KWin 6.8.3, Wayland**, `KDE_SESSION_VERSION=6`.
Source snapshots (shallow clones under `/tmp/detent-research/src/`):

| Repo | Commit |
|---|---|
| plasma/kwin (master) | `2b31310fb0b9b9199f7a2c569157cb48cc8d2a8f` |
| plasma/plasma-pa | `4e269ad200ffe12c393e418744204e5cf03ae081` |
| plasma/powerdevil | `7ad6df72dee7ef89eab38357a871c629320bef07` |
| xremap/xremap | `5bb2b582cd603d3467b61cd8aca259898d113ec3` |
| jinliu/kdotool | `be03ce90c09350898556436bac74ed35fe928617` |
| zed-industries/zed (main, sparse: gpui, gpui_linux, gpui_platform) | `4aa53044fd5c2e98058a8e5b893b58d34b069c70` |

URL shorthands: `KW` = `https://invent.kde.org/plasma/kwin/-/blob/2b31310fb0b9b9199f7a2c569157cb48cc8d2a8f`, `PA` = `https://invent.kde.org/plasma/plasma-pa/-/blob/4e269ad200ffe12c393e418744204e5cf03ae081`, `PD` = `https://invent.kde.org/plasma/powerdevil/-/blob/7ad6df72dee7ef89eab38357a871c629320bef07`, `XR` = `https://github.com/xremap/xremap/blob/5bb2b582cd603d3467b61cd8aca259898d113ec3`, `KD` = `https://github.com/jinliu/kdotool/blob/be03ce90c09350898556436bac74ed35fe928617`, `ZED` = `https://github.com/zed-industries/zed/blob/4aa53044fd5c2e98058a8e5b893b58d34b069c70`.

KWin master is slightly ahead of 6.8.3, so every shortcut name below was also checked on the owner's machine with `busctl --user call org.kde.kglobalaccel /component/<c> org.kde.kglobalaccel.Component shortcutNames`. Status: **VERIFIED on device.**

---

## A. KDE integration (`detent-kde`, zbus)

### A1. kglobalaccel D-Bus API

- Service `org.kde.kglobalaccel`, registry object `/kglobalaccel`, interface `org.kde.KGlobalAccel`. `allComponents() -> ao` lists component object paths. On the owner's machine it returns 22, including `/component/kwin`, `/component/kmix`, `/component/org_kde_powerdevil`, `/component/plasmashell`, `/component/mediacontrol` and `/component/ksmserver`.
- Per component: `/component/<name>`, interface `org.kde.kglobalaccel.Component`. Introspected live:
  - `invokeShortcut(s)` and `invokeShortcut(ss)` (name, context)
  - `shortcutNames() -> as` and `shortcutNames(s) -> as`
  - `allShortcutInfos() -> a(ssssssaiai)`
  - `getShortcutContexts`, `isActive`, `cleanUp`
  - properties `friendlyName`, `uniqueName`
  - signals `globalShortcutPressed/Released/Repeated (ssx)`
- **Prior art for invoking:** plasma-pa's own applet calls `invokeShortcut` on `/component/kmix` (`PA/src/qml/globalservice.cpp#L53`). This is the sanctioned pattern. **ADOPT**: the same call KDE itself uses, and it triggers the full handler, OSD included.

#### Verified KWin 6 shortcut names (component `kwin`)

| Action | Exact name | Registered at |
|---|---|---|
| Overview (toggle) | `Overview` | `KW/src/plugins/overview/overvieweffect.cpp#L151` |
| Grid View (toggle) | `Grid View` | `KW/src/plugins/overview/overvieweffect.cpp#L157` |
| Cycle Overview / Grid | `Cycle Overview`, `Cycle Overview Opposite` | `…/overvieweffect.cpp#L138`, `#L144` |
| Present Windows (current desktop) | `Expose` | `KW/src/plugins/windowview/windowvieweffect.cpp#L47` |
| Present Windows (all desktops) | `ExposeAll` | `…/windowvieweffect.cpp#L56` |
| Present Windows (window class) | `ExposeClass`, `ExposeClassCurrentDesktop` | `…#L65`, `#L74` |
| Next / previous desktop | `Switch to Next Desktop`, `Switch to Previous Desktop` | `KW/src/virtualdesktops.cpp#L769-L770` |
| Desktop left / right / up / down | `Switch One Desktop to the Left`, `Switch One Desktop to the Right`, `Switch One Desktop Up`, `Switch One Desktop Down` | `KW/src/virtualdesktops.cpp#L775-L778` |
| Maximize | `Window Maximize` | `KW/src/useractions.cpp#L830` |
| Minimize | `Window Minimize` | `KW/src/useractions.cpp#L838` |
| Close | `Window Close` | `KW/src/useractions.cpp#L828` |
| Quick tile left / right | `Window Quick Tile Left`, `Window Quick Tile Right` (plus Top, Bottom, Top Left, …) | `KW/src/useractions.cpp#L890-L896` |
| Fullscreen | `Window Fullscreen` | `KW/src/useractions.cpp#L852` |
| Peek at desktop | `Show Desktop` | `KW/src/useractions.cpp#L1012` |
| Move window one desktop | `Window One Desktop to the Left/Right` | `KW/src/useractions.cpp#L950-L952` |
| Zoom in / out / reset | `view_zoom_in`, `view_zoom_out`, `view_actual_size` | `KW/src/plugins/zoom/zoom.cpp#L55-L66` (via `KStandardActions::zoomIn/zoomOut/actualSize`, whose object names are the `view_*` standard names; confirmed live) |

Notes:
- `Switch to Next/Previous Desktop` wrap around per KWin's navigation-wrapping setting. "Left/Right" respects the grid layout. Both exist; pick per binding. **ADOPT** both as distinct actions.
- Zoom, Overview and Present Windows are effects that are enabled by default (`"EnabledByDefault": true` in `KW/src/plugins/{zoom,overview,windowview}/metadata.json`). If the user disables the effect, its shortcut disappears from `shortcutNames`. **ADAPT**: `detent-kde` should check `shortcutNames()` at startup and report missing actions instead of failing silently.
- `org.kde.KWin /KWin` also has direct `nextDesktop()`, `previousDesktop()`, `setCurrentDesktop(i)` and `showDesktop(b)` (`KW/src/dbusinterface.h`, interface `org.kde.KWin`). **REJECT** as the primary path: these skip the OSD/animation path shortcuts trigger. Keep them only as a fallback.

### A2. Volume with OSD

- Component: **`kmix`** (the legacy name kept by plasma-pa's kded). Set at `PA/src/kded/audioshortcutsservice.cpp#L202` (`action->setProperty("componentName", u"kmix"_s)`).
- Names (verified live, 9 in total): `increase_volume`, `decrease_volume`, `increase_volume_small`, `decrease_volume_small`, `mute`, `mic_mute`, `increase_microphone_volume`, `decrease_microphone_volume`, plus push-to-talk. Registered at `PA/src/kded/audioshortcutsservice.cpp#L80` (increase_volume), `#L95` (decrease_volume), `#L110`/`#L125` (small), `#L168` (mute), `#L181` (mic_mute).
- **OSD: yes.** The triggered handler calls `changeVolumePercent(sink, volumeStep)` and then `showVolume(percent)` (`audioshortcutsservice.cpp#L83-L91`). `showVolume` calls `m_osdDBusInterface->volumeChanged(percent, max)` on `/org/kde/osdService` (`#L363-L369`), gated by the user's `VolumeOsd` setting. It also plays the feedback sound. So `invokeShortcut("increase_volume")` behaves exactly like the keyboard volume key.
- **Volume step:** `VolumeStep`, default **5** (%) (`PA/src/globalconfig.kcfg#L13-L16`), stored in `~/.config/plasmaparc [General] VolumeStep`. The key is unset on the owner's machine, so the step is 5. The `_small` variants use a 1% step.
- Alternatives:
  - `org.kde.plasmashell /org/kde/osdService` (interface `org.kde.osdService`; live: `showText(ss)`, `volumeChanged(i)`/`(ii)`, `brightnessChanged(i)`, `screenBrightnessChanged(issi(iiii))`, `virtualDesktopChanged(s)`, `mediaPlayerVolumeChanged(iss)`, …). **ADAPT**: use it only to show custom OSD text (for example "DPI 1600" or "SmartShift on") via `showText(icon, text)`. Never use it for volume, because it only draws and does not change anything.
  - `wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+`. **REJECT**: no OSD, no feedback sound, ignores the Plasma step and max-volume (`RaiseMaximumVolume` 150%) settings, and needs a subprocess.

### A3. Brightness

- Component **`org_kde_powerdevil`**. This is the KAboutData component name at `PD/daemon/powerdevilapp.cpp#L101`, which `KActionCollection` inherits. Names (verified live): `Increase Screen Brightness`, `Decrease Screen Brightness`, `Increase Screen Brightness Small`, `Decrease Screen Brightness Small`, `Increase Keyboard Brightness`, `Decrease Keyboard Brightness`, … Registered at `PD/daemon/powerdevilscreenbrightnessagent.cpp#L141-L163`. The handler `actOnBrightnessKey` shows the indicator (OSD). **ADOPT** as the primary path, for consistency with volume.
- Direct API (Plasma 6.2+): service `org.kde.ScreenBrightness`, object `/org/kde/ScreenBrightness`, interface `org.kde.ScreenBrightness`. Methods `AdjustBrightnessStep(u step_action, u flags)`, where step 0=Increase, 1=Decrease, 2=IncreaseSmall, 3=DecreaseSmall and flags `0x1` = SuppressIndicator (so 0 shows the OSD). Also `AdjustBrightnessRatio(d,u)` and per-display `/org/kde/ScreenBrightness/displayN` (`SetBrightness`). See `PD/daemon/dbus/org.kde.ScreenBrightness.xml#L57-L82`. Present live on the owner's machine (`display4`). **ADAPT**: this is the fallback when the shortcut path is missing, and the path to use for "set to X%".
- Legacy `org.kde.Solid.PowerManagement /org/kde/Solid/PowerManagement/Actions/BrightnessControl` (`setBrightness`, `brightness`, `brightnessMax`, …: `PD/daemon/actions/bundled/org.kde.Solid.PowerManagement.Actions.BrightnessControl.xml`) still exists live but is single-display legacy. **REJECT** for new code.
- Caveat: on a desktop with external monitors, brightness works only if DDC/CI is available to PowerDevil. To verify on the owner's monitor (`display4` exists, so probably fine).

### A4. Active window and desktop context on Wayland (KWin script)

**D-Bus scripting API (KWin 6)**, from `KW/src/scripting/scripting.h#L316-L346` and `KW/src/scripting/scripting.cpp`, and introspected live:

- `org.kde.KWin /Scripting`, interface `org.kde.kwin.Scripting`:
  - `loadScript(s path [, s pluginName]) -> i id`
  - `loadDeclarativeScript(...)`
  - `isScriptLoaded(s) -> b`
  - `unloadScript(s pluginName) -> b`
  - `start()`
- Each loaded script is exported at **`/Scripting/Script<id>`** (Plasma 6, `scripting.cpp#L112`). In Plasma 5 it was `/<id>`. Interface `org.kde.kwin.Script` has `run()` and `stop()` (`KW/src/scripting/org.kde.kwin.Script.xml`). `stop()` also unloads the script.
- `loadScript` returns an error/-1 if `pluginName` is already loaded (`scripting.cpp#L773-L783`). So call `isScriptLoaded` → `unloadScript` → `loadScript` → `Script<id>.run()`.

**JS API (KWin 6)** (`KW/src/scripting/workspace_wrapper.h`, `KW/src/window.h`):

- `workspace.windowActivated(Window)` signal (`workspace_wrapper.h#L182`; replaces KWin 5's `clientActivated`) and `workspace.activeWindow` property (`#L64`).
- `window.resourceClass` (`window.h#L191`), `window.caption`, `window.captionChanged`, `window.resourceName`, `window.desktopFileName`.
- `workspace.currentDesktopChanged` (`#L276`; in master the signature is `(previous, current, output)`, so ignore the args and read `workspace.currentDesktop`). `workspace.currentDesktop.name` and `.id` (`KW/src/virtualdesktops.h#L53,#L63`).
- `workspace.cursorPos` + `cursorPosChanged` (`#L153`). This gives the global pointer position, which a Wayland client otherwise cannot get. It is useful for placing the radial menu.
- `callDBus(service, path, interface, method, args…)` with up to 9 args (`KW/src/scripting/scripting.h#L114-L124`).

**xremap (Rust/zbus, MIT). The closest match. ADOPT the pattern.**

- KWin script: `XR/src/client/kde/kwin-script.js#L1-L62`. On `workspace.windowActivated` (`#L49-L55`, with a KDE 5/6 switch on `workspace.windowList`) it calls `callDBus("com.k0kubun.Xremap", "/com/k0kubun/Xremap", "com.k0kubun.Xremap", "NotifyActiveWindow2", caption, resourceClass)`. It also re-sends on `captionChanged` of the active window only (`#L20-L46`).
- The D-Bus server is a zbus `#[interface(name = "com.k0kubun.Xremap")]` on a well-known name (`XR/src/client/kde/kde_client.rs#L33-L36`, `#L121-L126`). It starts the server **before** loading the script, because the script sends a message immediately (`kde_client.rs#L49`).
- Loader: `XR/src/client/kde/plugin_script_handler.rs#L22-L109`. It writes the JS to a temp file, then `loadScript(path, PLUGIN_NAME)` and `run` on `/Scripting/Script{id}`, falling back to `/{id}` for KDE 5 (`#L53-L70`). It also has `isScriptLoaded` and `unloadScript`. Comment `#L92-L94`: "unload is not really usable…" (fragile if other ad-hoc scripts run in between).
- Ad-hoc one-shot scripts (initial active window, close-by-class): `XR/src/client/kde/adhoc_script_handler.rs#L26-L80`. The path is chosen by `KDE_SESSION_VERSION` (`#L37-L43`), and `stop` doubles as unload.

**kdotool (Rust, Apache-2.0 per `KD/Cargo.toml#L7`; ideas only, no code copy). ADAPT one idea.**

- `KD/src/lib.rs#L148-L164`: `loadScript` → `/Scripting/Script{id}` `run` → `stop`.
- `KD/src/templates.rs#L11-L27,#L80`: the script calls back with `callDBus("<unique bus name>", "/", "", "result", …)`. It targets the caller's **unique connection name** with an empty interface, so the tool needs no well-known name. Useful for one-shot queries such as cursor position.
- Requires `KDE_SESSION_VERSION=6` (`KD/src/lib.rs#L68`).

**Recommendation for `detent-kde`:**
- Own a well-known name such as `dev.detent.Kde` with a zbus interface (`WindowActivated(s class, s caption, s desktopFileName)`, `DesktopChanged(s id, s name)`).
- Ship a static JS (embedded via `include_str!`) and write it to `$XDG_RUNTIME_DIR/detent/kwin-context.js`, not `/tmp`.
- `isScriptLoaded("detent-context")` → `unloadScript` → `loadScript` → `run`, with the server started first.
- Re-check on `org.kde.KWin` NameOwnerChanged (KWin restart) and re-load.
- On exit, `unloadScript`.
- Prefer `desktopFileName` (for example `org.kde.dolphin`) for app matching over `resourceClass`, with `resourceClass` as a fallback. This is an inference; xremap matches on class.
- Reject the KWin 5 `/<id>` fallback: Plasma 6 is the target.
- Pitfall: a KDE forum post claims ad-hoc scripts do not work on Plasma 6 Wayland. xremap and kdotool contradict this in practice, and the owner's live introspection shows the methods. Still, **verify with one test load on the device** before relying on it.

Other references:
- https://discuss.kde.org/t/run-kwin-script-from-command-line-with-argument/12984
- https://bugs.kde.org/show_bug.cgi?id=482685 (re-connecting signals inside handlers logs errors on 6.0.1. Avoid connect/disconnect churn; xremap's captionChanged handling does exactly this, so keep it minimal or skip captions.)
- KWin scripting API docs: https://develop.kde.org/docs/plasma/kwin/api/

### A5. Listing and verifying names on the user's machine

- On Fedora 44 the Qt 6 CLI is **`qdbus-qt6`** (package `qt6-qttools`). Neither `qdbus` nor `qdbus6` exists on the owner's machine. `busctl --user` (systemd) and `gdbus` are always present. **ADOPT `busctl`** in docs and scripts, because it is distro-neutral.
- Commands (all verified live):
  ```sh
  busctl --user call org.kde.kglobalaccel /kglobalaccel org.kde.KGlobalAccel allComponents
  busctl --user call org.kde.kglobalaccel /component/kwin org.kde.kglobalaccel.Component shortcutNames
  busctl --user call org.kde.kglobalaccel /component/kwin org.kde.kglobalaccel.Component invokeShortcut s "Overview"
  qdbus-qt6 org.kde.kglobalaccel /component/kmix org.kde.kglobalaccel.Component.shortcutNames
  busctl --user introspect org.kde.KWin /Scripting
  busctl --user introspect org.kde.plasmashell /org/kde/osdService
  ```
- `detentctl` could expose `detentctl kde shortcuts [component]`, which wraps `allComponents` + `shortcutNames`. **ADOPT** as a diagnostics aid.

---

## B. GPUI Wayland layer-shell

### B1. Upstream status

- **Merged:** zed-industries/zed PR **#35610** "gpui: Implement support for wlr layer shell" by happenslol. Merged 2025-10-29 by Cole Miller, merge commit **`b92664c52d7c49bb49acd06d2d40eb23b4e8c601`**. https://github.com/zed-industries/zed/pull/35610. It re-lands the earlier #32651 (reverted) and renames `WindowKind::Overlay` to `WindowKind::LayerShell`. History: https://git.secluded.site/zed/commit/c110f7801516a1948ade4a51213f1fc8ea7f8efc
- **crates.io `gpui` is still 0.2.2** (published 2025-10-22, *before* the merge). There is no newer release (checked via the crates.io API on 2026-11-23). So **the `gpui` crate on crates.io does NOT have layer-shell**, which matches `vendor/README.md`.
- API on current main (`ZED/crates/gpui/src/platform/layer_shell.rs#L1-L83`):
  ```rust
  pub enum Layer { Background, Bottom, Top, #[default] Overlay }
  bitflags! { pub struct Anchor: u32 { TOP=1; BOTTOM=2; LEFT=4; RIGHT=8 } }
  pub enum KeyboardInteractivity { None, Exclusive, #[default] OnDemand }
  pub struct LayerShellOptions {
      pub namespace: String, pub layer: Layer, pub anchor: Anchor,
      pub exclusive_zone: Option<Pixels>, pub exclusive_edge: Option<Anchor>,
      pub margin: Option<(Pixels, Pixels, Pixels, Pixels)>, // top,right,bottom,left
      pub keyboard_interactivity: KeyboardInteractivity,
  }
  pub struct LayerShellNotSupportedError; // open_window errors, no fallback
  ```
  - `WindowKind::LayerShell(LayerShellOptions)` is behind `#[cfg(all(target_os="linux", feature="wayland"))]` (`ZED/crates/gpui/src/platform.rs#L2958`).
  - Runtime setters on `Window`: only `set_exclusive_zone` and `set_exclusive_edge` (`ZED/crates/gpui/src/window.rs#L2851,#L2860`).
  - Example: `ZED/crates/gpui/examples/layer_shell.rs#L75-L100` (clock bar, `KeyboardInteractivity::None`, transparent background).
  - It binds `zwlr_layer_shell_v1` versions 1..=5 (`ZED/crates/gpui_linux/src/linux/wayland/client.rs#L310`).
- **Upstream main moved the platform code into separate crates** (`crates/gpui_linux`, `crates/gpui_platform`, with `publish = false`). A future re-vendor from git is a multi-crate job, not a single-crate drop-in.
- Open follow-ups, not merged:
  - #48466 fixes sizing for surfaces anchored to opposite edges (send 0 and create the renderer lazily). Open since 2026-02-05. https://github.com/zed-industries/zed/pull/48466
  - #61420 adds `Window::set_keyboard_interactivity`. Open since 2026-07-21. https://github.com/zed-industries/zed/pull/61420
  - #65055 adds session-lock (proposed). https://github.com/zed-industries/zed/pull/65055
  - Issue #46378: `displays()` is empty at startup on Wayland, which breaks output selection. https://github.com/zed-industries/zed/issues/46378

### B2. Forks and republished snapshots that include layer-shell (checked by downloading crates)

| Crate | Version / date | Owner | Layer-shell? | Notes |
|---|---|---|---|---|
| `gpui-pre` | 0.3.14, 2026-11-23 | huacnlee (Longbridge, author of gpui-component) | **yes** (`src/platform/layer_shell.rs`, `examples/layer_shell.rs`) | "gpui-pre snapshot of zed@f126950", Apache-2.0. Ships with `gpui-pre-linux` and `gpui-pre-platform` 0.3.14. It is what **gpui-component 0.11.0** depends on (`gpui-pre =0.3.14`). |
| `gpui-ce` | 0.3.0, 2026-10-20 | philocalyst | **yes** | Community fork, Apache-2.0, plus `gpui_ce_linux`. Versioning is erratic (0.4.8 and 0.3.3 were published earlier). https://github.com/gpui-ce/gpui-ce |
| `rgpui` | 1.8.2 | launcher-rs | yes (example `layer_shell.rs`) | Smaller fork. |
| `gpui-standalone`, `open-gpui` | — | — | yes (docs.rs `layer_shell` module) | Low adoption. |

Projects using GPUI layer-shell: GPUi Shell (bar for Hyprland/Niri), https://andre-brandao.github.io/gpui-shell, and the gpuiv/gpuix wrappers.

**Recommendation.** The choice is between three options:
- **ADAPT** (preferred): switch the vendored GPUI from crates.io `gpui 0.2.2` to **`gpui-pre 0.3.14`**, either via `package = "gpui-pre"` rename or by vendoring it. That gets upstream's merged layer-shell code without writing patch #1, and it tracks a maintained snapshot (gpui-component's).
- Pin `zed` by git rev `b92664c5…` or later. This drags the whole Zed workspace.
- Backport #35610 onto the vendored 0.2.2 as `DETENT PATCH #1`. The diff is moderate: `layer_shell.rs`, the Wayland `window.rs` surface-state enum and the client bind.

Because ADR 0005 and `vendor/README.md` already chose vendoring, record the decision there. **REJECT** writing our own layer-shell patch from scratch.

### B3. KWin layer-shell behaviour (affects the radial menu)

- KWin implements `zwlr_layer_shell_v1` **version 5** (`KW/src/wayland/layershell_v1.cpp#L22`). It is confirmed live: `wayland-info` shows `zwlr_layer_shell_v1 version 5`. KWin first added it in Sept/Oct 2020 (MR https://invent.kde.org/plasma/kwin/-/merge_requests/202, kwayland-server commit 2020-09-24), so Plasma 5.20 (inferred from dates).
- **Keyboard interactivity is effectively boolean in KWin.** `set_keyboard_interactivity` stores `acceptsFocus = keyboard_interactivity` (`KW/src/wayland/layershell_v1.cpp#L244-L246`), so `OnDemand` and `Exclusive` behave the same. For Top/Overlay layers, KWin **immediately activates** a surface that accepts focus (`KW/src/layershellv1window.cpp#L309-L321`). Consequences:
  - A radial menu with GPUI's **default `OnDemand` would steal focus** from the app the user is acting on. That would change the active window, so KWin shortcuts like "Window Close" would hit the menu, and `windowActivated` would fire with the overlay.
  - **Use `KeyboardInteractivity::None`.** Escape-to-cancel then comes from the daemon (mouse-button release) rather than the keyboard.
- Pointer: a layer surface is a normal `wl_surface`, so it gets `wl_pointer.enter` with surface-local coordinates and motion while the pointer is over it. **Not verified:** whether KWin sends `enter` when a surface maps under a stationary pointer before any motion. Test this on the device.
- Recommended design, which does not rely on `enter`:
  - The daemon already knows the gesture (HID++ diverted button plus raw XY or rel motion), so it drives the selection over the plugin bus.
  - The overlay is a fixed-size surface anchored `TOP|LEFT`, with `margin` = cursor position minus half the size.
  - The cursor position comes from the KWin script (`workspace.cursorPos`) via `detent-kde`, or the overlay is full-screen (all four anchors) and reads the first `enter`/`motion`.
  - Full-screen anchoring needs the #48466 sizing fix or explicit output size, and hits #46378 at startup. That is another reason for the fixed-size-plus-margin approach.

### B4. Fallback without GPUI (SCTK layer shell + tiny-skia/softbuffer)

- Stack:
  - `smithay-client-toolkit` 0.21.1 (MIT): `shell::wlr_layer::{LayerShell, LayerSurface, Anchor, KeyboardInteractivity, Layer}` and its `examples/simple_layer.rs`.
  - `tiny-skia` 0.12.0 (BSD-3) for anti-aliased paths and fills.
  - `wl_shm` buffers via SCTK's `SlotPool` (no need for softbuffer).
  - Text via `cosmic-text` or `ab_glyph` if labels are needed. Icons as pre-rasterised PNG or SVG through `resvg`.
- Cost: about 400-700 LOC for one radial menu (ring segments, hover highlight, labels or icons, fade-in), plus HiDPI (`wp_fractional_scale_v1` / buffer scale). No GPU and no Vulkan init, so the first frame comes in well under 16 ms. CPU raster of a ~400×400 px surface per frame is about 1 ms. Binary and dependency footprint is a fraction of GPUI's.
- Benefit: no fork maintenance, starts instantly (GPUI/wgpu init can take 100-300 ms; measure that), full protocol control (`KeyboardInteractivity::None`, input region, `set_layer`).
- Downside: no layout or text system, no styling reuse with any other GPUI UI (settings app). Animations are hand-rolled.
- Prior art: SCTK's `simple_layer` example. `layer-shika` 0.3.1 (codeberg, layer-shell + Slint) is another option.
- Verdict: **ADAPT as a fallback.** Keep the radial renderer behind a small trait so a SCTK + tiny-skia backend can replace GPUI if gpui-pre or the vendoring cost or startup latency proves bad. The menu is simple enough that the fallback is cheap insurance. Benchmark GPUI cold-start for the overlay before committing.

---

## Items still marked "to verify on device"

1. Load and run a test KWin script via D-Bus on 6.8.3 (`/Scripting/Script<id>` `run`), and confirm the `callDBus` callback arrives.
2. `invokeShortcut("increase_volume")` on `/component/kmix` shows the OSD (expected from source; harmless to test).
3. Brightness via `org_kde_powerdevil` on the owner's external monitor (DDC/CI).
4. A GPUI (gpui-pre) layer surface on KWin with `KeyboardInteractivity::None`: no focus steal; whether pointer `enter` arrives without motion.
5. GPUI overlay cold-start latency vs the SCTK fallback.
