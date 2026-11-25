//! Client IPC server (docs/specs/ipc.md): JSON-RPC 2.0, one JSON object per line, over
//! `$XDG_RUNTIME_DIR/detent/ctl.sock` (or `$DETENT_SOCKET`). One thread per client; never
//! on the data plane: device requests go to the device's worker with a bounded wait.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;

use detent_ipc::{code, method};
use serde_json::{Value, json};
use tracing::{debug, info, warn};

use crate::shared::Shared;

pub(crate) mod handlers;

pub fn socket_path() -> PathBuf {
    detent_ipc::socket_path()
}

pub fn serve(shared: Arc<Shared>) {
    let path = socket_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            warn!("IPC disabled: cannot bind {}: {e}", path.display());
            return;
        }
    };
    info!("IPC listening on {}", path.display());
    for stream in listener.incoming().flatten() {
        let s = shared.clone();
        let _ = std::thread::Builder::new()
            .name("detent-ipc".into())
            .spawn(move || client(stream, s));
    }
}

fn client(stream: UnixStream, shared: Arc<Shared>) {
    let Ok(mut w) = stream.try_clone() else {
        return;
    };
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(
                    w,
                    "{}",
                    error(Value::Null, code::PARSE, &format!("parse error: {e}"))
                );
                continue;
            }
        };
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let m = req.get("method").and_then(Value::as_str).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(json!({}));
        if m == method::EVENTS_SUBSCRIBE {
            stream_events(&mut w, &shared, id, &params);
            return;
        }
        let reply = match handlers::handle(&shared, m, &params) {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err((code, msg)) => error(id, code, &msg),
        };
        if writeln!(w, "{reply}").is_err() {
            break;
        }
    }
}

fn error(id: Value, code: i32, msg: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": msg}})
}

/// `events.subscribe`: acknowledge, then stream notifications until the client leaves.
fn stream_events(w: &mut UnixStream, shared: &Shared, id: Value, params: &Value) {
    let topics: Vec<String> = params
        .get("topics")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|t| t.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let rx = shared.subscribe(topics);
    if writeln!(
        w,
        "{}",
        json!({"jsonrpc": "2.0", "id": id, "result": "subscribed"})
    )
    .is_err()
    {
        return;
    }
    while let Ok(ev) = rx.recv() {
        let msg = json!({"jsonrpc": "2.0", "method": "event", "params": ev});
        if writeln!(w, "{msg}").is_err() {
            debug!("watch client left");
            return;
        }
    }
}
