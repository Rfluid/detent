//! `settings list|get|set|unset`. The settable paths come from `detent_ipc::settings`
//! (one catalogue for daemon, CLI and the direct path).

use anyhow::Result;
use detent_ipc::method;
use detent_ipc::settings::{self, SettingInfo};
use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::{SetArgs, SettingsCmd};
use crate::cmd::devices::fetch_state;
use crate::ctx::Ctx;
use crate::direct;
use crate::exit::{self, Exit};
use crate::output::{human, state};

#[derive(Debug, Serialize)]
pub struct SetOut {
    pub path: String,
    pub value: String,
    /// Saved into the config file (`--persist`).
    pub persisted: bool,
    /// `daemon` (live until the next config reload unless persisted) or `direct`.
    pub via: &'static str,
    /// The section as the device reports it after the change (daemon only).
    pub reported: Value,
}

fn set(ctx: &Ctx, a: SetArgs) -> Result<()> {
    // Validate locally first: clear message and exit 65 even without the daemon.
    let s = settings::find(&a.path)
        .ok_or_else(|| Exit::new(exit::DATAERR, settings::unknown(&a.path)))?;
    s.parse(&a.value).map_err(|e| Exit::new(exit::DATAERR, e))?;
    let out = match ctx.try_daemon()? {
        Some(mut d) => {
            let reported = d.call(
                method::DEVICE_SET,
                ctx.dev_params(json!({"path": a.path, "value": a.value, "persist": a.persist})),
            )?;
            SetOut {
                path: a.path,
                value: a.value,
                persisted: a.persist,
                via: "daemon",
                reported,
            }
        }
        None => {
            if a.persist {
                Ctx::require_daemon_os()?;
                return Err(Exit::new(
                    exit::UNAVAILABLE,
                    "--persist needs the daemon running (or edit the config: `detentctl config edit`)",
                )
                .into());
            }
            direct::set(ctx.device.as_deref(), &a.path, &a.value)?;
            SetOut {
                path: a.path,
                value: a.value,
                persisted: false,
                via: "direct",
                reported: Value::Null,
            }
        }
    };
    if ctx.json {
        return ctx.emit(&out, |_| {});
    }
    let note = match (out.via, out.persisted) {
        (_, true) => "saved to config",
        ("daemon", false) => "until the next config reload; --persist saves it",
        _ if !detent_ipc::platform::DAEMON_SUPPORTED => "applied directly to the device",
        _ => "applied directly; detentd is not running",
    };
    ctx.done(format!("{} = {} ({note})", out.path, out.value));
    Ok(())
}

fn is_unknown_method(e: &anyhow::Error) -> bool {
    e.downcast_ref::<detent_ipc::ClientError>()
        .and_then(detent_ipc::ClientError::rpc_code)
        == Some(detent_ipc::code::METHOD_NOT_FOUND)
}

pub fn run(ctx: &Ctx, cmd: SettingsCmd) -> Result<()> {
    match cmd {
        SettingsCmd::List => {
            let rows: Vec<SettingInfo> = match ctx.try_daemon()? {
                Some(mut d) => {
                    match d.call_as(method::DEVICE_SCHEMA, ctx.dev_params(json!({}))) {
                        Ok(rows) => rows,
                        // A daemon older than `device.schema`: same result from the state.
                        Err(e) if is_unknown_method(&e) => settings::describe(
                            &d.call(method::DEVICE_GET, ctx.dev_params(json!({})))?,
                        ),
                        Err(e) => return Err(e),
                    }
                }
                None => settings::describe(&fetch_state(ctx)?),
            };
            ctx.emit(&rows, |st| human::settings(st, &rows))
        }
        SettingsCmd::Get { path } => {
            let st = fetch_state(ctx)?;
            // Settings read their reported value; anything else is a state path.
            if let Some(s) = settings::find(&path) {
                let v = &st[s.section]["reported"][s.field];
                if v.is_null() {
                    return Err(Exit::new(
                        exit::NOINPUT,
                        format!("this device doesn't report `{path}`"),
                    )
                    .into());
                }
                return ctx.emit(v, |_| match v {
                    Value::String(s) => println!("{s}"),
                    other => println!("{other}"),
                });
            }
            state::print(&st, Some(&path), ctx.json)
                .map_err(|e| Exit::new(exit::NOINPUT, e.to_string()).into())
        }
        SettingsCmd::Set(a) => set(ctx, a),
        SettingsCmd::Unset { path } => {
            if settings::find(&path).is_none() {
                return Err(Exit::new(exit::DATAERR, settings::unknown(&path)).into());
            }
            let reported = ctx.daemon()?.call(
                method::DEVICE_UNSET,
                ctx.dev_params(json!({ "path": path })),
            )?;
            if ctx.json {
                return ctx.emit(&json!({"path": path, "reported": reported}), |_| {});
            }
            ctx.done(format!(
                "{path}: live change dropped; the config value applies"
            ));
            Ok(())
        }
    }
}
