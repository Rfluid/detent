//! IPC method handlers. Each returns the JSON result or `(code, message)`.

use std::sync::Arc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::time::Duration;

use detent_ipc::types::{PluginInfo, RemapState, Status};
use detent_ipc::{code, method};
use serde_json::{Value, json};

use crate::msg::{Cmd, WorkerMsg};
use crate::persist;
use crate::shared::{Shared, lock};

const REPLY_TIMEOUT: Duration = Duration::from_secs(10);

type RpcResult = Result<Value, (i32, String)>;

fn invalid(msg: impl Into<String>) -> (i32, String) {
    (code::INVALID_PARAMS, msg.into())
}

/// Pick the device: `params.device` matches key, name or node; default the first one.
fn device_tx(
    shared: &Shared,
    params: &Value,
) -> Result<(String, SyncSender<WorkerMsg>), (i32, String)> {
    let devs = lock(&shared.devices);
    let want = params.get("device").and_then(Value::as_str);
    let found = devs.iter().find(|(k, d)| match want {
        None => true,
        Some(w) => {
            let w = w.to_lowercase();
            k.to_lowercase() == w
                || d.node == w
                || d.node.ends_with(&w)
                || d.name.to_lowercase().contains(&w)
        }
    });
    found.map(|(k, d)| (k.clone(), d.tx.clone())).ok_or((
        code::DEVICE,
        match want {
            Some(w) => format!("no attached device matches `{w}`"),
            None => "no device attached (is the mouse connected and awake?)".to_string(),
        },
    ))
}

/// Send a command to a worker and wait (bounded) for its reply. Runs on an IPC thread.
fn ask<T>(
    tx: &SyncSender<WorkerMsg>,
    mk: impl FnOnce(SyncSender<T>) -> Cmd,
) -> Result<T, (i32, String)> {
    let (rtx, rrx) = sync_channel(1);
    tx.send(WorkerMsg::Cmd(mk(rtx)))
        .map_err(|_| (code::DEVICE, "device disconnected".to_string()))?;
    rrx.recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| (code::DEVICE, "device did not answer in time".to_string()))
}

fn str_param(p: &Value, k: &str) -> Result<String, (i32, String)> {
    match p.get(k) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(v @ (Value::Number(_) | Value::Bool(_))) => Ok(v.to_string()),
        _ => Err(invalid(format!("`{k}` is required"))),
    }
}

pub fn status(shared: &Shared) -> Status {
    let cfg = shared.config.load();
    Status {
        version: env!("CARGO_PKG_VERSION").to_string(),
        pid: std::process::id(),
        uptime_s: shared.started.elapsed().as_secs(),
        config_path: shared.config_path.display().to_string(),
        config_error: lock(&shared.config_error).clone(),
        bindings: cfg.source.bind.len()
            + cfg
                .source
                .layer
                .values()
                .map(|l| l.bindings.len())
                .sum::<usize>(),
        profiles: cfg.profiles.iter().map(|p| p.name.clone()).collect(),
        active_profile: shared.active_profile_name(),
        context: lock(&shared.context).clone(),
        remap_enabled: shared.remap_enabled(),
        devices: lock(&shared.devices)
            .iter()
            .map(|(k, d)| d.summary(k))
            .collect(),
        plugins: serde_json::from_value::<Vec<PluginInfo>>(shared.plugins_summary())
            .unwrap_or_default(),
    }
}

fn to_json<T: serde::Serialize>(v: &T) -> RpcResult {
    serde_json::to_value(v).map_err(|e| (code::INVALID_PARAMS, e.to_string()))
}

