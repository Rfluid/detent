//! Blocking JSON-RPC client (detentctl, tray, settings GUI, scripts).
//!
//! Layout: the client over a Unix stream socket; `client/no_socket.rs` stands in for the
//! stream on OSes without Unix sockets (Windows), where connecting always fails with
//! [`ClientError::Unsupported`] (ADR 0010).

#[cfg(not(unix))]
mod no_socket;

use std::io::{BufRead, BufReader, ErrorKind, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::Path;

#[cfg(not(unix))]
use no_socket::UnixStream;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::types::{RemapState, Status};
use crate::{method, socket_path};

/// Default reply timeout: the daemon answers device reads within its own 10 s limit.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Nothing listens on the socket (no file, or a stale file after a crash).
    #[error("detentd is not running (no daemon at {0})")]
    NotRunning(String),
    #[error("IPC: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Rpc(RpcError),
    #[error("bad reply from detentd: {0}")]
    Protocol(String),
    /// No daemon on this OS yet; the message says where to follow support (ADR 0010).
    #[error("{0}")]
    Unsupported(String),
}

impl ClientError {
    /// The JSON-RPC error code, for `Rpc` errors.
    pub fn rpc_code(&self) -> Option<i32> {
        match self {
            Self::Rpc(e) => Some(e.code),
            _ => None,
        }
    }
}

pub struct Client {
    w: UnixStream,
    r: BufReader<UnixStream>,
    next: u64,
}

impl Client {
    /// Connect to [`socket_path`] (`$DETENT_SOCKET` or the runtime dir).
    pub fn connect() -> Result<Self, ClientError> {
        Self::connect_to(&socket_path())
    }

    pub fn connect_to(path: &Path) -> Result<Self, ClientError> {
        let s = UnixStream::connect(path).map_err(|e| match e.kind() {
            ErrorKind::NotFound | ErrorKind::ConnectionRefused => {
                ClientError::NotRunning(path.display().to_string())
            }
            ErrorKind::Unsupported => {
                ClientError::Unsupported(crate::platform::unsupported("detentd"))
            }
            _ => ClientError::Io(e),
        })?;
        s.set_read_timeout(Some(DEFAULT_TIMEOUT))?;
        Ok(Self {
            r: BufReader::new(s.try_clone()?),
            w: s,
            next: 1,
        })
    }

