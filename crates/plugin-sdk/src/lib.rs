//! Detent plugin SDK. Guide: `docs/plugins/authoring.md`.
//!
//! ```ignore
//! struct Hello;
//! impl Plugin for Hello {
//!     fn manifest(&self) -> Manifest { manifest("hello", &[("wave", "Say hi", false)], &[]) }
//!     fn on_action(&mut self, ctx: &mut Context, a: &ActionCall) { ctx.haptic(1); }
//! }
//! fn main() -> Result<(), PluginError> { detent_plugin::run(Hello) }
//! ```
//!
//! The SDK owns the socket and a dispatch loop. Code on other threads sends with a
//! [`Sender`] (`Context::sender()` / `Connection::sender()`). UI plugins call [`connect`]
//! and drive [`Connection::recv`] from a background thread.
//!
//! Linux only for now, like the daemon that hosts the bus (`SOCK_SEQPACKET`; ADR 0010):
//! on other OSes this crate is empty.
#![cfg(target_os = "linux")]

use std::os::fd::OwnedFd;
use std::sync::Arc;

pub use detent_bus::{
    ActionDecl, ButtonEdge, Capability, Codec, DaemonToPlugin, DeviceCommand, Event,
    LeaseRevokeReason, LeaseTarget, Manifest, PROTOCOL_VERSION, PluginToDaemon, Topic, builtin,
};
use detent_bus::{MAX_PACKET, codec};
use rustix::net::{
    AddressFamily, RecvFlags, SendFlags, SocketAddrUnix, SocketType, connect as sock_connect, recv,
    send, socket,
};

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("cannot reach detentd bus socket {path}: {err}")]
    Connect { path: String, err: std::io::Error },
    #[error("bus I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol: {0}")]
    Protocol(String),
    #[error("the daemon closed the connection")]
    Closed,
}

/// An invocation of one of this plugin's actions.
#[derive(Debug, Clone)]
pub struct ActionCall {
    /// Local action name (without the `<plugin>.` prefix).
    pub action: String,
    pub params_json: String,
    pub trigger: String,
    /// Rotation delta for continuous actions.
    pub delta: i16,
    pub ts_us: u64,
}

/// Thread-safe, cloneable handle for sending to the daemon. Sends are non-blocking from
/// the daemon's point of view (it never waits on us), blocking only if our own socket
/// buffer is full.
#[derive(Clone, Debug)]
pub struct Sender {
    fd: Arc<OwnedFd>,
    codec: Codec,
}

impl Sender {
    pub fn send(&self, msg: &PluginToDaemon) -> Result<(), PluginError> {
        let bytes = codec::encode(self.codec, msg).map_err(|e| PluginError::Protocol(e.0))?;
        send(&*self.fd, &bytes, SendFlags::NOSIGNAL).map_err(std::io::Error::from)?;
        Ok(())
    }

    pub fn haptic(&self, waveform: u8) -> Result<(), PluginError> {
        self.send(&PluginToDaemon::Device(DeviceCommand::HapticPulse {
            waveform,
            intensity: 0,
        }))
    }

    pub fn invoke(&self, action: &str, params_json: &str) -> Result<(), PluginError> {
        self.send(&PluginToDaemon::Invoke {
            action: action.to_string(),
            params_json: params_json.to_string(),
        })
    }

    pub fn set_context(&self, key: &str, value: &str) -> Result<(), PluginError> {
        self.send(&PluginToDaemon::SetContext {
            key: key.to_string(),
            value: value.to_string(),
        })
    }
}

/// Outbound handle given to callbacks. Messages are sent when the callback returns.
#[derive(Debug)]
pub struct Context {
    outbox: Vec<PluginToDaemon>,
    sender: Option<Sender>,
}

impl Default for Context {
    fn default() -> Self {
        Self::detached()
    }
}

impl Context {
    /// A context not connected to a daemon (tests): inspect with [`Context::take_outbox`].
    pub fn detached() -> Self {
        Self {
            outbox: Vec::new(),
            sender: None,
        }
    }

    pub fn send(&mut self, msg: PluginToDaemon) {
        self.outbox.push(msg);
    }

    /// Play a haptic waveform (see `detentctl haptic` for names; 4 = subtle tick).
    pub fn haptic(&mut self, waveform: u8) {
        self.send(PluginToDaemon::Device(DeviceCommand::HapticPulse {
            waveform,
            intensity: 0,
        }));
    }

    pub fn invoke(&mut self, action: &str, params_json: &str) {
        self.send(PluginToDaemon::Invoke {
            action: action.to_string(),
            params_json: params_json.to_string(),
        });
    }

    pub fn set_context(&mut self, key: &str, value: &str) {
        self.send(PluginToDaemon::SetContext {
            key: key.to_string(),
            value: value.to_string(),
        });
    }

    pub fn lease(&mut self, id: u32, targets: Vec<LeaseTarget>, max_ms: u32) {
        self.send(PluginToDaemon::LeaseRequest {
            id,
            targets,
            max_ms,
        });
    }

    pub fn release(&mut self, id: u32) {
        self.send(PluginToDaemon::LeaseRelease { id });
    }

    pub fn subscribe(&mut self, topics: Vec<Topic>) {
        self.send(PluginToDaemon::Subscribe(topics));
    }

    /// A handle for sending from other threads (None in detached test contexts).
    pub fn sender(&self) -> Option<Sender> {
        self.sender.clone()
    }

    /// Drain queued messages (used by the dispatch loop and by tests).
    pub fn take_outbox(&mut self) -> Vec<PluginToDaemon> {
        std::mem::take(&mut self.outbox)
    }
}

