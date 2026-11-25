//! Release update check: is there a newer GitHub Release of Detent than this build?
//!
//! Shared by `detentctl update check` and `detent-tray`. Contract (asset names, endpoint,
//! token fallback): docs/dev/release.md. This crate only *tells*; it never downloads or
//! installs anything. [`Release::asset_for`] names the archive an installer would fetch.
//!
//! The repository is public, so the check is an anonymous `GET /releases/latest`. GitHub
//! allows 60 anonymous API requests an hour per IP; only when that limit answers (HTTP
//! 403/429) does the check retry with a token: `gh auth token` when gh is installed and
//! logged in, else the file named by `token_file` under `[update]` in
//! `~/.config/detent/tray.toml` ([`settings`]).
//!
//! It also holds the sponsor links and the one-time "Enjoying Detent?" nudge
//! ([`sponsor`], gated on [`state::State`] in `$XDG_STATE_HOME/detent/state.json` and
//! `[sponsor] nudge` in tray.toml): the same two consumers, the same config file, and
//! too little code for a crate of its own.

use std::time::Duration;

use semver::Version;
use serde::Deserialize;

pub mod assets;
pub mod settings;
pub mod sponsor;
pub mod state;

pub use assets::{Asset, asset_name, checksum_name, host_target};
pub use settings::{Settings, SponsorSettings, Token, TokenSource, find_token};

pub const REPO: &str = "Rfluid/detent";
pub const REPO_URL: &str = "https://github.com/Rfluid/detent";
pub const RELEASES_URL: &str = "https://github.com/Rfluid/detent/releases";
/// `/releases/latest` skips drafts and pre-releases.
pub const LATEST_API: &str = "https://api.github.com/repos/Rfluid/detent/releases/latest";

const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The rate-limit fallback's token was refused.
    #[error(
        "GitHub rejected the token (HTTP {0}); refresh it with `gh auth login` or replace the token file"
    )]
    Unauthorized(u16),
    #[error("no published release found (HTTP 404): {REPO} has no release yet")]
    NotFound,
    #[error("GitHub answered HTTP {0}{hint}", hint = http_hint(*.0))]
    Http(u16),
    #[error("network: {0}")]
    Network(String),
    #[error("unexpected release data: {0}")]
    Parse(String),
    #[error("config: {0}")]
    Config(String),
}

/// Extra text for [`Error::Http`]: 403/429 without a token is the anonymous rate limit.
fn http_hint(status: u16) -> String {
    if is_rate_limit(status) {
        format!(
            " (anonymous rate limit: try again within the hour, or log in with `gh auth login` \
             or set `token_file` under [update] in {})",
            settings::config_path().display()
        )
    } else {
        String::new()
    }
}

/// GitHub answers an exhausted rate limit with 403 (primary) or 429 (secondary).
fn is_rate_limit(status: u16) -> bool {
    matches!(status, 403 | 429)
}

/// The newest published release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The git tag, e.g. `v0.2.0`.
    pub tag: String,
    pub version: Version,
    /// The release page (`html_url`).
    pub url: String,
    /// Attached files (`assets`); empty when the body had none.
    pub assets: Vec<Asset>,
}

impl Release {
    /// The archive for `target` (a release triple, see [`assets::TARGETS`]), if attached.
    pub fn asset_for(&self, target: &str) -> Option<&Asset> {
        self.find(&asset_name(&self.tag, target))
    }

    /// The `.sha256` file for [`Release::asset_for`], if attached.
    pub fn checksum_for(&self, target: &str) -> Option<&Asset> {
        self.find(&checksum_name(&self.tag, target))
    }

    fn find(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name == name)
    }
}

/// Result of comparing this build with the newest release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub current: Version,
    pub latest: Release,
    pub update_available: bool,
}

