//! The daemon proper (Linux): startup, device scan + udev hotplug, signals and orderly
//! shutdown. `main.rs` only declares the modules and picks this or the unsupported-OS exit.

use std::os::fd::AsFd;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use anyhow::Result;
use detent_config::CompiledConfig;
use detent_config::schema::ConflictPolicy;
use detent_hidpp_transport::discover::{self, DeviceInfo, LOGITECH_VENDOR};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use signal_hook::consts::{SIGINT, SIGTERM};
use tracing::{debug, error, info, warn};

use crate::device::Target;
use crate::msg::Cmd;
use crate::shared::{Shared, lock};
use crate::{bus, config_watch, conflicts, device, dispatch, ipc, receivers};

fn init_logging() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_env("DETENT_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("info"));
    let journald = std::env::var_os("JOURNAL_STREAM").is_some();
    let b = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false);
    if journald {
        // journald adds its own timestamps
        b.without_time().with_ansi(false).init();
    } else {
        b.init();
    }
}

/// Load the config file. Missing file → empty config (no bindings). Invalid file →
/// error is logged and returned; the caller keeps whatever is running.
pub fn load_config(path: &std::path::Path) -> Result<CompiledConfig, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => detent_config::load(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            detent_config::load("").map_err(|e| e.to_string())
        }
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn parse_args() -> PathBuf {
    let mut args = std::env::args().skip(1);
    let mut path = detent_config::default_path();
    while let Some(a) = args.next() {
        match a.as_str() {
            "-c" | "--config" => {
                if let Some(p) = args.next() {
                    path = PathBuf::from(p);
                }
            }
            "-V" | "--version" => {
                println!("detentd {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "-h" | "--help" => {
                println!(
                    "detentd [--config PATH]\n\nRuns the Detent daemon in the foreground.\n\
                     Logs: DETENT_LOG=debug for more detail.\nConfig: {}",
                    path.display()
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument `{other}` (try --help)");
                std::process::exit(2);
            }
        }
    }
    path
}

fn is_candidate(info: &DeviceInfo) -> bool {
    info.vendor == LOGITECH_VENDOR
}

fn try_attach(target: &Target, shared: &Arc<Shared>) {
    let info = &target.info;
    // udev may still be applying permissions right after `add`: retry briefly.
    for attempt in 0..5 {
        match device::attach(target, shared) {
            Ok(()) => return,
            Err(e) => {
                let denied = e
                    .chain()
                    .any(|c| c.to_string().contains("Permission denied"));
                if attempt == 4 || !denied {
                    match target.slot() {
                        Some(s) => debug!("{} slot {s}: not attached: {e:#}", info.node.display()),
                        None => debug!("{}: not attached: {e:#}", info.node.display()),
                    }
                    if denied {
                        warn!(
                            "{}: permission denied. Install the udev rules: see \
                             docs/user/quickstart.md",
                            info.node.display()
                        );
                    }
                    return;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

/// Attach what's new. Returns receivers with paired devices that didn't attach (off), for
/// [`receivers::Watch`].
fn scan(shared: &Arc<Shared>) -> Vec<(PathBuf, Vec<u8>)> {
    let devs = match discover::logitech() {
        Ok(devs) => devs,
        Err(e) => {
            warn!("device scan failed: {e}");
            return Vec::new();
        }
    };
    let direct: Vec<String> = lock(&shared.devices)
        .values()
        .filter(|d| d.slot.is_none())
        .map(|d| d.node.clone())
        .collect();
    let mut pending = Vec::new();
    for d in devs.iter().filter(|d| is_candidate(d)) {
        if d.receiver {
            let left = receivers::attach_slots(d, shared, try_attach);
            if !left.is_empty() {
                pending.push((d.node.clone(), left));
            }
        } else if !direct.contains(&d.node.display().to_string()) {
            try_attach(&Target::direct(d.clone()), shared);
        }
    }
    pending
}

/// udev monitor on `hidraw`; rescans on any add, and when a paired device that is off
/// wakes up on a watched receiver. Falls back to polling every 5 s if the monitor can't
/// be created.
fn hotplug(shared: Arc<Shared>) {
    let mut watch = receivers::Watch::default();
    watch.update(scan(&shared));
    let monitor = udev::MonitorBuilder::new()
        .and_then(|b| b.match_subsystem("hidraw"))
        .and_then(|b| b.listen());
    let socket = match monitor {
        Ok(s) => Some(s),
        Err(e) => {
            warn!("udev monitor unavailable ({e}); polling for devices every 5 s");
            None
        }
    };
    let tick = Timespec {
        tv_sec: 5,
        tv_nsec: 0,
    };
    let mut last_scan = Instant::now();
    while !shared.shutdown.load(Ordering::Relaxed) {
        let mut added = false;
        {
            let watched: Vec<_> = watch.fds().collect();
            let sock_fd = socket.as_ref().map(|s| s.as_fd());
            let mut fds: Vec<PollFd<'_>> = sock_fd
                .iter()
                .chain(watched.iter())
                .map(|fd| PollFd::new(fd, PollFlags::IN))
                .collect();
            if fds.is_empty() {
                std::thread::sleep(Duration::from_secs(5));
            } else {
                let _ = poll(&mut fds, Some(&tick));
            }
        }
        if let Some(sock) = socket.as_ref() {
            for ev in sock.iter() {
                if ev.event_type() == udev::EventType::Add {
                    debug!("hotplug add: {:?}", ev.devnode());
                    added = true;
                }
            }
        } else {
            added |= last_scan.elapsed() > Duration::from_secs(5);
        }
        let woke = watch.drain();
        if added || woke || last_scan.elapsed() > Duration::from_secs(60) {
            // A just-added node needs a moment for the kernel driver's own setup; a
            // device that just woke up is still reconnecting.
            std::thread::sleep(Duration::from_millis(300));
            watch.update(scan(&shared));
            last_scan = Instant::now();
        }
    }
}

pub fn main() {
    init_logging();
    let path = parse_args();
    info!(
        "detentd {} starting; config {}",
        env!("CARGO_PKG_VERSION"),
        path.display()
    );

    let (config, cfg_err) = match load_config(&path) {
        Ok(c) => (c, None),
        Err(e) => {
            error!("config rejected, running without bindings: {e}");
            let empty = detent_config::load("").unwrap_or_else(|_| unreachable_empty());
            (empty, Some(e))
        }
    };
    let policy = config.source.daemon.conflict_policy;
    let found = conflicts::running_conflicts();
    if !found.is_empty() {
        match policy {
            ConflictPolicy::Refuse => {
                error!(
                    "{} is running; refusing to start (daemon.conflict_policy = \"refuse\")",
                    found.join(", ")
                );
                std::process::exit(1);
            }
            ConflictPolicy::Warn => warn!(
                "{} is running and may fight Detent over the mouse (diversion, settings). \
                 Stop it for reliable behaviour.",
                found.join(", ")
            ),
            ConflictPolicy::Ignore => {}
        }
    }
    if policy != ConflictPolicy::Ignore {
        conflicts::watch(found);
    }

    let (dispatch_tx, dispatch_rx) = std::sync::mpsc::sync_channel(256);
    let shared = Arc::new(Shared::new(config, path, dispatch_tx));
    *lock(&shared.config_error) = cfg_err;

    let bus = bus::Bus::new(&shared);
    *lock(&shared.actions) = Some(bus.clone());
    {
        let b = bus.clone();
        let s = bus.clone();
        let d = shared.clone();
        let spawned = std::thread::Builder::new()
            .name("detent-bus".into())
            .spawn(move || b.serve())
            .and_then(|_| {
                std::thread::Builder::new()
                    .name("detent-supervisor".into())
                    .spawn(move || s.supervise())
            })
            .and_then(|_| {
                std::thread::Builder::new()
                    .name("detent-dispatch".into())
                    .spawn(move || dispatch::run(d, dispatch_rx))
            });
        if let Err(e) = spawned {
            error!("cannot start plugin bus threads: {e}");
            std::process::exit(1);
        }
    }

    let mut signals = match signal_hook::iterator::Signals::new([SIGTERM, SIGINT]) {
        Ok(s) => s,
        Err(e) => {
            error!("cannot install signal handlers: {e}");
            std::process::exit(1);
        }
    };

    type Service = fn(Arc<Shared>);
    let threads: [(&str, Service); 3] = [
        ("detent-hotplug", hotplug),
        ("detent-ipc", ipc::serve),
        ("detent-config", config_watch::watch),
    ];
    for (name, f) in threads {
        let s = shared.clone();
        if let Err(e) = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || f(s))
        {
            error!("cannot start {name} thread: {e}");
            std::process::exit(1);
        }
    }

    if let Some(sig) = signals.forever().next() {
        info!("signal {sig}: shutting down");
    }
    shutdown(&shared);
}

fn shutdown(shared: &Arc<Shared>) {
    shared.shutdown.store(true, Ordering::Relaxed);
    if let Some(s) = lock(&shared.actions).clone() {
        s.shutdown();
    }
    shared.broadcast(|| Cmd::Shutdown);
    // Workers undivert, the data planes release keys and the grab, then detach.
    let deadline = Instant::now() + Duration::from_secs(3);
    while !lock(&shared.devices).is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let left = lock(&shared.devices).len();
    if left > 0 {
        warn!("{left} device(s) did not detach in time");
    }
    let _ = std::fs::remove_file(ipc::socket_path());
    info!("stopped");
}

/// The empty config always compiles; this only exists to avoid `unwrap`.
fn unreachable_empty() -> CompiledConfig {
    error!("internal: empty config failed to compile");
    std::process::exit(1)
}
