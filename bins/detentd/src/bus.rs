//! Plugin bus server + supervisor (docs/specs/plugin-bus.md, docs/architecture/plugins.md).
//!
//! - `SOCK_SEQPACKET` at `$XDG_RUNTIME_DIR/detent/bus.sock`, one message per packet.
//! - One reader thread per connection; sends are non-blocking (`MSG_DONTWAIT`): a full
//!   plugin socket drops the message and counts it. Nothing here runs on the data plane.
//! - The supervisor starts enabled plugins, restarts crashed ones with exponential backoff,
//!   pings every 2 s (3 missed → killed and restarted), and applies config reloads.
//!
//! Layout: this file owns the state, the accept loop and the handshake; `handle` answers
//! plugin messages (capabilities, leases); `supervisor` runs the processes; `sink` is the
//! [`ActionSink`](crate::shared::ActionSink) the rest of the daemon talks to.

mod handle;
mod sink;
mod supervisor;

use std::collections::BTreeMap;
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use detent_bus::{
    Capability, Codec, DaemonToPlugin, Event, LeaseRevokeReason, MAX_PACKET, Manifest,
    PROTOCOL_VERSION, PluginToDaemon, Topic, codec,
};
use detent_config::{CompiledConfig, NativePlugin};
use detent_engine::engine::Lease;
use rustix::net::{
    AddressFamily, RecvFlags, SendFlags, SocketAddrUnix, SocketType, accept, bind, listen, recv,
    send, socket,
};
use serde_json::{Value, json};
use tracing::{debug, info, warn};

use crate::shared::{Shared, lock};
use handle::send_lease;
use supervisor::Proc;

struct Conn {
    fd: Arc<OwnedFd>,
    codec: Codec,
    manifest: Manifest,
    granted: Vec<Capability>,
    topics: Vec<Topic>,
    dropped: Arc<AtomicU64>,
    missed_pings: u32,
    connected_at: Instant,
}

struct ActiveLease {
    plugin: String,
    id: u32,
}

#[derive(Default)]
struct State {
    lease: Option<ActiveLease>,
    conns: BTreeMap<String, Conn>,
    procs: BTreeMap<String, Proc>,
}

pub struct Bus {
    state: Mutex<State>,
    shared: Weak<Shared>,
    nonce: AtomicU64,
}

fn send_raw(fd: &OwnedFd, bytes: &[u8]) -> bool {
    send(fd, bytes, SendFlags::DONTWAIT | SendFlags::NOSIGNAL).is_ok()
}