    /// `None` waits forever.
    pub fn set_timeout(&self, t: Option<Duration>) -> Result<(), ClientError> {
        self.w.set_read_timeout(t)?;
        Ok(())
    }

    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, ClientError> {
        let id = self.next;
        self.next += 1;
        let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.w, "{req}")?;
        let mut line = String::new();
        if self.r.read_line(&mut line)? == 0 {
            return Err(ClientError::Protocol("connection closed".into()));
        }
        let v: Value =
            serde_json::from_str(&line).map_err(|e| ClientError::Protocol(e.to_string()))?;
        if let Some(e) = v.get("error") {
            let e: RpcError = serde_json::from_value(e.clone())
                .map_err(|e| ClientError::Protocol(e.to_string()))?;
            return Err(ClientError::Rpc(e));
        }
        Ok(v.get("result").cloned().unwrap_or(Value::Null))
    }

    /// `call` and deserialize the result.
    pub fn call_as<T: DeserializeOwned>(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<T, ClientError> {
        let v = self.call(method, params)?;
        serde_json::from_value(v).map_err(|e| ClientError::Protocol(e.to_string()))
    }

    pub fn status(&mut self) -> Result<Status, ClientError> {
        self.call_as(method::STATUS, json!({}))
    }

    /// Is remapping (bindings, diversion, gestures) active?
    pub fn remap(&mut self) -> Result<RemapState, ClientError> {
        self.call_as(method::REMAP_GET, json!({}))
    }

    /// Pause (`false`) or resume (`true`) remapping without stopping the daemon.
    pub fn set_remap(&mut self, enabled: bool) -> Result<RemapState, ClientError> {
        self.call_as(method::REMAP_SET, json!({ "enabled": enabled }))
    }

    /// Subscribe and call `f` for every event until it returns false or the daemon exits.
    /// Empty `topics` = everything.
    pub fn watch(
        mut self,
        topics: &[String],
        mut f: impl FnMut(Value) -> bool,
    ) -> Result<(), ClientError> {
        self.set_timeout(None)?;
        self.call(method::EVENTS_SUBSCRIBE, json!({ "topics": topics }))?;
        let mut line = String::new();
        loop {
            line.clear();
            if self.r.read_line(&mut line)? == 0 {
                return Ok(());
            }
            let v: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if !f(v.get("params").cloned().unwrap_or(Value::Null)) {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
#[cfg(unix)] // a fake daemon on a Unix socket
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

    use serde_json::{Value, json};

    use super::*;
    use crate::code;

    fn temp_socket(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("detent-ipc-test-{}-{name}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("ctl.sock");
        let _ = std::fs::remove_file(&p);
        p
    }

    /// A one-connection fake daemon: answers each request with `reply(method, params)`.
    fn fake_daemon(
        path: &Path,
        reply: impl Fn(&str, &Value) -> Value + Send + 'static,
    ) -> std::thread::JoinHandle<()> {
        let l = UnixListener::bind(path).expect("bind");
        std::thread::spawn(move || {
            let (s, _) = l.accept().expect("accept");
            let mut w = s.try_clone().expect("clone");
            for line in BufReader::new(s).lines() {
                let Ok(line) = line else { break };
                let req: Value = serde_json::from_str(&line).expect("json");
                let m = req["method"].as_str().unwrap_or("").to_string();
                let id = req["id"].clone();
                if m == method::EVENTS_SUBSCRIBE {
                    writeln!(
                        w,
                        "{}",
                        json!({"jsonrpc":"2.0","id":id,"result":"subscribed"})
                    )
                    .expect("w");
                    for i in 0..3 {
                        let ev = json!({"jsonrpc":"2.0","method":"event","params":{"topic":"remap","n":i}});
                        // The client may hang up after the events it wanted.
                        if writeln!(w, "{ev}").is_err() {
                            return;
                        }
                    }
                    return;
                }
                let body = reply(&m, &req["params"]);
                let msg = if body.get("code").is_some() {
                    json!({"jsonrpc":"2.0","id":id,"error":body})
                } else {
                    json!({"jsonrpc":"2.0","id":id,"result":body})
                };
                writeln!(w, "{msg}").expect("w");
            }
        })
    }

    #[test]
    fn not_running_when_no_socket() {
        let p = temp_socket("none");
        match Client::connect_to(&p) {
            Err(ClientError::NotRunning(s)) => assert!(s.contains("ctl.sock")),
            Err(e) => panic!("unexpected error {e}"),
            Ok(_) => panic!("connected to nothing"),
        }
    }

    #[test]
    fn round_trip_typed_and_errors() {
        let p = temp_socket("rt");
        let h = fake_daemon(&p, |m, params| match m {
            "remap.set" => json!({"enabled": params["enabled"], "changed": true}),
            "status" => json!({
                "version": "9.9.9", "pid": 42, "uptime_s": 7,
                "config_path": "/c.toml", "config_error": null, "bindings": 3,
                "profiles": ["firefox"], "active_profile": "base", "context": {"app": "x"},
                "remap_enabled": false, "devices": [{"id": "B042:1", "name": "MX", "node": "/dev/hidraw9"}],
                "plugins": [{"name": "kde", "state": "connected"}]
            }),
            _ => json!({"code": code::METHOD_NOT_FOUND, "message": "unknown method"}),
        });
        let mut c = Client::connect_to(&p).expect("connect");
        let r = c.set_remap(false).expect("remap");
        assert!(!r.enabled && r.changed);
        let s = c.status().expect("status");
        assert_eq!(s.pid, 42);
        assert!(!s.remap_enabled);
        assert_eq!(s.devices[0].node, "/dev/hidraw9");
        assert_eq!(s.devices[0].frames, 0, "stats default when absent");
        assert_eq!(s.plugins[0].name, "kde");
        let e = c.call("nope", json!({})).expect_err("error");
        assert_eq!(e.rpc_code(), Some(code::METHOD_NOT_FOUND));
        drop(c);
        h.join().expect("join");
    }

    #[test]
    fn watch_streams_until_callback_stops() {
        let p = temp_socket("watch");
        let h = fake_daemon(&p, |_, _| json!(null));
        let c = Client::connect_to(&p).expect("connect");
        let mut seen = Vec::new();
        c.watch(&["remap".into()], |ev| {
            seen.push(ev["n"].as_u64().unwrap_or(99));
            seen.len() < 2
        })
        .expect("watch");
        assert_eq!(seen, vec![0, 1]);
        h.join().expect("join");
    }
}