/// `v1.2.3` or `1.2.3` → a semver version.
pub fn parse_tag(tag: &str) -> Result<Version, Error> {
    let t = tag.trim();
    Version::parse(t.strip_prefix('v').unwrap_or(t))
        .map_err(|e| Error::Parse(format!("tag {t:?}: {e}")))
}

/// A `/releases/latest` body → [`Release`].
pub fn parse_release(body: &str) -> Result<Release, Error> {
    #[derive(Deserialize)]
    struct Api {
        tag_name: String,
        #[serde(default)]
        html_url: Option<String>,
        #[serde(default)]
        assets: Vec<Asset>,
    }
    let a: Api = serde_json::from_str(body).map_err(|e| Error::Parse(e.to_string()))?;
    Ok(Release {
        version: parse_tag(&a.tag_name)?,
        url: a
            .html_url
            .unwrap_or_else(|| format!("{RELEASES_URL}/tag/{}", a.tag_name)),
        tag: a.tag_name,
        assets: a.assets,
    })
}

/// Compare `current` (a `CARGO_PKG_VERSION`) with a release. Strictly newer only: a
/// local build ahead of the latest release is not an update.
pub fn compare(current: &str, latest: Release) -> Result<Report, Error> {
    let current = Version::parse(current)
        .map_err(|e| Error::Parse(format!("own version {current:?}: {e}")))?;
    Ok(Report {
        update_available: latest.version > current,
        current,
        latest,
    })
}

/// `GET /repos/Rfluid/detent/releases/latest`, anonymous or with a token. Blocking
/// (≤ 15 s).
pub fn fetch_latest(token: Option<&Token>) -> Result<Release, Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();
    let mut req = agent
        .get(LATEST_API)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "detent-update");
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {}", t.value));
    }
    let mut resp = req.call().map_err(|e| Error::Network(e.to_string()))?;
    match (resp.status().as_u16(), token.is_some()) {
        (200, _) => {}
        (s @ (401 | 403), true) => return Err(Error::Unauthorized(s)),
        (404, _) => return Err(Error::NotFound),
        (s, _) => return Err(Error::Http(s)),
    }
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| Error::Network(e.to_string()))?;
    parse_release(&body)
}

/// Everything in one call: anonymous fetch (token retry on the rate limit) → compare.
pub fn check(current: &str, settings: &Settings) -> Result<Report, Error> {
    let latest = fetch_with_fallback(fetch_latest, || find_token(settings))?;
    compare(current, latest)
}

