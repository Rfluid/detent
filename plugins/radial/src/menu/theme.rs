//! Palettes: the light and dark Actions Ring colours, `theme`, and colour parsing.

/// Colours as `0xRRGGBBAA`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colors {
    /// Bubble fill at rest.
    pub bubble: u32,
    /// Glyph (single-colour icon / text) at rest.
    pub icon: u32,
    /// Bubble fill when highlighted.
    pub accent: u32,
    /// Glyph colour when highlighted.
    pub accent_icon: u32,
    /// The centre (close / back) disc and its glyph.
    pub center: u32,
    pub center_icon: u32,
    /// Label pill.
    pub label_bg: u32,
    pub label_text: u32,
    /// Gauge track (the unfilled part of the volume bar and arc).
    pub track: u32,
    /// Hairline around bubbles, the centre and the gauge card (separates them from a
    /// background of the same tone; transparent = none).
    pub edge: u32,
    /// Drop shadow under bubbles.
    pub shadow: u32,
}

impl Colors {
    /// The Actions Ring look: pale lavender bubbles, near-black glyphs, violet highlight
    /// with a white glyph, light grey centre.
    pub const LIGHT: Self = Self {
        bubble: 0xf3f0fff2,
        icon: 0x1c1b1fff,
        accent: 0x814efaff,
        accent_icon: 0xffffffff,
        center: 0xefeef2f2,
        center_icon: 0x1c1b1fff,
        label_bg: 0xffffffee,
        label_text: 0x1c1b1fff,
        track: 0x1c1b1f24,
        edge: 0x1c1b1f0f,
        shadow: 0x2a1f4d29,
    };
    /// The same ring for dark desktops: deep violet-grey bubbles with light glyphs, a
    /// lighter violet highlight (keeps contrast on dark), a faint light edge so bubbles
    /// don't sink into dark wallpapers, deeper shadows.
    pub const DARK: Self = Self {
        bubble: 0x2b2833f2,
        icon: 0xe9e4f2ff,
        accent: 0x8f5cffff,
        accent_icon: 0xffffffff,
        center: 0x211f27f2,
        center_icon: 0xe9e4f2ff,
        label_bg: 0x2b2833f0,
        label_text: 0xe9e4f2ff,
        track: 0xffffff26,
        edge: 0xffffff1a,
        shadow: 0x00000080,
    };

    pub(super) fn slot(&mut self, name: &str) -> Option<&mut u32> {
        Some(match name {
            "bubble" => &mut self.bubble,
            "icon" => &mut self.icon,
            "accent" => &mut self.accent,
            "accent_icon" => &mut self.accent_icon,
            "center" => &mut self.center,
            "center_icon" => &mut self.center_icon,
            "label_bg" => &mut self.label_bg,
            "label_text" => &mut self.label_text,
            "track" => &mut self.track,
            "edge" => &mut self.edge,
            "shadow" => &mut self.shadow,
            _ => return None,
        })
    }
}

/// `theme = "system" | "light" | "dark"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// Follow the desktop's light/dark preference (XDG portal `color-scheme`, which Plasma
    /// sets from the colour scheme), read each time the ring opens.
    #[default]
    System,
    Light,
    Dark,
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa` → `0xRRGGBBAA`.
pub fn parse_color(s: &str) -> Result<u32, String> {
    let h = s.trim().trim_start_matches('#');
    let bad = || format!("colour `{s}`: use #rgb, #rrggbb or #rrggbbaa");
    if !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(bad());
    }
    let v = u32::from_str_radix(h, 16).map_err(|_| bad())?;
    match h.len() {
        3 => {
            let (r, g, b) = ((v >> 8) & 0xf, (v >> 4) & 0xf, v & 0xf);
            Ok((r * 0x11) << 24 | (g * 0x11) << 16 | (b * 0x11) << 8 | 0xff)
        }
        6 => Ok(v << 8 | 0xff),
        8 => Ok(v),
        _ => Err(bad()),
    }
}
