//! `$XDG_STATE_HOME/detent/state.json` (default `~/.local/state/detent/state.json`):
//! facts Detent records about itself, not settings. Settings live in `tray.toml`.
//!
//! Loading is tolerant: a missing, unreadable or corrupt file is [`State::default`], so a
//! damaged file can at worst restart the sponsor nudge's week, never stop the tray.
//! Saving is atomic (a temp file in the same directory, then `rename`), so a crash mid-write
//! leaves either the old file or the new one.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What Detent remembers between runs. Unknown fields are ignored and every field has a
/// default, so older and newer builds read each other's files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// When the tray first ran, in Unix seconds. Set once by
    /// [`crate::sponsor::record_first_run`]; `None` only until then, including in files
    /// written before the field existed.
    pub first_run: Option<u64>,
    /// The sponsor nudge was answered ("Don't ask again" or dismissed). Final.
    pub sponsor_nudge_done: bool,
}

/// `$XDG_STATE_HOME/detent/state.json`, else `~/.local/state/detent/state.json`. The same
/// HOME-based rule on every OS, like [`crate::settings::config_path`]; only the Linux tray
/// writes it today.
pub fn path() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::settings::home().join(".local").join("state"))
        .join("detent")
        .join("state.json")
}

impl State {
    /// [`State::load_from`] at [`path`].
    pub fn load() -> Self {
        Self::load_from(&path())
    }

    /// The state at `path`; missing or corrupt reads as the default.
    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// [`State::save_to`] at [`path`].
    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&path())
    }

    /// Write atomically, creating the parent directory.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        let dir = path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)?;
        let body = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "state.json".into());
        let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
        let written = std::fs::File::create(&tmp).and_then(|mut f| {
            f.write_all(body.as_bytes())?;
            f.write_all(b"\n")?;
            f.sync_all()
        });
        match written.and_then(|()| std::fs::rename(&tmp, path)) {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("detent-state-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn missing_is_default() {
        assert_eq!(
            State::load_from(Path::new("/nonexistent/detent/state.json")),
            State::default()
        );
    }

    #[test]
    fn round_trip_creates_the_directory() {
        let p = dir("rt").join("nested").join("state.json");
        let s = State {
            first_run: Some(1_790_000_000),
            sponsor_nudge_done: true,
        };
        s.save_to(&p).unwrap();
        assert_eq!(State::load_from(&p), s);
        // Overwrite in place; no temp file is left behind.
        let s2 = State {
            sponsor_nudge_done: false,
            ..s
        };
        s2.save_to(&p).unwrap();
        assert_eq!(State::load_from(&p), s2);
        let entries = std::fs::read_dir(p.parent().unwrap()).unwrap().count();
        assert_eq!(entries, 1, "only state.json");
    }

    #[test]
    fn corrupt_or_partial_files_read_as_default() {
        let d = dir("bad");
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("state.json");
        for body in ["", "{", "not json", "[1,2]", r#"{"first_run":"yesterday"}"#] {
            std::fs::write(&p, body).unwrap();
            assert_eq!(State::load_from(&p), State::default(), "{body:?}");
        }
        // Older files (no field) and newer ones (extra fields) still load.
        std::fs::write(&p, r#"{"sponsor_nudge_done":true,"future":1}"#).unwrap();
        assert_eq!(
            State::load_from(&p),
            State {
                first_run: None,
                sponsor_nudge_done: true
            }
        );
    }

    #[test]
    fn path_follows_xdg_state_home_shape() {
        let p = path();
        assert!(p.ends_with("detent/state.json"), "{}", p.display());
    }
}
