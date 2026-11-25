//! Menu model and settings, parsed from `[plugins.radial]` (delivered by the daemon as JSON).
//! Spec: docs/plugins/radial.md ("Settings").
//!
//! Parsing resolves icons (file system lookups), so it runs on the bus thread, never on
//! the UI thread.

mod parse;
#[cfg(test)]
mod tests;
mod theme;

use crate::icon::{self, Icon};

pub use theme::{Colors, Theme, parse_color};

/// When item names are drawn next to the bubbles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Labels {
    /// Never: icons only.
    Never,
    /// A tooltip next to the highlighted bubble once it has been highlighted for
    /// `tooltip_ms` (default).
    #[default]
    Hover,
    /// Under every bubble.
    Always,
}

/// One step of what an item does: an action invocation or a pause (macros).
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// `(action name, params JSON)` for a bus `Invoke`.
    Invoke(String, String),
    DelayMs(u32),
}

/// An adjustable item (`adjust = "kde.volume"`): hovering shows a gauge, the wheel changes
/// the value. `source` names a provider that answers `<source>.get` / `.set` / `.mute`
/// with `{ source, seq, percent, muted, max, label }` (docs/plugins/radial.md, "Adjustable
/// items").
#[derive(Debug, Clone, PartialEq)]
pub struct Adjust {
    pub source: String,
    /// Percent per main-wheel notch / per thumbwheel step.
    pub step: u32,
    pub fine_step: u32,
    /// Upper limit in percent (100; up to 150 like Plasma's "raise maximum volume").
    pub max: u32,
    /// Flip the thumbwheel's direction.
    pub invert_fine: bool,
    /// Glyph shown while muted (resolved with the item's icon).
    pub muted_icon: Option<Icon>,
}

impl Adjust {
    pub const MAX_LIMIT: u32 = 150;
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemAction {
    /// Any action in the shared namespace (`kde.overview`, ...), with JSON params.
    Action(String, String),
    Exec(String),
    Keys(String),
    /// Launch a desktop application by its desktop-file id (`org.kde.konsole`).
    App(String),
    /// Open a URL, file or folder with the default handler (`xdg-open`).
    Open(String),
    /// Several steps in order (`steps = [{ keys = "Ctrl+A" }, { delay_ms = 50 }, ...]`).
    Macro(Vec<Step>),
    /// A folder: selecting it shows these items as a ring; the centre goes back.
    Folder(Vec<Item>),
    /// A value set with the wheel (volume); selecting it pins its gauge.
    Adjust(Adjust),
}

/// `'...'` for `/bin/sh -c`.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

impl ItemAction {
    /// What selecting the item runs. Empty for folders (they navigate instead) and
    /// adjustable items (they pin their gauge).
    pub fn steps(&self) -> Vec<Step> {
        let exec = |cmd: String| {
            Step::Invoke("exec".into(), serde_json::json!({ "cmd": cmd }).to_string())
        };
        match self {
            Self::Action(a, p) => vec![Step::Invoke(a.clone(), p.clone())],
            Self::Exec(c) => vec![exec(c.clone())],
            Self::Keys(k) => vec![Step::Invoke(
                "keys".into(),
                serde_json::json!({ "keys": k }).to_string(),
            )],
            Self::App(id) => vec![exec(format!("gtk-launch {}", sh_quote(id)))],
            Self::Open(target) => vec![exec(format!(
                "xdg-open {}",
                sh_quote(&icon::expand_home(target))
            ))],
            Self::Macro(steps) => steps.clone(),
            Self::Folder(_) | Self::Adjust(_) => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub label: String,
    pub icon: Icon,
    pub action: ItemAction,
    /// Per-item overrides of the theme's bubble / accent fill.
    pub color: Option<u32>,
    pub accent: Option<u32>,
}

/// Bubble geometry in logical px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    /// Distance from the cursor to each bubble's centre.
    pub ring: f32,
    pub bubble: f32,
    pub center: f32,
}

impl Layout {
    /// Actions Ring proportions: bubble ≈ 0.57 × ring radius, centre ≈ 0.6 × bubble.
    fn for_ring(ring: f32) -> Self {
        Self {
            ring,
            bubble: (ring * 0.57).round(),
            center: (ring * 0.34).round(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    pub items: Vec<Item>,
    /// Per-application rings: `(app id, items)`, matched against the `app` context.
    pub apps: Vec<(String, Vec<Item>)>,
}

/// Does a configured app key match the focused app (`org.kde.konsole` matches `konsole`)?
fn app_matches(key: &str, app: &str) -> bool {
    let (k, a) = (key.to_lowercase(), app.to_lowercase());
    k == a || a.rsplit('.').next() == Some(k.as_str()) || k.rsplit('.').next() == Some(a.as_str())
}

impl Menu {
    pub fn items_for(&self, app: Option<&str>) -> &[Item] {
        app.and_then(|a| self.apps.iter().find(|(k, _)| app_matches(k, a)))
            .map_or(&self.items, |(_, i)| i)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub layout: Layout,
    pub haptic_ticks: bool,
    /// Waveform played when the highlighted item changes / on selection.
    pub tick_waveform: u8,
    pub select_waveform: u8,
    pub labels: Labels,
    /// Hover time before the tooltip shows (`labels = "hover"`).
    pub tooltip_ms: u32,
    pub theme: Theme,
    /// Both palettes, with `colors = { ... }` overrides applied to each.
    pub light: Colors,
    pub dark: Colors,
    /// Open animation length (0 = none).
    pub animation_ms: u32,
    pub menus: Vec<(String, Menu)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            layout: Layout::for_ring(84.0),
            haptic_ticks: true,
            tick_waveform: 4, // subtle-collision (Options+ uses it for Actions Ring hover)
            select_waveform: 1, // damp-state-change
            labels: Labels::Hover,
            tooltip_ms: 500,
            theme: Theme::System,
            light: Colors::LIGHT,
            dark: Colors::DARK,
            animation_ms: 160,
            menus: Vec::new(),
        }
    }
}

/// Bubbles per ring, as in the Actions Ring (8 slots; folders hold up to 9).
pub const MAX_RING_ITEMS: usize = 8;
pub const MAX_FOLDER_ITEMS: usize = 9;

impl Settings {
    /// The palette to draw with; `system_dark` = the desktop prefers dark.
    pub fn colors(&self, system_dark: bool) -> Colors {
        let dark = match self.theme {
            Theme::System => system_dark,
            Theme::Light => false,
            Theme::Dark => true,
        };
        if dark { self.dark } else { self.light }
    }

    pub fn menu(&self, name: &str) -> Option<&Menu> {
        self.menus.iter().find(|(n, _)| n == name).map(|(_, m)| m)
    }
}
