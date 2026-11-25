//! `status`, `enable`/`disable`/`toggle`, `version`, `update check`.

use anyhow::Result;
use detent_ipc::method;
use detent_ipc::types::{RemapState, Status};
use serde::Serialize;
use serde_json::json;

use crate::cmd::daemon::service_state;
use crate::ctx::Ctx;
use crate::exit::{self, Exit};
use crate::output::human;

/// `status --json`: `{"running": true, ...Status}` or `{"running": false, socket, service}`.
#[derive(Serialize)]
pub struct StatusOut<'a> {
    pub running: bool,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub status: Option<&'a Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
}

pub fn status(ctx: &Ctx) -> Result<()> {
    Ctx::require_daemon_os()?;
    let Some(mut d) = ctx.try_daemon()? else {
        let socket = ctx.socket.display().to_string();
        let service = service_state();
        ctx.emit(
            &StatusOut {
                running: false,
                status: None,
                socket: Some(socket.clone()),
                service: Some(service.clone()),
            },
            |st| human::not_running(st, &socket, &service),
        )?;
        // Exit 69 plus the "how to start it" hint.
        return Err(detent_ipc::ClientError::NotRunning(socket).into());
    };
    let s: Status = d.call_as(method::STATUS, json!({}))?;
    ctx.emit(
        &StatusOut {
            running: true,
            status: Some(&s),
            socket: None,
            service: None,
        },
        |st| human::status(st, &s),
    )
}

/// `enable` (Some(true)), `disable` (Some(false)), `toggle` (None).
pub fn remap(ctx: &Ctx, want: Option<bool>) -> Result<()> {
    let mut d = ctx.daemon()?;
    let enabled = match want {
        Some(e) => e,
        None => {
            !d.call_as::<RemapState>(method::REMAP_GET, json!({}))?
                .enabled
        }
    };
    let r: RemapState = d.call_as(method::REMAP_SET, json!({ "enabled": enabled }))?;
    if ctx.quiet && !ctx.json {
        return Ok(());
    }
    ctx.emit(&r, |st| human::remap(st, &r))
}

#[derive(Serialize)]
pub struct BuildInfo {
    pub version: &'static str,
    pub commit: &'static str,
    pub target: &'static str,
    pub profile: &'static str,
}

#[derive(Serialize)]
pub struct DaemonVersion {
    pub version: String,
    pub pid: u32,
}

#[derive(Serialize)]
pub struct VersionOut {
    pub detentctl: BuildInfo,
    /// `null` when the daemon isn't running.
    pub detentd: Option<DaemonVersion>,
}

pub fn build_info() -> BuildInfo {
    BuildInfo {
        version: env!("CARGO_PKG_VERSION"),
        commit: env!("DETENT_GIT_COMMIT"),
        target: env!("DETENT_TARGET"),
        profile: env!("DETENT_PROFILE"),
    }
}

pub fn version(ctx: &Ctx) -> Result<()> {
    let detentd = match ctx.try_daemon()? {
        Some(mut d) => {
            let s: Status = d.call_as(method::STATUS, json!({}))?;
            Some(DaemonVersion {
                version: s.version,
                pid: s.pid,
            })
        }
        None => None,
    };
    let v = VersionOut {
        detentctl: build_info(),
        detentd,
    };
    ctx.emit(&v, |st| human::version(st, &v))
}

/// `update check --json`. `latest`/`update_available`/`url` are `null` when the check
/// couldn't run. `supported` is always `true` (the check needs no token); it stays because
/// JSON fields are never removed.
#[derive(Serialize)]
pub struct UpdateOut {
    pub current: &'static str,
    pub latest: Option<String>,
    pub update_available: Option<bool>,
    pub supported: bool,
    pub url: Option<String>,
    pub message: String,
}

/// Printed under "Detent X is available" in human mode (not with `-q` or `--json`).
pub const SPONSOR_LINE: &str =
    "Detent is free and built by one person — sponsor: https://github.com/sponsors/Rfluid";