/// Anonymous first; on a rate-limit answer retry once with a token, if one is found.
/// Injected fetch and token lookup so the policy is testable without the network.
fn fetch_with_fallback(
    fetch: impl Fn(Option<&Token>) -> Result<Release, Error>,
    token: impl FnOnce() -> Option<Token>,
) -> Result<Release, Error> {
    match fetch(None) {
        Err(Error::Http(s)) if is_rate_limit(s) => match token() {
            Some(t) => fetch(Some(&t)),
            None => Err(Error::Http(s)),
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(tag: &str) -> Release {
        Release {
            tag: tag.into(),
            version: parse_tag(tag).unwrap(),
            url: String::new(),
            assets: Vec::new(),
        }
    }

    #[test]
    fn tags_parse_with_or_without_v() {
        assert_eq!(parse_tag("v1.2.3").unwrap(), Version::new(1, 2, 3));
        assert_eq!(parse_tag("0.0.1").unwrap(), Version::new(0, 0, 1));
        assert!(parse_tag("main-abc").is_err());
    }

    #[test]
    fn only_strictly_newer_is_an_update() {
        assert!(compare("0.0.1", rel("v0.1.0")).unwrap().update_available);
        assert!(
            compare("0.9.9", rel("v0.10.0")).unwrap().update_available,
            "semver, not text"
        );
        assert!(!compare("0.1.0", rel("v0.1.0")).unwrap().update_available);
        assert!(!compare("0.2.0", rel("v0.1.0")).unwrap().update_available);
        assert!(
            !compare("1.0.0", rel("v1.0.0-rc1"))
                .unwrap()
                .update_available
        );
        assert!(
            compare("1.0.0-rc1", rel("v1.0.0"))
                .unwrap()
                .update_available
        );
    }

    #[test]
    fn release_body() {
        let r = parse_release(
            r#"{"tag_name":"v0.2.0","html_url":"https://github.com/Rfluid/detent/releases/tag/v0.2.0","assets":[]}"#,
        )
        .unwrap();
        assert_eq!(r.tag, "v0.2.0");
        assert_eq!(r.version, Version::new(0, 2, 0));
        assert!(r.url.ends_with("/tag/v0.2.0"));
        let r = parse_release(r#"{"tag_name":"v0.3.0"}"#).unwrap();
        assert_eq!(r.url, format!("{RELEASES_URL}/tag/v0.3.0"));
        assert!(parse_release("{}").is_err());
    }

    #[test]
    fn release_assets_pick_this_tag_and_target() {
        let body = r#"{"tag_name":"v0.1.0","assets":[
            {"name":"detent-v0.1.0-x86_64-unknown-linux-gnu.tar.gz","browser_download_url":"https://x/l.tgz"},
            {"name":"detent-v0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256","browser_download_url":"https://x/l.sha"},
            {"name":"detent-v0.1.0-x86_64-pc-windows-msvc.zip","browser_download_url":"https://x/w.zip"},
            {"name":"detent-v0.0.9-aarch64-apple-darwin.tar.gz","browser_download_url":"https://x/old"}
        ]}"#;
        let r = parse_release(body).unwrap();
        let linux = "x86_64-unknown-linux-gnu";
        assert_eq!(r.asset_for(linux).unwrap().url, "https://x/l.tgz");
        assert_eq!(r.checksum_for(linux).unwrap().url, "https://x/l.sha");
        assert_eq!(
            r.asset_for("x86_64-pc-windows-msvc").unwrap().url,
            "https://x/w.zip"
        );
        assert!(
            r.asset_for("aarch64-apple-darwin").is_none(),
            "another tag's asset doesn't count"
        );
        assert!(r.checksum_for("x86_64-pc-windows-msvc").is_none());
    }

    fn token() -> Token {
        Token {
            value: "t".into(),
            source: TokenSource::Gh,
        }
    }

    #[test]
    fn anonymous_first_token_only_on_rate_limit() {
        use std::cell::RefCell;
        // 200 anonymously: the token lookup never runs.
        let r = fetch_with_fallback(|_| Ok(rel("v0.1.0")), || panic!("no token lookup"));
        assert_eq!(r.unwrap().tag, "v0.1.0");
        // 404 and network errors don't retry either.
        let r = fetch_with_fallback(|_| Err(Error::NotFound), || panic!("no token lookup"));
        assert!(matches!(r, Err(Error::NotFound)));
        // Rate limited, token found: second call carries it.
        for status in [403, 429] {
            let calls = RefCell::new(Vec::new());
            let r = fetch_with_fallback(
                |t| {
                    calls.borrow_mut().push(t.is_some());
                    if t.is_some() {
                        Ok(rel("v0.2.0"))
                    } else {
                        Err(Error::Http(status))
                    }
                },
                || Some(token()),
            );
            assert_eq!(r.unwrap().tag, "v0.2.0");
            assert_eq!(*calls.borrow(), [false, true]);
        }
        // Rate limited, no token: the rate-limit error, with the hint.
        let r = fetch_with_fallback(|_| Err(Error::Http(403)), || None);
        let e = r.unwrap_err();
        assert!(matches!(e, Error::Http(403)));
        let msg = e.to_string();
        assert!(
            msg.contains("rate limit") && msg.contains("token_file"),
            "{msg}"
        );
        assert_eq!(Error::Http(500).to_string(), "GitHub answered HTTP 500");
    }
}
