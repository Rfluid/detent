//! `[update]` and `[sponsor]` in `$XDG_CONFIG_HOME/detent/tray.toml`, and where the GitHub
//! token comes from.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;

use crate::Error;

/// `[update]` of the tray's config file. Every key is optional.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Background checks in the tray (start, then daily). Manual checks always run.
    #[serde(default = "yes")]
    pub check: bool,
    /// File holding a read-only GitHub token, used when `gh auth token` gives none. Only
    /// read when the anonymous check hits GitHub's rate limit. `~/` is expanded.
    #[serde(default)]
    pub token_file: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            check: true,
            token_file: None,
        }
    }
}

fn yes() -> bool {
    true
}

#[derive(Debug, Default, Deserialize)]
struct File {
    #[serde(default)]
    update: Settings,
}

/// `[sponsor]` of the tray's config file: the opt-out for the one-time sponsor nudge
/// ([`crate::sponsor`]). Parsed on its own, so a typo under `[update]` can't switch the
/// opt-out back on (and the other way round).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SponsorSettings {
    /// `false`: never show the "Enjoying Detent?" notification.
    #[serde(default = "yes")]
    pub nudge: bool,
}

impl Default for SponsorSettings {
    fn default() -> Self {
        Self { nudge: true }
    }
}

#[derive(Debug, Default, Deserialize)]
struct SponsorFile {
    #[serde(default)]
    sponsor: SponsorSettings,
}

impl SponsorSettings {
    /// Parse the `[sponsor]` table out of a tray.toml body (other tables are ignored).
    pub fn parse(text: &str) -> Result<Self, Error> {
        let f: SponsorFile = toml::from_str(text).map_err(|e| Error::Config(e.to_string()))?;
        Ok(f.sponsor)
    }

    /// Read [`config_path`]. A missing file is the default; an invalid one is an error.
    pub fn load() -> Result<Self, Error> {
        Self::load_from(&config_path())
    }

    pub fn load_from(path: &Path) -> Result<Self, Error> {
        read_or_default(path, Self::parse)
    }
}

/// `parse` the file at `path`; a missing file is `T::default()`.
fn read_or_default<T: Default>(
    path: &Path,
    parse: impl FnOnce(&str) -> Result<T, Error>,
) -> Result<T, Error> {
    match std::fs::read_to_string(path) {
        Ok(t) => parse(&t).map_err(|e| Error::Config(format!("{}: {e}", path.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(Error::Config(format!("{}: {e}", path.display()))),
    }
}

/// `$XDG_CONFIG_HOME/detent/tray.toml` (default `~/.config/detent/tray.toml`).
pub fn config_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
        .join("detent")
        .join("tray.toml")
}

pub(crate) fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

impl Settings {
    /// Parse the `[update]` table out of a tray.toml body (other tables are ignored).
    pub fn parse(text: &str) -> Result<Self, Error> {
        let f: File = toml::from_str(text).map_err(|e| Error::Config(e.to_string()))?;
        Ok(f.update)
    }

    /// Read [`config_path`]. A missing file is the default; an invalid one is an error.
    pub fn load() -> Result<Self, Error> {
        Self::load_from(&config_path())
    }

    pub fn load_from(path: &Path) -> Result<Self, Error> {
        read_or_default(path, Self::parse)
    }

    /// `token_file` with `~/` expanded.
    pub fn token_path(&self) -> Option<PathBuf> {
        let p = self.token_file.as_ref()?;
        Some(match p.strip_prefix("~") {
            Ok(rest) => home().join(rest),
            Err(_) => p.clone(),
        })
    }
}

/// Where a token came from (for messages; the token itself is never printed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSource {
    Gh,
    File(PathBuf),
}

#[derive(Clone, PartialEq, Eq)]
pub struct Token {
    pub value: String,
    pub source: TokenSource,
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Token")
            .field("value", &"<redacted>")
            .field("source", &self.source)
            .finish()
    }
}