/// Exit code for a failed check (docs/user/cli.md).
fn update_exit(e: &detent_update::Error) -> u8 {
    use detent_update::Error as E;
    match e {
        E::NotFound => exit::UNAVAILABLE,
        E::Unauthorized(_) => exit::NOPERM,
        E::Network(_) | E::Http(_) => exit::IOERR,
        E::Parse(_) => exit::PROTOCOL,
        E::Config(_) => exit::CONFIG,
    }
}

/// Compare with the latest GitHub Release (crates/update, shared with the tray). Never
/// downloads anything: it says what's new and where to get it.
pub fn update_check(ctx: &Ctx) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    let res = detent_update::Settings::load().and_then(|s| detent_update::check(current, &s));
    let (out, err) = match res {
        Ok(r) => {
            let message = if r.update_available {
                format!(
                    "Detent {} is available (you have {current}): {}",
                    r.latest.tag, r.latest.url
                )
            } else {
                format!(
                    "detentctl {current} is up to date (latest release {})",
                    r.latest.tag
                )
            };
            let out = UpdateOut {
                current,
                latest: Some(r.latest.tag),
                update_available: Some(r.update_available),
                supported: true,
                url: Some(r.latest.url),
                message,
            };
            (out, None)
        }
        Err(e) => {
            let out = UpdateOut {
                current,
                latest: None,
                update_available: None,
                supported: true,
                url: None,
                message: e.to_string(),
            };
            (out, Some(e))
        }
    };
    match err {
        None => ctx.emit(&out, |_| {
            println!("{}", out.message);
            if out.update_available == Some(true) && !ctx.quiet {
                println!("{SPONSOR_LINE}");
            }
        }),
        Some(e) => {
            if ctx.json {
                ctx.emit(&out, |_| {})?;
            }
            Err(Exit::new(update_exit(&e), format!("update check: {e}")).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_running_json_shape() {
        let v = serde_json::to_value(StatusOut {
            running: false,
            status: None,
            socket: Some("/s".into()),
            service: Some("inactive".into()),
        })
        .unwrap();
        assert_eq!(
            v,
            json!({"running": false, "socket": "/s", "service": "inactive"})
        );
    }

    #[test]
    fn running_json_is_flat() {
        let s: Status = serde_json::from_value(json!({
            "version": "1", "config_path": "/c", "active_profile": "base"
        }))
        .unwrap();
        let v = serde_json::to_value(StatusOut {
            running: true,
            status: Some(&s),
            socket: None,
            service: None,
        })
        .unwrap();
        assert_eq!(v["running"], true);
        assert_eq!(v["version"], "1");
        assert_eq!(v["remap_enabled"], true);
        assert!(v.get("socket").is_none());
    }

    #[test]
    fn update_exit_codes() {
        use detent_update::Error as E;
        assert_eq!(update_exit(&E::NotFound), exit::UNAVAILABLE);
        assert_eq!(update_exit(&E::Unauthorized(401)), exit::NOPERM);
        assert_eq!(update_exit(&E::Network("x".into())), exit::IOERR);
        assert_eq!(update_exit(&E::Http(403)), exit::IOERR);
        let v = serde_json::to_value(UpdateOut {
            current: "0.0.1",
            latest: None,
            update_available: None,
            supported: true,
            url: None,
            message: E::NotFound.to_string(),
        })
        .unwrap();
        assert_eq!(v["supported"], true);
        assert!(v["latest"].is_null() && v["url"].is_null());
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys.len(),
            6,
            "the JSON shape is unchanged (no sponsor field): {keys:?}"
        );
    }

    #[test]
    fn sponsor_line_names_github_sponsors() {
        assert!(SPONSOR_LINE.ends_with(detent_update::sponsor::SPONSOR_URL));
    }

    #[test]
    fn version_json_shape() {
        let v = serde_json::to_value(VersionOut {
            detentctl: build_info(),
            detentd: None,
        })
        .unwrap();
        assert_eq!(v["detentctl"]["version"], env!("CARGO_PKG_VERSION"));
        assert!(v["detentctl"]["commit"].is_string());
        assert!(v["detentd"].is_null());
    }
}
