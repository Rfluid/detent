//! `plugins list|status|restart`, `actions list|invoke`, `bindings`.

use anyhow::{Context, Result};
use detent_config::listing::BindingRow;
use detent_ipc::method;
use detent_ipc::types::{ActionInfo, PluginInfo};
use serde_json::{Value, json};

use crate::cli::{ActionsCmd, BindingsArgs, PluginsCmd};
use crate::ctx::Ctx;
use crate::exit::{self, Exit};
use crate::output::human;

pub fn plugins(ctx: &Ctx, cmd: PluginsCmd) -> Result<()> {
    let mut d = ctx.daemon()?;
    match cmd {
        PluginsCmd::List => {
            let list: Vec<PluginInfo> = d.call_as(method::PLUGINS_LIST, json!({}))?;
            ctx.emit(&list, |st| human::plugins(st, &list))
        }
        PluginsCmd::Status { name } => {
            let list: Vec<PluginInfo> = d.call_as(method::PLUGINS_LIST, json!({}))?;
            let p = list.into_iter().find(|p| p.name == name).ok_or_else(|| {
                Exit::new(
                    exit::NOINPUT,
                    format!("no plugin `{name}` (see `detentctl plugins`)"),
                )
            })?;
            ctx.emit(&p, |st| human::plugin(st, &p))
        }
        PluginsCmd::Restart { name } => {
            d.call(method::PLUGINS_RESTART, json!({ "name": name }))?;
            ctx.done(format!("restarting {name}"));
            Ok(())
        }
    }
}

pub fn actions(ctx: &Ctx, cmd: ActionsCmd) -> Result<()> {
    let mut d = ctx.daemon()?;
    match cmd {
        ActionsCmd::List => {
            let list: Vec<ActionInfo> = d.call_as(method::ACTIONS_LIST, json!({}))?;
            ctx.emit(&list, |st| human::actions(st, &list))
        }
        ActionsCmd::Invoke(a) => {
            let p: Value = match &a.params {
                Some(p) => serde_json::from_str(p)
                    .map_err(|e| Exit::new(exit::DATAERR, format!("params must be JSON: {e}")))?,
                None => json!({}),
            };
            d.call(
                method::ACTIONS_INVOKE,
                json!({"action": a.action, "params": p}),
            )?;
            ctx.done(format!("invoked {}", a.action));
            Ok(())
        }
    }
}

/// From the daemon (the config it actually runs) unless `--file` is given or it isn't
/// running, then from the file.
pub fn bindings(ctx: &Ctx, a: &BindingsArgs) -> Result<()> {
    let daemon = match &a.file {
        Some(_) => None,
        None => ctx.try_daemon()?,
    };
    let from_daemon: Option<Vec<BindingRow>> = match daemon {
        Some(mut d) => match d.call_as(method::BINDINGS_LIST, json!({})) {
            Ok(rows) => Some(rows),
            // A daemon older than `bindings.list`: read the file instead.
            Err(e)
                if e.downcast_ref::<detent_ipc::ClientError>()
                    .and_then(detent_ipc::ClientError::rpc_code)
                    == Some(detent_ipc::code::METHOD_NOT_FOUND) =>
            {
                None
            }
            Err(e) => return Err(e),
        },
        None => None,
    };
    let mut rows: Vec<BindingRow> = match from_daemon {
        Some(rows) => rows,
        None => {
            let path = a.file.clone().unwrap_or_else(detent_config::default_path);
            let text =
                std::fs::read_to_string(&path).with_context(|| path.display().to_string())?;
            let cfg = detent_config::parse(&text)
                .map_err(|e| Exit::new(exit::CONFIG, format!("{}: {e}", path.display())))?;
            detent_config::listing::bindings(&cfg)
        }
    };
    if let Some(p) = &a.profile {
        rows.retain(|r| &r.profile == p);
    }
    ctx.emit(&rows, |st| human::bindings(st, &rows))
}