/// `gh auth token` when gh is installed and logged in.
pub fn gh_token() -> Option<String> {
    let out = Command::new("gh")
        .args(["auth", "token", "--hostname", "github.com"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    non_empty(String::from_utf8(out.stdout).ok()?)
}

fn non_empty(s: String) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// The rate-limit fallback token: `gh auth token` first, then `token_file`.
pub fn find_token(settings: &Settings) -> Option<Token> {
    find_token_with(gh_token, settings)
}

/// [`find_token`] with the gh lookup injected (tests).
pub fn find_token_with(gh: impl FnOnce() -> Option<String>, settings: &Settings) -> Option<Token> {
    if let Some(value) = gh() {
        return Some(Token {
            value,
            source: TokenSource::Gh,
        });
    }
    let path = settings.token_path()?;
    let value = non_empty(std::fs::read_to_string(&path).ok()?)?;
    Some(Token {
        value,
        source: TokenSource::File(path),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_parse() {
        assert_eq!(Settings::parse("").unwrap(), Settings::default());
        assert_eq!(
            Settings::parse("[other]\nx = 1\n").unwrap(),
            Settings::default()
        );
        let s = Settings::parse("[update]\ncheck = false\ntoken_file = \"/t\"\n").unwrap();
        assert!(!s.check);
        assert_eq!(s.token_path(), Some(PathBuf::from("/t")));
        assert!(
            Settings::parse("[update]\nchek = false\n").is_err(),
            "typos are errors"
        );
    }

    #[test]
    fn sponsor_table() {
        // Missing file body, missing table, missing key: the nudge is on.
        assert!(SponsorSettings::parse("").unwrap().nudge);
        assert!(
            SponsorSettings::parse("[update]\ncheck = false\n")
                .unwrap()
                .nudge
        );
        assert!(SponsorSettings::parse("[sponsor]\n").unwrap().nudge);
        assert!(
            !SponsorSettings::parse("[sponsor]\nnudge = false\n")
                .unwrap()
                .nudge
        );
        assert!(SponsorSettings::parse("[sponsor]\nnuge = false\n").is_err());
        // Each table parses on its own: [sponsor] doesn't upset [update] and vice versa.
        let both = "[update]\nchek = 1\n[sponsor]\nnudge = false\n";
        assert!(!SponsorSettings::parse(both).unwrap().nudge);
        assert!(Settings::parse(both).is_err());
        let both = "[update]\ncheck = false\n[sponsor]\nnudge = false\n";
        assert!(!Settings::parse(both).unwrap().check);
        let missing = std::path::Path::new("/nonexistent/detent/tray.toml");
        assert_eq!(
            SponsorSettings::load_from(missing).unwrap(),
            SponsorSettings::default()
        );
    }

    #[test]
    fn tilde_expands() {
        let s = Settings {
            check: true,
            token_file: Some("~/tok".into()),
        };
        assert_eq!(s.token_path(), Some(home().join("tok")));
    }

    #[test]
    fn no_gh_no_file_means_no_token() {
        assert_eq!(find_token_with(|| None, &Settings::default()), None);
        let missing = Settings {
            check: true,
            token_file: Some("/nonexistent/detent-token".into()),
        };
        assert_eq!(find_token_with(|| None, &missing), None);
    }

    #[test]
    fn gh_wins_then_file() {
        let dir = std::env::temp_dir().join(format!("detent-update-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("token");
        std::fs::write(&f, "  abc\n").unwrap();
        let s = Settings {
            check: true,
            token_file: Some(f.clone()),
        };
        let t = find_token_with(|| Some("gh".into()), &s).unwrap();
        assert_eq!((t.value.as_str(), t.source), ("gh", TokenSource::Gh));
        let t = find_token_with(|| None, &s).unwrap();
        assert_eq!(t.value, "abc");
        assert_eq!(t.source, TokenSource::File(f));
        assert!(
            !format!("{t:?}").contains("abc"),
            "Debug never prints the token"
        );
        std::fs::write(dir.join("empty"), "\n").unwrap();
        let e = Settings {
            check: true,
            token_file: Some(dir.join("empty")),
        };
        assert_eq!(find_token_with(|| None, &e), None);
    }
}