impl Conn {
    fn send(&self, msg: &DaemonToPlugin) -> bool {
        match codec::encode(self.codec, msg) {
            Ok(b) if send_raw(&self.fd, &b) => true,
            _ => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    fn has(&self, c: &Capability) -> bool {
        self.granted.contains(c)
    }
}

/// Capabilities the config allows `name` to have.
fn allowed_caps(cfg: &CompiledConfig, name: &str) -> Vec<Capability> {
    let explicit = cfg
        .source
        .plugins
        .get(name)
        .and_then(|p| p.caps.as_ref())
        .map(|c| {
            c.iter()
                .filter_map(|s| Capability::from_name(s))
                .collect::<Vec<_>>()
        });
    match explicit {
        Some(c) => c,
        None => NativePlugin::from_name(name)
            .map(|n| {
                n.default_caps()
                    .iter()
                    .filter_map(|s| Capability::from_name(s))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// `[plugins.<name>]` settings (minus reserved keys) as JSON.
fn settings_json(cfg: &CompiledConfig, name: &str) -> String {
    cfg.source
        .plugins
        .get(name)
        .map(|p| serde_json::to_string(&p.settings).unwrap_or_else(|_| "{}".into()))
        .unwrap_or_else(|| "{}".into())
}

impl Bus {
    pub fn new(shared: &Arc<Shared>) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State::default()),
            shared: Arc::downgrade(shared),
            nonce: AtomicU64::new(1),
        })
    }

    fn shared(&self) -> Option<Arc<Shared>> {
        self.shared.upgrade()
    }

    /// Accept loop. Runs forever on its own thread.
    pub fn serve(self: Arc<Self>) {
        let path = detent_bus::socket_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        }
        let _ = std::fs::remove_file(&path);
        let listener =
            match socket(AddressFamily::UNIX, SocketType::SEQPACKET, None).and_then(|fd| {
                let addr = SocketAddrUnix::new(&path)?;
                bind(&fd, &addr)?;
                listen(&fd, 16)?;
                Ok(fd)
            }) {
                Ok(fd) => fd,
                Err(e) => {
                    warn!("plugin bus disabled: {}: {e}", path.display());
                    return;
                }
            };
        info!("plugin bus listening on {}", path.display());
        loop {
            match accept(&listener) {
                Ok(fd) => {
                    let me = self.clone();
                    let _ = std::thread::Builder::new()
                        .name("detent-bus-conn".into())
                        .spawn(move || me.connection(fd));
                }
                Err(e) => {
                    warn!("bus accept: {e}");
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }

    fn connection(self: Arc<Self>, fd: OwnedFd) {
        let fd = Arc::new(fd);
        let mut buf = vec![0u8; MAX_PACKET];
        let n = match recv(&*fd, &mut buf[..], RecvFlags::empty()) {
            Ok((n, _)) if n > 0 => n,
            _ => return,
        };
        let codec = codec::sniff(&buf[..n]);
        let manifest = match codec::decode::<PluginToDaemon>(codec, &buf[..n]) {
            Ok(PluginToDaemon::Hello(m)) => m,
            Ok(other) => {
                warn!("bus: first message must be Hello, got {other:?}");
                return;
            }
            Err(e) => {
                warn!("bus: bad Hello: {e}");
                return;
            }
        };
        let name = manifest.name.clone();
        if manifest.protocol != PROTOCOL_VERSION {
            warn!(
                "plugin `{name}` speaks protocol {}, daemon {PROTOCOL_VERSION}; rejected",
                manifest.protocol
            );
            return;
        }
        let Some(shared) = self.shared() else { return };
        let cfg = shared.config.load_full();
        let allowed = allowed_caps(&cfg, &name);
        let granted: Vec<Capability> = manifest
            .caps
            .iter()
            .filter(|c| allowed.contains(c))
            .cloned()
            .collect();
        let denied: Vec<&str> = manifest
            .caps
            .iter()
            .filter(|c| !allowed.contains(c))
            .map(Capability::name)
            .collect();
        if !denied.is_empty() {
            warn!(
                "plugin `{name}`: capabilities not granted: {} (add them to [plugins.{name}] caps)",
                denied.join(", ")
            );
        }
        let conn = Conn {
            fd: fd.clone(),
            codec,
            manifest: manifest.clone(),
            granted: granted.clone(),
            topics: Vec::new(),
            dropped: Arc::new(AtomicU64::new(0)),
            missed_pings: 0,
            connected_at: Instant::now(),
        };
        conn.send(&DaemonToPlugin::Welcome {
            protocol: PROTOCOL_VERSION,
            granted,
        });
        conn.send(&DaemonToPlugin::Config {
            settings_json: settings_json(&cfg, &name),
        });
        if let Some(old) = lock(&self.state).conns.insert(name.clone(), conn) {
            old.send(&DaemonToPlugin::Shutdown);
        }
        info!(
            "plugin `{name}` {} connected ({} actions)",
            manifest.version,
            manifest.actions.len()
        );
        shared.publish("plugin", json!({"plugin": name, "connected": true}));

        loop {
            let n = match recv(&*fd, &mut buf[..], RecvFlags::empty()) {
                Ok((n, _)) if n > 0 => n,
                _ => break,
            };
            match codec::decode::<PluginToDaemon>(codec, &buf[..n]) {
                Ok(m) => self.handle(&shared, &name, &fd, m),
                Err(e) => debug!("plugin `{name}`: undecodable message: {e}"),
            }
        }
        let held_lease = {
            let mut st = lock(&self.state);
            if st.conns.get(&name).is_some_and(|c| Arc::ptr_eq(&c.fd, &fd)) {
                st.conns.remove(&name);
            }
            let held = st.lease.as_ref().is_some_and(|l| l.plugin == name);
            if held {
                st.lease = None;
            }
            held
        };
        if held_lease {
            // A crashed plugin must never keep the cursor.
            send_lease(&shared, Lease::default());
            info!("plugin `{name}` gone: its lease was revoked");
        }
        info!("plugin `{name}` disconnected");
        shared.publish("plugin", json!({"plugin": name, "connected": false}));
    }

    pub fn send_to(&self, name: &str, msg: &DaemonToPlugin) -> bool {
        lock(&self.state)
            .conns
            .get(name)
            .is_some_and(|c| c.send(msg))
    }

    /// Forward a published event to subscribed plugins.
    pub fn publish(&self, topic: &str, ev: &Value) {
        let (t, event) = match topic {
            "link" => (
                Topic::Link,
                Event::Link {
                    connected: ev["connected"].as_bool().unwrap_or(false),
                },
            ),
            "context" => (
                Topic::Context,
                Event::Context {
                    key: ev["key"].as_str().unwrap_or("").into(),
                    value: ev["value"].as_str().unwrap_or("").into(),
                },
            ),
            _ => return,
        };
        let st = lock(&self.state);
        for c in st.conns.values().filter(|c| c.topics.contains(&t)) {
            c.send(&DaemonToPlugin::Event(event.clone()));
        }
    }

    /// Config reload: send each plugin its new settings.
    pub fn config_changed(&self, cfg: &CompiledConfig) {
        if let Some(shared) = self.shared() {
            self.revoke(&shared, LeaseRevokeReason::ConfigReload);
        }
        let st = lock(&self.state);
        for (name, c) in &st.conns {
            c.send(&DaemonToPlugin::Config {
                settings_json: settings_json(cfg, name),
            });
        }
    }
}
