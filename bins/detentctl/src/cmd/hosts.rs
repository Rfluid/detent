//! `hosts list|switch|rename`: Easy-Switch slots (the computers the mouse is paired with).
//! Through the daemon when it runs, else straight to the device. Hosts are 1-based here,
//! as printed on the mouse.

use anyhow::Result;
use detent_hidpp_features::hosts::{self, Target};
use detent_ipc::method;
use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::{HostsCmd, RenameArgs};
use crate::ctx::Ctx;
use crate::direct;
use crate::exit::{self, Exit};

#[derive(Debug, Serialize)]
pub struct SwitchOut {
    pub switched: bool,
    pub from: Value,
    pub to: Value,
    /// `daemon` or `direct`.
    pub via: &'static str,
}

fn dataerr(msg: impl Into<String>) -> anyhow::Error {
    Exit::new(exit::DATAERR, msg.into()).into()
}

fn is_unknown_method(e: &anyhow::Error) -> bool {
    e.downcast_ref::<detent_ipc::ClientError>()
        .and_then(detent_ipc::ClientError::rpc_code)
        == Some(detent_ipc::code::METHOD_NOT_FOUND)
}

/// This computer's short host name (`fedora.local` → `fedora`), as Solaar uses it.
pub fn this_host() -> Result<String> {
    let from_file = std::fs::read_to_string("/proc/sys/kernel/hostname").ok();
    let from_env = || std::env::var("COMPUTERNAME").ok();
    let from_cmd = || {
        std::process::Command::new("hostname")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let name = from_file
        .or_else(from_env)
        .or_else(from_cmd)
        .map(|h| h.trim().split('.').next().unwrap_or("").to_string())
        .filter(|h| !h.is_empty())
        .ok_or_else(|| anyhow::anyhow!("couldn't read this computer's host name"))?;
    Ok(name)
}

/// The hosts section: `hosts.list`, an older daemon's `device.get`, or the device.
fn list(ctx: &Ctx) -> Result<Value> {
    let v = match ctx.try_daemon()? {
        Some(mut d) => match d.call(method::HOSTS_LIST, ctx.dev_params(json!({}))) {
            Ok(v) => v,
            Err(e) if is_unknown_method(&e) => {
                d.call(method::DEVICE_GET, ctx.dev_params(json!({})))?["hosts"]["reported"].clone()
            }
            Err(e) => return Err(e),
        },
        None => serde_json::to_value(direct::hosts::read(ctx.device.as_deref())?)?,
    };
    if v.is_null() {
        return Err(Exit::new(exit::NOINPUT, "this device has no Easy-Switch hosts").into());
    }
    Ok(v)
}

fn switch(ctx: &Ctx, to: &str) -> Result<()> {
    let t =
        Target::parse(to).ok_or_else(|| dataerr(format!("host `{to}`: use 1-3, next or prev")))?;
    let out = match ctx.try_daemon()? {
        Some(mut d) => {
            let r = d.call(method::HOSTS_SWITCH, ctx.dev_params(json!({ "to": to })))?;
            SwitchOut {
                switched: r["switched"].as_bool().unwrap_or(false),
                from: r["from"].clone(),
                to: r["to"].clone(),
                via: "daemon",
            }
        }
        None => {
            let (from, to) = direct::hosts::switch(ctx.device.as_deref(), t)?;
            SwitchOut {
                switched: from != to,
                from: json!(from + 1),
                to: json!(to + 1),
                via: "direct",
            }
        }
    };
    if ctx.json {
        return ctx.emit(&out, |_| {});
    }
    if out.switched {
        ctx.done(format!(
            "switched to host {}: the mouse now talks to that computer",
            out.to
        ));
    } else {
        ctx.done(format!("already on host {}", out.to));
    }
    Ok(())
}

fn rename(ctx: &Ctx, a: RenameArgs) -> Result<()> {
    let slot = match Target::parse(&a.host) {
        Some(Target::Slot(s)) => s,
        _ => return Err(dataerr(format!("host `{}`: use a host number", a.host))),
    };
    let name = match a.name {
        Some(n) => n,
        None => this_host()?,
    };
    hosts::validate_name(&name, None).map_err(|e| dataerr(e.to_string()))?;
    let v = match ctx.try_daemon()? {
        Some(mut d) => d.call(
            method::HOSTS_RENAME,
            ctx.dev_params(json!({ "host": a.host, "name": name })),
        )?,
        None => serde_json::to_value(direct::hosts::rename(ctx.device.as_deref(), slot, &name)?)?,
    };
    let stored = v["slots"][slot as usize]["name"].as_str().unwrap_or("");
    if ctx.json {
        return ctx.emit(&v, |_| {});
    }
    if stored == name {
        ctx.done(format!("host {} is now \"{name}\"", slot + 1));
    } else {
        ctx.done(format!(
            "host {}: wrote \"{name}\", the mouse reports \"{stored}\"",
            slot + 1
        ));
    }
    Ok(())
}

pub fn run(ctx: &Ctx, cmd: HostsCmd) -> Result<()> {
    match cmd {
        HostsCmd::List => {
            let v = list(ctx)?;
            ctx.emit(&v, |st| crate::output::human::hosts(st, &v))
        }
        HostsCmd::Switch { to } => switch(ctx, &to),
        HostsCmd::Rename(a) => rename(ctx, a),
    }
}
