//! Icon resolution for ring items: file paths, freedesktop icon-theme names, desktop
//! files. Pure file-system lookups; called while parsing settings (bus thread).
//!
//! Lookup follows the freedesktop Icon Theme spec in a simplified form: the current theme
//! (KDE's `[Icons] Theme` in `kdeglobals`, else `breeze`), the themes it `Inherits`, then
//! `hicolor`, then `/usr/share/pixmaps`. Within a theme, scalable and larger directories
//! win. `<name>-symbolic` is preferred when a single-colour glyph is wanted.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// How an item is drawn inside its bubble.
#[derive(Debug, Clone, PartialEq)]
pub enum Icon {
    /// No icon: the bubble shows the label's initials.
    None,
    /// An SVG drawn as a single-colour mask (the theme's icon colour; white when
    /// highlighted), like the Actions Ring glyphs.
    Glyph(PathBuf),
    /// A full-colour image (PNG/JPEG/WebP/SVG), fitted inside the bubble.
    Image(PathBuf),
    /// A photo filling the whole bubble, cropped to a circle.
    Photo(PathBuf),
    /// Short plain text (emoji don't render in GPUI on Linux).
    Text(String),
}

impl Icon {
    pub fn photo(p: &str) -> Self {
        let path = PathBuf::from(expand_home(p));
        if path.is_file() {
            Self::Photo(path)
        } else {
            eprintln!("detent-radial: image `{p}` not found");
            Self::None
        }
    }
}

pub fn expand_home(p: &str) -> String {
    match (p.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(h)) => format!("{h}/{rest}"),
        _ => p.to_string(),
    }
}

fn is_svg(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"))
}

/// `icon = "..."`: a path (contains `/` or starts with `~`) or an icon-theme name.
/// `tint`: `Some(true)` draw as a single-colour glyph, `Some(false)` keep the image's
/// colours, `None` = glyph for SVGs and symbolic theme icons, colour otherwise.
pub fn resolve(spec: &str, tint: Option<bool>) -> Icon {
    resolve_logged(spec, tint, true)
}

/// Like `resolve`, without logging a miss (for automatic default icons).
pub fn resolve_quiet(spec: &str, tint: Option<bool>) -> Icon {
    resolve_logged(spec, tint, false)
}

fn resolve_logged(spec: &str, tint: Option<bool>, log: bool) -> Icon {
    let as_icon = |path: PathBuf, symbolic: bool| {
        let glyph = is_svg(&path) && tint.unwrap_or(symbolic);
        if glyph {
            Icon::Glyph(path)
        } else {
            Icon::Image(path)
        }
    };
    if spec.contains('/') || spec.starts_with('~') {
        let path = PathBuf::from(expand_home(spec));
        if path.is_file() {
            let glyph = is_svg(&path) && tint.unwrap_or(true);
            return if glyph {
                Icon::Glyph(path)
            } else {
                Icon::Image(path)
            };
        }
        if log {
            eprintln!("detent-radial: icon file `{spec}` not found");
        }
        return Icon::None;
    }
    let want_symbolic = tint != Some(false);
    let found = want_symbolic
        .then(|| lookup(&format!("{spec}-symbolic")).map(|p| (p, true)))
        .flatten()
        .or_else(|| lookup(spec).map(|p| (p, spec.ends_with("-symbolic"))));
    match found {
        Some((p, symbolic)) => as_icon(p, symbolic),
        None => {
            if log {
                eprintln!("detent-radial: icon `{spec}` not found in the icon theme");
            }
            Icon::None
        }
    }
}

/// XDG data dirs, most specific first.
fn data_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    match std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        Some(d) => out.push(PathBuf::from(d)),
        None => {
            if let Some(h) = std::env::var_os("HOME") {
                out.push(PathBuf::from(h).join(".local/share"));
            }
        }
    }
    let sys = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    out.extend(sys.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    out
}

fn icon_bases() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(h) = std::env::var_os("HOME") {
        out.push(PathBuf::from(h).join(".icons"));
    }
    out.extend(data_dirs().into_iter().map(|d| d.join("icons")));
    out
}

/// `key=value` from an INI section.
fn ini_get(text: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line == format!("[{section}]");
        } else if in_section
            && let Some((k, v)) = line.split_once('=')
            && k.trim() == key
        {
            return Some(v.trim().to_string());
        }
    }
    None
}

fn current_theme() -> String {
    if let Ok(t) = std::env::var("DETENT_ICON_THEME")
        && !t.is_empty()
    {
        return t;
    }
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    config
        .and_then(|c| std::fs::read_to_string(c.join("kdeglobals")).ok())
        .and_then(|t| ini_get(&t, "Icons", "Theme"))
        .unwrap_or_else(|| "breeze".into())
}