#[expect(
    clippy::too_many_lines,
    reason = "predates the lint; split it when next changed (ADR 0008)"
)]
pub fn handle(shared: &Arc<Shared>, m: &str, params: &Value) -> RpcResult {
    match m {
        method::STATUS => to_json(&status(shared)),
        method::DEVICES_LIST => Ok(Value::Array(
            lock(&shared.devices)
                .iter()
                .map(|(k, d)| serde_json::to_value(d.summary(k)).unwrap_or_default())
                .collect(),
        )),
        method::DEVICE_GET | method::DEVICE_FEATURES | method::DEVICE_SCHEMA => {
            let (_, tx) = device_tx(shared, params)?;
            let state = ask(&tx, Cmd::Get)?;
            Ok(match m {
                method::DEVICE_FEATURES => state.get("features").cloned().unwrap_or(Value::Null),
                method::DEVICE_SCHEMA => to_json(&detent_ipc::settings::describe(&state))?,
                _ => state,
            })
        }
        method::DEVICE_SET => {
            let (key, tx) = device_tx(shared, params)?;
            let path = str_param(params, "path")?;
            let value = str_param(params, "value")?;
            let persist_it = params
                .get("persist")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let result = ask(&tx, |reply| Cmd::Set {
                path: path.clone(),
                value: value.clone(),
                reply,
            })?
            .map_err(invalid)?;
            if persist_it {
                let state = ask(&tx, Cmd::Get)?;
                let name = state["identity"]["name"].as_str().unwrap_or("").to_string();
                persist::write(&shared.config_path, &name, &path, &value).map_err(|e| {
                    (
                        code::CONFIG,
                        format!("applied, but saving to the config failed: {e}"),
                    )
                })?;
            }
            shared.publish(
                "setting",
                json!({"device": key, "path": path, "value": value, "persist": persist_it}),
            );
            Ok(result)
        }
        method::DEVICE_UNSET => {
            let (key, tx) = device_tx(shared, params)?;
            let path = str_param(params, "path")?;
            let result = ask(&tx, |reply| Cmd::Unset {
                path: path.clone(),
                reply,
            })?
            .map_err(invalid)?;
            shared.publish(
                "setting",
                json!({"device": key, "path": path, "value": null, "unset": true}),
            );
            Ok(result)
        }
        method::DEVICE_RAW => {
            let (_, tx) = device_tx(shared, params)?;
            let feature = params
                .get("feature")
                .and_then(Value::as_u64)
                .ok_or_else(|| invalid("feature (number) required"))?
                as u16;
            let function = params
                .get("function")
                .and_then(Value::as_u64)
                .ok_or_else(|| invalid("function (number) required"))?
                as u8;
            let p: Vec<u8> = params
                .get("params")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_u64)
                        .map(|v| v as u8)
                        .collect()
                })
                .unwrap_or_default();
            let r = ask(&tx, |reply| Cmd::Raw {
                feature,
                function,
                params: p,
                reply,
            })?
            .map_err(|e| (code::HIDPP, e))?;
            Ok(json!(r))
        }
        method::HAPTIC_PLAY => {
            let (_, tx) = device_tx(shared, params)?;
            let w = params
                .get("waveform")
                .and_then(Value::as_str)
                .unwrap_or("damp-state-change");
            let id = detent_hidpp_features::haptics::waveform::by_name(w)
                .or_else(|| w.parse().ok())
                .ok_or_else(|| invalid(format!("unknown waveform `{w}`")))?;
            tx.send(WorkerMsg::Cmd(Cmd::Haptic(id)))
                .map_err(|_| (code::DEVICE, "device disconnected".to_string()))?;
            Ok(json!("ok"))
        }
        method::REMAP_GET => to_json(&RemapState {
            enabled: shared.remap_enabled(),
            changed: false,
        }),
        method::REMAP_SET => {
            let enabled = params
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or_else(|| invalid("`enabled` (bool) is required"))?;
            let changed = shared.set_remap(enabled);
            to_json(&RemapState { enabled, changed })
        }
        method::BINDINGS_LIST => to_json(&detent_config::listing::bindings(
            &shared.config.load().source,
        )),
        method::CONFIG_RELOAD => match crate::config_watch::reload(shared) {
            Ok(()) => Ok(json!("ok")),
            Err(e) => Err((code::CONFIG, e)),
        },
        method::CONFIG_VALIDATE => {
            let text = str_param(params, "text")?;
            detent_config::load(&text)
                .map(|_| json!("ok"))
                .map_err(|e| (code::CONFIG, e.to_string()))
        }
        method::ACTIONS_LIST => Ok(shared.actions_list()),
        method::ACTIONS_INVOKE => {
            let action = str_param(params, "action")?;
            let p = params
                .get("params")
                .map(Value::to_string)
                .unwrap_or_else(|| "{}".into());
            shared
                .invoke_named(&action, &p, "detentctl")
                .map(|()| json!("ok"))
                .map_err(invalid)
        }
        method::PLUGINS_LIST => Ok(shared.plugins_summary()),
        method::PLUGINS_RESTART => {
            let name = str_param(params, "name")?;
            shared
                .restart_plugin(&name)
                .map(|()| json!("ok"))
                .map_err(invalid)
        }
        other => Err((code::METHOD_NOT_FOUND, format!("unknown method `{other}`"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared() -> Arc<Shared> {
        let cfg =
            detent_config::load(include_str!("../../../../config/detent.example.toml")).unwrap();
        let (tx, _rx) = sync_channel(1);
        Arc::new(Shared::new(cfg, "/x.toml".into(), tx))
    }

    #[test]
    fn status_and_remap_round_trip() {
        let s = shared();
        let st: Status =
            serde_json::from_value(handle(&s, method::STATUS, &json!({})).unwrap()).unwrap();
        assert!(st.remap_enabled);
        assert!(st.bindings > 0);
        assert_eq!(st.pid, std::process::id());

        let r = handle(&s, method::REMAP_SET, &json!({"enabled": false})).unwrap();
        assert_eq!(r, json!({"enabled": false, "changed": true}));
        let r = handle(&s, method::REMAP_GET, &json!({})).unwrap();
        assert_eq!(r["enabled"], false);
        assert_eq!(
            handle(&s, method::REMAP_SET, &json!({})).unwrap_err().0,
            code::INVALID_PARAMS
        );
    }

    #[test]
    fn device_methods_without_devices_and_unknown_methods() {
        let s = shared();
        assert_eq!(
            handle(&s, method::DEVICE_SCHEMA, &json!({})).unwrap_err().0,
            code::DEVICE
        );
        assert_eq!(
            handle(&s, "nope", &json!({})).unwrap_err().0,
            code::METHOD_NOT_FOUND
        );
        let b = handle(&s, method::BINDINGS_LIST, &json!({})).unwrap();
        assert!(b.as_array().is_some_and(|a| !a.is_empty()));
    }
}