pub trait Plugin {
    fn manifest(&self) -> Manifest;

    /// Called with the plugin's `[plugins.<name>]` settings on connect and every reload.
    fn on_config(&mut self, _ctx: &mut Context, _settings_json: &str) {}

    fn on_action(&mut self, ctx: &mut Context, call: &ActionCall);

    fn on_event(&mut self, _ctx: &mut Context, _event: &Event) {}

    /// Every other message (leases, state diffs, ...).
    fn on_message(&mut self, _ctx: &mut Context, _msg: &DaemonToPlugin) {}

    /// Called once after `Welcome`, before any other callback.
    fn on_start(&mut self, _ctx: &mut Context, _granted: &[Capability]) {}

    /// The daemon asked us to stop (or closed the bus). Clean up here.
    fn on_shutdown(&mut self) {}
}

/// Helper to build a manifest: `(name, doc, continuous)` triples.
pub fn manifest(name: &str, actions: &[(&str, &str, bool)], caps: &[Capability]) -> Manifest {
    Manifest {
        name: name.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        protocol: PROTOCOL_VERSION,
        codec: Codec::Postcard,
        actions: actions
            .iter()
            .map(|(n, d, c)| ActionDecl {
                name: n.to_string(),
                doc: d.to_string(),
                continuous: *c,
            })
            .collect(),
        caps: caps.to_vec(),
    }
}

/// Live connection to detentd.
#[derive(Debug)]
pub struct Connection {
    sender: Sender,
    granted: Vec<Capability>,
    buf: Vec<u8>,
}

/// Connect, send `Hello`, wait for `Welcome`.
pub fn connect(manifest: &Manifest) -> Result<Connection, PluginError> {
    let path = detent_bus::socket_path();
    let fd =
        socket(AddressFamily::UNIX, SocketType::SEQPACKET, None).map_err(std::io::Error::from)?;
    let addr = SocketAddrUnix::new(&path).map_err(std::io::Error::from)?;
    sock_connect(&fd, &addr).map_err(|e| PluginError::Connect {
        path: path.display().to_string(),
        err: e.into(),
    })?;
    let sender = Sender {
        fd: Arc::new(fd),
        codec: manifest.codec,
    };
    sender.send(&PluginToDaemon::Hello(manifest.clone()))?;
    let mut conn = Connection {
        sender,
        granted: Vec::new(),
        buf: vec![0u8; MAX_PACKET],
    };
    match conn.recv()? {
        DaemonToPlugin::Welcome { protocol, granted } => {
            if protocol != PROTOCOL_VERSION {
                return Err(PluginError::Protocol(format!(
                    "daemon speaks protocol {protocol}, we speak {PROTOCOL_VERSION}"
                )));
            }
            conn.granted = granted;
            Ok(conn)
        }
        other => Err(PluginError::Protocol(format!(
            "expected Welcome, got {other:?}"
        ))),
    }
}

impl Connection {
    /// Blocking receive of the next message.
    pub fn recv(&mut self) -> Result<DaemonToPlugin, PluginError> {
        let (n, _) = recv(&*self.sender.fd, &mut self.buf[..], RecvFlags::empty())
            .map_err(std::io::Error::from)?;
        if n == 0 {
            return Err(PluginError::Closed);
        }
        codec::decode(self.sender.codec, &self.buf[..n]).map_err(|e| PluginError::Protocol(e.0))
    }

    pub fn send(&self, msg: &PluginToDaemon) -> Result<(), PluginError> {
        self.sender.send(msg)
    }

    pub fn sender(&self) -> Sender {
        self.sender.clone()
    }

    pub fn granted(&self) -> &[Capability] {
        &self.granted
    }
}

fn flush(ctx: &mut Context, conn: &Connection) -> Result<(), PluginError> {
    for m in ctx.take_outbox() {
        conn.send(&m)?;
    }
    Ok(())
}

/// Connect and run the dispatch loop until the daemon sends `Shutdown` or goes away.
pub fn run<P: Plugin>(mut plugin: P) -> Result<(), PluginError> {
    let mut conn = connect(&plugin.manifest())?;
    let mut ctx = Context {
        outbox: Vec::new(),
        sender: Some(conn.sender()),
    };
    let granted = conn.granted().to_vec();
    plugin.on_start(&mut ctx, &granted);
    flush(&mut ctx, &conn)?;
    loop {
        let msg = match conn.recv() {
            Ok(m) => m,
            Err(PluginError::Closed) => {
                plugin.on_shutdown();
                return Ok(());
            }
            Err(e) => {
                plugin.on_shutdown();
                return Err(e);
            }
        };
        match &msg {
            DaemonToPlugin::Ping { nonce } => conn.send(&PluginToDaemon::Pong { nonce: *nonce })?,
            DaemonToPlugin::Shutdown => {
                plugin.on_shutdown();
                return Ok(());
            }
            DaemonToPlugin::Config { settings_json } => plugin.on_config(&mut ctx, settings_json),
            DaemonToPlugin::ActionInvoked {
                action,
                params_json,
                trigger,
                delta,
                ts_us,
            } => plugin.on_action(
                &mut ctx,
                &ActionCall {
                    action: action.clone(),
                    params_json: params_json.clone(),
                    trigger: trigger.clone(),
                    delta: *delta,
                    ts_us: *ts_us,
                },
            ),
            DaemonToPlugin::Event(e) => plugin.on_event(&mut ctx, e),
            other => plugin.on_message(&mut ctx, other),
        }
        flush(&mut ctx, &conn)?;
    }
}
