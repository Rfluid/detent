//! `daemon start|stop|restart|status` and `logs`: thin wrappers over `systemctl --user`
//! and `journalctl --user` for the `detent.service` unit (packaging/systemd/), as
//! service-backed tools usually do instead of managing processes themselves.

#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use detent_ipc::method;
use detent_ipc::types::Status;
use serde::Serialize;
use serde_json::json;

use crate::cli::{DaemonCmd, LogsArgs};
use crate::ctx::Ctx;
use crate::exit::{self, Exit};

pub const UNIT: &str = "detent.service";
const READY_TIMEOUT: Duration = Duration::from_secs(10);

fn systemctl(args: &[&str]) -> Command {
    let mut c = Command::new("systemctl");
    c.arg("--user").args(args);
    c
}

fn missing(tool: &str, e: std::io::Error) -> anyhow::Error {
    if e.kind() == std::io::ErrorKind::NotFound {
        Exit::new(
            exit::UNAVAILABLE,
            format!("`{tool}` not found: without systemd, run `detentd` directly"),
        )
        .into()
    } else {
        anyhow::Error::from(e).context(format!("running {tool}"))
    }
}

/// `systemctl --user is-active detent.service` (`active`, `inactive`, `failed`, ...).
pub fn service_state() -> String {
    systemctl(&["is-active", UNIT])
        .stderr(Stdio::null())
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

#[derive(Serialize)]
pub struct ServiceOut {
    pub unit: &'static str,
    pub active_state: String,
    pub sub_state: String,
    /// `enabled` = starts with the graphical session.
    pub unit_file_state: String,
    pub main_pid: u32,
}

fn show() -> Result<ServiceOut> {
    let out = systemctl(&[
        "show",
        UNIT,
        "-p",
        "ActiveState,SubState,UnitFileState,MainPID",
    ])
    .output()
    .map_err(|e| missing("systemctl", e))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let get = |k: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(k)?.strip_prefix('='))
            .unwrap_or("")
            .to_string()
    };
    Ok(ServiceOut {
        unit: UNIT,
        active_state: get("ActiveState"),
        sub_state: get("SubState"),
        unit_file_state: get("UnitFileState"),
        main_pid: get("MainPID").parse().unwrap_or(0),
    })
}

/// Wait until the daemon answers `status` (it binds the socket a moment after start).
fn wait_ready(ctx: &Ctx) -> Result<Status> {
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        if let Some(mut d) = ctx.try_daemon()?
            && let Ok(s) = d.call_as::<Status>(method::STATUS, json!({}))
        {
            return Ok(s);
        }
        if Instant::now() >= deadline {
            return Err(Exit::new(
                exit::UNAVAILABLE,
                format!(
                    "{UNIT} started but detentd did not answer within {}s: see `detentctl logs`",
                    READY_TIMEOUT.as_secs()
                ),
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub fn run(ctx: &Ctx, cmd: DaemonCmd) -> Result<()> {
    Ctx::require_daemon_os()?;
    let (verb, wait) = match &cmd {
        DaemonCmd::Start(w) => ("start", !w.no_wait),
        DaemonCmd::Restart(w) => ("restart", !w.no_wait),
        DaemonCmd::Stop => ("stop", false),
        DaemonCmd::Status => {
            if ctx.json {
                let s = show()?;
                return ctx.emit(&s, |_| {});
            }
            let st = systemctl(&["status", UNIT, "--no-pager"])
                .status()
                .map_err(|e| missing("systemctl", e))?;
            // systemctl's LSB codes: 3 = not running.
            return match st.code() {
                Some(0) => Ok(()),
                _ => Err(Exit::new(exit::UNAVAILABLE, format!("{UNIT} is not running")).into()),
            };
        }
    };
    ctx.debug(format_args!("systemctl --user {verb} {UNIT}"));
    let st = systemctl(&[verb, UNIT])
        .status()
        .map_err(|e| missing("systemctl", e))?;
    if !st.success() {
        return Err(Exit::new(
            exit::FAILURE,
            format!("`systemctl --user {verb} {UNIT}` failed (is the unit installed? `just install-service`)"),
        )
        .into());
    }
    if !wait {
        ctx.done(format!("{UNIT}: {verb} requested"));
        return Ok(());
    }
    let s = wait_ready(ctx).context("waiting for detentd")?;
    if ctx.json {
        return ctx.emit(
            &json!({"running": true, "version": s.version, "pid": s.pid}),
            |_| {},
        );
    }
    ctx.done(format!("detentd {} running (pid {})", s.version, s.pid));
    Ok(())
}

/// Replace this process with journalctl (pager, colors and Ctrl+C behave natively).
/// The compositor's unit: KWin logs there when it drops a client for a protocol error.
const COMPOSITOR_UNIT: &str = "plasma-kwin_wayland.service";

/// `journalctl` arguments selecting one plugin: its own lines (prefixed `detent-<name>:`),
/// the supervisor's lines about it (``plugin `<name>` …``) and KWin dropping a client.
fn plugin_filter(name: &str) -> Result<Vec<String>> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(Exit::new(exit::USAGE, format!("--plugin {name}: not a plugin name")).into());
    }
    Ok(vec![
        "-u".into(),
        COMPOSITOR_UNIT.into(),
        "-g".into(),
        format!("detent-{name}|plugin .{name}.|error in client communication"),
    ])
}

pub fn logs(ctx: &Ctx, a: &LogsArgs) -> Result<()> {
    Ctx::require_daemon_os()?;
    let mut c = Command::new("journalctl");
    c.args(["--user", "-u", UNIT, "-n", &a.lines.to_string()]);
    if let Some(p) = &a.plugin {
        c.args(plugin_filter(p)?);
    }
    if a.follow {
        c.arg("-f");
    }
    if let Some(s) = &a.since {
        c.args(["--since", s]);
    }
    if ctx.json {
        c.args(["-o", "json"]);
    }
    if ctx.quiet {
        c.arg("-q");
    }
    ctx.debug(format_args!("{c:?}"));
    #[cfg(unix)]
    let e = c.exec();
    // No exec(2) here; unreachable today (no daemon off Linux), kept so it builds.
    #[cfg(not(unix))]
    let e = match c.status() {
        Ok(_) => return Ok(()),
        Err(e) => e,
    };
    Err(missing("journalctl", e))
}

#[cfg(test)]
mod tests {
    use super::plugin_filter;

    #[test]
    fn plugin_filter_matches_its_lines_and_the_compositor() {
        let args = plugin_filter("radial").unwrap();
        assert_eq!(args[..3], ["-u", "plasma-kwin_wayland.service", "-g"]);
        assert!(args[3].starts_with("detent-radial|plugin .radial.|"));
        assert!(plugin_filter("").is_err());
        assert!(plugin_filter("a|b").is_err());
    }
}