/// Directory score: scalable first, then by nominal size.
fn dir_score(sub: &str) -> u32 {
    if sub.contains("scalable") {
        return 10_000;
    }
    sub.split(['/', 'x', '@'])
        .filter_map(|p| p.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
}

/// Every icon directory of the theme chain, in lookup order.
fn theme_dirs() -> &'static [PathBuf] {
    static DIRS: OnceLock<Vec<PathBuf>> = OnceLock::new();
    DIRS.get_or_init(|| {
        let bases = icon_bases();
        let mut queue = vec![current_theme()];
        let mut seen: Vec<String> = Vec::new();
        let mut out = Vec::new();
        while let Some(theme) = queue.first().cloned() {
            queue.remove(0);
            if seen.contains(&theme) || seen.len() > 8 {
                continue;
            }
            seen.push(theme.clone());
            let mut dirs: Vec<(u32, PathBuf)> = Vec::new();
            for base in &bases {
                let root = base.join(&theme);
                let Ok(index) = std::fs::read_to_string(root.join("index.theme")) else {
                    continue;
                };
                if let Some(inh) = ini_get(&index, "Icon Theme", "Inherits") {
                    queue.extend(inh.split(',').map(|s| s.trim().to_string()));
                }
                let listed = ini_get(&index, "Icon Theme", "Directories").unwrap_or_default();
                for sub in listed.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    dirs.push((dir_score(sub), root.join(sub)));
                }
            }
            dirs.sort_by_key(|d| std::cmp::Reverse(d.0));
            out.extend(dirs.into_iter().map(|(_, d)| d));
            if queue.is_empty() && !seen.iter().any(|t| t == "hicolor") {
                queue.push("hicolor".into());
            }
        }
        out.extend(data_dirs().into_iter().map(|d| d.join("pixmaps")));
        out
    })
}

/// Find `<name>.{svg,png}` in the icon theme chain.
fn lookup(name: &str) -> Option<PathBuf> {
    theme_dirs().iter().find_map(|d| {
        ["svg", "png"]
            .iter()
            .map(|ext| d.join(format!("{name}.{ext}")))
            .find(|p| p.is_file())
    })
}

/// What a desktop file says about an app.
#[derive(Debug, Clone, PartialEq)]
pub struct DesktopEntry {
    pub name: String,
    /// Icon name or absolute path.
    pub icon: Option<String>,
}

/// `org.kde.konsole` → `…/applications/org.kde.konsole.desktop`; a short id (`firefox`)
/// also matches `org.mozilla.firefox.desktop`.
pub fn desktop_entry(id: &str) -> Option<DesktopEntry> {
    let file = format!("{id}.desktop");
    let suffix = format!(".{id}.desktop");
    let path = data_dirs().into_iter().find_map(|d| {
        let apps = d.join("applications");
        let exact = apps.join(&file);
        if exact.is_file() {
            return Some(exact);
        }
        std::fs::read_dir(&apps)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(&suffix))
            })
    })?;
    let text = std::fs::read_to_string(path).ok()?;
    Some(DesktopEntry {
        name: ini_get(&text, "Desktop Entry", "Name")?,
        icon: ini_get(&text, "Desktop Entry", "Icon"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ini_and_scores() {
        let t = "[Icon Theme]\nName=Breeze\nInherits=hicolor\n[Other]\nInherits=x\n";
        assert_eq!(
            ini_get(t, "Icon Theme", "Inherits").as_deref(),
            Some("hicolor")
        );
        assert_eq!(ini_get(t, "Other", "Inherits").as_deref(), Some("x"));
        assert_eq!(ini_get(t, "Missing", "Name"), None);
        assert!(dir_score("scalable/apps") > dir_score("48x48/apps"));
        assert!(dir_score("actions/32") > dir_score("actions/16"));
        assert_eq!(dir_score("actions/symbolic"), 0);
    }

    #[test]
    fn missing_icons_resolve_to_none() {
        assert_eq!(resolve("/nonexistent/x.svg", None), Icon::None);
        assert_eq!(resolve("detent-test-no-such-icon-name", None), Icon::None);
        assert_eq!(Icon::photo("/nonexistent/me.jpg"), Icon::None);
    }

    #[test]
    fn file_icons() {
        let dir = std::env::temp_dir().join(format!("detent-radial-icon-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let svg = dir.join("a.svg");
        let png = dir.join("b.png");
        let _ = std::fs::write(&svg, "<svg/>");
        let _ = std::fs::write(&png, "x");
        let s = svg.to_string_lossy();
        assert_eq!(resolve(&s, None), Icon::Glyph(svg.clone()));
        assert_eq!(resolve(&s, Some(false)), Icon::Image(svg.clone()));
        assert_eq!(
            resolve(&png.to_string_lossy(), None),
            Icon::Image(png.clone())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
