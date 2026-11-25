//! Per-invocation context: global flags, the daemon connection, output mode.

use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::Result;
use detent_ipc::{Client, ClientError};
use serde::Serialize;
use serde_json::Value;

use crate::cli::{ColorWhen, Global};
use crate::output::Style;

pub struct Ctx {
    pub json: bool,
    pub verbose: u8,
    pub quiet: bool,
    pub style: Style,
    pub socket: PathBuf,
    pub device: Option<String>,
}

/// `--color auto` honours NO_COLOR, TERM=dumb and whether `stream` is a terminal.
pub fn color_enabled(when: ColorWhen, is_tty: bool) -> bool {
    match when {
        ColorWhen::Always => true,
        ColorWhen::Never => false,
        ColorWhen::Auto => {
            is_tty
                && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
                && std::env::var("TERM").map_or(true, |t| t != "dumb")
        }
    }
}

impl Ctx {
    pub fn new(g: &Global) -> Self {
        Self {
            json: g.json,
            verbose: g.verbose,
            quiet: g.quiet,
            style: Style::new(color_enabled(g.color, std::io::stdout().is_terminal())),
            socket: g.socket.clone().unwrap_or_else(detent_ipc::socket_path),
            device: g.device.clone(),
        }
    }

    /// The daemon, or `None` if it isn't running (other connection errors are errors).
    /// Always `None` on OSes without a daemon yet: callers fall back to the device.
    pub fn try_daemon(&self) -> Result<Option<Daemon<'_>>> {
        if !detent_ipc::platform::DAEMON_SUPPORTED {
            self.debug(format_args!(
                "no daemon on this OS; using the device directly"
            ));
            return Ok(None);
        }
        match Client::connect_to(&self.socket) {
            Ok(c) => Ok(Some(Daemon { c, ctx: self })),
            Err(ClientError::NotRunning(_)) => {
                self.debug(format_args!("no daemon at {}", self.socket.display()));
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// The daemon, or a "not running" error (exit 69, with a hint). Off Linux: "not yet
    /// supported on <OS>" (exit 69), see [`Ctx::require_daemon_os`].
    pub fn daemon(&self) -> Result<Daemon<'_>> {
        Self::require_daemon_os()?;
        let c = Client::connect_to(&self.socket)?;
        Ok(Daemon { c, ctx: self })
    }

    /// Commands that need detentd (or its service/journal) fail here on OSes without one.
    pub fn require_daemon_os() -> Result<(), ClientError> {
        if detent_ipc::platform::DAEMON_SUPPORTED {
            Ok(())
        } else {
            Err(ClientError::Unsupported(detent_ipc::platform::unsupported(
                "the detentd daemon",
            )))
        }
    }

    /// `params` plus `device` from `--device`.
    pub fn dev_params(&self, mut extra: Value) -> Value {
        if let (Some(d), Value::Object(m)) = (self.device.as_deref(), &mut extra) {
            m.insert("device".into(), Value::from(d));
        }
        extra
    }

    /// Print `v` as JSON with `--json`, else run the human printer.
    pub fn emit<T: Serialize + ?Sized>(&self, v: &T, human: impl FnOnce(&Style)) -> Result<()> {
        if self.json {
            println!("{}", serde_json::to_string_pretty(v)?);
        } else {
            human(&self.style);
        }
        Ok(())
    }

    /// A confirmation line ("config reloaded"): human mode only, not with `-q`.
    pub fn done(&self, msg: impl std::fmt::Display) {
        if !self.quiet && !self.json {
            println!("{msg}");
        }
    }

    /// `-v` detail on stderr.
    pub fn debug(&self, msg: std::fmt::Arguments<'_>) {
        if self.verbose > 0 {
            eprintln!("{}", self.style.dim(&msg.to_string()));
        }
    }
}

/// A connected client that traces requests with `-vv`.
pub struct Daemon<'a> {
    c: Client,
    ctx: &'a Ctx,
}

impl Daemon<'_> {
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        if self.ctx.verbose > 1 {
            eprintln!("{}", self.ctx.style.dim(&format!("→ {method} {params}")));
        }
        let r = self.c.call(method, params);
        if self.ctx.verbose > 1 {
            match &r {
                Ok(v) => eprintln!("{}", self.ctx.style.dim(&format!("← {v}"))),
                Err(e) => eprintln!("{}", self.ctx.style.dim(&format!("← error: {e}"))),
            }
        }
        Ok(r?)
    }

    pub fn call_as<T: serde::de::DeserializeOwned>(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<T> {
        let v = self.call(method, params)?;
        serde_json::from_value(v)
            .map_err(|e| ClientError::Protocol(format!("{method}: {e}")).into())
    }

    pub fn into_client(self) -> Client {
        self.c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_choice() {
        assert!(color_enabled(ColorWhen::Always, false));
        assert!(!color_enabled(ColorWhen::Never, true));
        assert!(!color_enabled(ColorWhen::Auto, false));
    }
}
