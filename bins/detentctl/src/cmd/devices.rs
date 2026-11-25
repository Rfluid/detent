//! `devices list|info|battery` and `haptic`.

use anyhow::Result;
use detent_hidpp_features::haptics::waveform;
use detent_ipc::method;
use detent_ipc::types::DeviceSummary;
use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::{DevicesCmd, HapticArgs};
use crate::ctx::Ctx;
use crate::direct;
use crate::exit::{self, Exit};
use crate::output::{human, state};

#[derive(Debug, Serialize)]
pub struct DeviceRow {
    pub node: String,
    /// Receiver slot (1–6) for a device paired to a receiver at `node`; absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<u8>,
    pub bus: String,
    pub vendor: u32,
    /// The device's product id (its wpid behind a receiver).
    pub product: u32,
    pub name: String,
    /// The daemon's device id when it has attached this node; `null` otherwise (or no daemon).
    pub attached: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BatteryOut {
    pub device: String,
    pub percent: Option<u64>,
    /// `critical` | `low` | `good` | `full` (as the device reports it).
    pub level: Value,
    pub charging: Value,
    /// The full battery section the device reported.
    pub raw: Value,
}

/// The device state: from the daemon when it runs, else read directly from the device.
pub fn fetch_state(ctx: &Ctx) -> Result<Value> {
    match ctx.try_daemon()? {
        Some(mut d) => d.call(method::DEVICE_GET, ctx.dev_params(json!({}))),
        None => {
            ctx.debug(format_args!("reading the device directly"));
            Ok(serde_json::to_value(direct::read_state(
                ctx.device.as_deref(),
            )?)?)
        }
    }
}

pub fn battery_of(state: &Value) -> Result<BatteryOut> {
    let b = &state["battery"]["reported"];
    if b.is_null() {
        return Err(Exit::new(exit::NOINPUT, "this device reports no battery").into());
    }
    Ok(BatteryOut {
        device: state["identity"]["name"]
            .as_str()
            .unwrap_or("?")
            .to_string(),
        percent: b["percent"].as_u64(),
        level: b["level"].clone(),
        charging: b["charging"].clone(),
        raw: b.clone(),
    })
}

fn list(ctx: &Ctx) -> Result<()> {
    let attached: Vec<DeviceSummary> = match ctx.try_daemon()? {
        Some(mut d) => d.call_as(method::DEVICES_LIST, json!({}))?,
        None => Vec::new(),
    };
    let rows: Vec<DeviceRow> = direct::endpoints()?
        .into_iter()
        .map(|ep| {
            let node = ep.info.node.display().to_string();
            DeviceRow {
                attached: attached
                    .iter()
                    .find(|a| a.node == node && a.slot == ep.slot())
                    .map(|a| a.id.clone()),
                slot: ep.slot(),
                bus: format!("{:?}", ep.info.bus),
                vendor: ep.info.vendor,
                product: ep.product(),
                name: ep.name().to_string(),
                node,
            }
        })
        .collect();
    ctx.emit(&rows, |st| human::devices(st, &rows))
}

pub fn run(ctx: &Ctx, cmd: DevicesCmd) -> Result<()> {
    match cmd {
        DevicesCmd::List => list(ctx),
        DevicesCmd::Info => {
            let s = fetch_state(ctx)?;
            state::print(&s, None, ctx.json)
        }
        DevicesCmd::Battery => {
            let b = battery_of(&fetch_state(ctx)?)?;
            ctx.emit(&b, |st| human::battery(st, &b))
        }
    }
}

#[derive(Serialize)]
struct Waveform {
    id: u8,
    name: &'static str,
}

pub fn haptic(ctx: &Ctx, a: &HapticArgs) -> Result<()> {
    if a.list {
        let list: Vec<Waveform> = waveform::NAMES
            .iter()
            .map(|&(id, name)| Waveform { id, name })
            .collect();
        return ctx.emit(&list, |_| {
            for w in &list {
                println!("{:>3}  {}", w.id, w.name);
            }
        });
    }
    let w = a.waveform.as_deref().unwrap_or("damp-state-change");
    if waveform::by_name(w).is_none() && w.parse::<u8>().is_err() {
        return Err(Exit::new(
            exit::DATAERR,
            format!("unknown waveform `{w}` (see `detentctl haptic --list`)"),
        )
        .into());
    }
    ctx.daemon()?.call(
        method::HAPTIC_PLAY,
        ctx.dev_params(json!({ "waveform": w })),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_shape() {
        let st = json!({"identity": {"name": "MX Master 4"},
                        "battery": {"reported": {"percent": 85, "level": "good", "charging": "discharging"}}});
        let b = battery_of(&st).unwrap();
        let v = serde_json::to_value(&b).unwrap();
        assert_eq!(v["device"], "MX Master 4");
        assert_eq!(v["percent"], 85);
        assert_eq!(v["level"], "good");
        assert!(battery_of(&json!({"battery": {"reported": null}})).is_err());
    }
}
