//! Opening the layer-shell surface on the right output, and the startup GPU warm-up.

use std::time::Instant;

use gpui::{
    App, Context, DisplayId, IntoElement, Render, Window, WindowAppearance,
    WindowBackgroundAppearance, WindowHandle, WindowKind, WindowOptions, div, layer_shell::*,
    point, prelude::*, px,
};

use super::{Emit, MenuView};
use crate::menu::Settings;
use crate::state::{At, Ring};

/// Pay the one-time GPU initialisation (~0.7 s) at startup instead of on the first menu:
/// open a 1×1 transparent surface on the background layer and close it once drawn.
pub fn warm_up(cx: &mut App) {
    struct Blank;
    impl Render for Blank {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }
    let w = cx.open_window(
        WindowOptions {
            titlebar: None,
            focus: false,
            window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds {
                origin: point(px(0.), px(0.)),
                size: gpui::size(px(1.), px(1.)),
            })),
            window_background: WindowBackgroundAppearance::Transparent,
            kind: WindowKind::LayerShell(LayerShellOptions {
                namespace: "detent-radial-warmup".to_string(),
                layer: Layer::Background,
                anchor: Anchor::TOP | Anchor::LEFT,
                keyboard_interactivity: KeyboardInteractivity::None,
                ..Default::default()
            }),
            ..Default::default()
        },
        |_, cx| cx.new(|_| Blank),
    );
    if let Ok(w) = w {
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1500))
                .await;
            let _ = w.update(cx, |_, window, _| window.remove_window());
        })
        .detach();
    }
}

/// The GPUI display for a compositor output name. GPUI derives Wayland display UUIDs from
/// the `wl_output` name (`Uuid::new_v5(NAMESPACE_DNS, name)`), the same name KWin reports.
fn display_for(cx: &App, output: &str) -> Option<DisplayId> {
    if output.is_empty() {
        return None;
    }
    let want = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, output.as_bytes());
    cx.displays()
        .into_iter()
        .find(|d| d.uuid().ok() == Some(want))
        .map(|d| d.id())
}

pub fn open(
    cx: &mut App,
    ring: Ring,
    at: Option<At>,
    settings: &Settings,
    emit: Emit,
) -> anyhow::Result<WindowHandle<MenuView>> {
    let layout = settings.layout;
    let system_dark = matches!(
        cx.window_appearance(),
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    );
    let colors = settings.colors(system_dark);
    let labels = settings.labels;
    let tooltip_ms = settings.tooltip_ms;
    let animation_ms = settings.animation_ms;
    // Open on the cursor's output; without it the compositor picks (KWin: the active one).
    let display_id = at.as_ref().and_then(|a| display_for(cx, &a.output));
    if let (Some(a), None) = (&at, display_id) {
        eprintln!(
            "detent-radial: output `{}` not found; letting the compositor choose",
            a.output
        );
    }
    // The output's logical size when KWin reported it, else 0×0. Both fill the output
    // (all four edges are anchored), but 0×0 kills the connection on a fractional-scale
    // output: GPUI applies the scale event that arrives before the first configure by
    // calling `wp_viewport.set_destination(0, 0)`, which KWin rejects as a protocol error
    // ("invalid destination size"). A real size keeps that call valid.
    let (w, h) = at.as_ref().and_then(|a| a.size).unwrap_or((0., 0.));
    let center = at.map(|a| point(px(a.x), px(a.y)));
    cx.open_window(
        WindowOptions {
            titlebar: None,
            focus: true,
            display_id,
            app_id: Some("detent-radial".to_string()),
            // The output's size, or 0×0 = "the compositor decides" (wlr-layer-shell
            // `set_size`): with all four edges anchored both are the whole output. Without
            // bounds GPUI requests its default window size (1536×1095) and KWin honours it,
            // so the overlay doesn't cover the output and its centre isn't the output's.
            window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds {
                origin: point(px(0.), px(0.)),
                size: gpui::size(px(w), px(h)),
            })),
            window_background: WindowBackgroundAppearance::Transparent,
            kind: WindowKind::LayerShell(LayerShellOptions {
                namespace: "detent-radial".to_string(),
                layer: Layer::Overlay,
                anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                exclusive_zone: Some(px(-1.)),
                // Modal while open: Esc / Tab / Enter reach the ring (KWin hands focus
                // back to the previous window when the surface goes away).
                keyboard_interactivity: KeyboardInteractivity::Exclusive,
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            cx.new(|_| MenuView {
                ring,
                highlighted: None,
                highlighted_at: Instant::now(),
                layout,
                colors,
                labels,
                tooltip_ms,
                animation_ms,
                shown_at: None,
                leaving: None,
                swapped: false,
                closing: None,
                center,
                drawn: None,
                hover: None,
                gauge: None,
                pill_rect: None,
                pill_hot: false,
                fan_opened_at: None,
                fan_leaving: None,
                fan_centers: Vec::new(),
                fan_d: 0.,
                hover_choice: None,
                scroll_acc: (0., 0.),
                focus,
                emit,
                backdrop: None,
            })
        },
    )
}
