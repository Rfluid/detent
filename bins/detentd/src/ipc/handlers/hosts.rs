//! `hosts.list`, `hosts.switch`, `hosts.rename` (Easy-Switch). Hosts are 1-based at the
//! edges (as printed on the mouse) and 0-based inside.

use std::sync::Arc;

use detent_engine::action::HostTarget;
use detent_hidpp_features::hosts::Target;
use detent_ipc::method;
use serde_json::Value;

use super::{RpcResult, ask, device_tx, invalid, str_param};
use crate::msg::{Cmd, HostOp};
use crate::shared::Shared;

/// `"2"` / `2` / `"next"` / `"prev"`.
pub fn parse_target(s: &str) -> Result<HostTarget, (i32, String)> {
    match Target::parse(s) {
        Some(Target::Slot(n)) => Ok(HostTarget::Slot(n)),
        Some(Target::Next) => Ok(HostTarget::Next),
        Some(Target::Prev) => Ok(HostTarget::Prev),
        None => Err(invalid(format!(
            "host `{s}`: use a number from 1, `next` or `prev`"
        ))),
    }
}

pub fn handle(shared: &Arc<Shared>, m: &str, params: &Value) -> RpcResult {
    let (_, tx) = device_tx(shared, params)?;
    let op = match m {
        method::HOSTS_SWITCH => HostOp::Switch(parse_target(&str_param(params, "to")?)?),
        method::HOSTS_RENAME => {
            let slot = match parse_target(&str_param(params, "host")?)? {
                HostTarget::Slot(s) => s,
                _ => return Err(invalid("rename needs a host number")),
            };
            HostOp::Rename {
                slot,
                name: str_param(params, "name")?,
            }
        }
        _ => HostOp::List,
    };
    ask(&tx, |reply| Cmd::Host { op, reply })?
}
