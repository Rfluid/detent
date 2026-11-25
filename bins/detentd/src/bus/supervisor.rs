//! Plugin supervisor: start enabled plugins, restart crashed ones with exponential
//! backoff, ping every 2 s (3 missed → killed and restarted), stop them on shutdown.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use detent_bus::DaemonToPlugin;
use detent_config::{CompiledConfig, NativePlugin};
use tracing::{debug, info, warn};

use super::Bus;
use crate::shared::{Shared, lock};

const PING_EVERY: Duration = Duration::from_secs(2);
const MAX_MISSED_PINGS: u32 = 3;
const BACKOFF_MAX: Duration = Duration::from_secs(30);
/// A plugin that ran this long is considered healthy again (backoff resets).
const HEALTHY_AFTER: Duration = Duration::from_secs(60);

pub(super) struct Proc {
    pub(super) exe: PathBuf,
    pub(super) child: Option<Child>,
    pub(super) restarts: u32,
    pub(super) next_start: Instant,
    pub(super) started_at: Option<Instant>,
    pub(super) last_error: Option<String>,
}

/// Plugins the config wants running: name → executable.
fn wanted(cfg: &CompiledConfig) -> BTreeMap<String, PathBuf> {
    let me = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("detentd"));
    let mut out = BTreeMap::new();
    for (name, p) in &cfg.source.plugins {
        if !p.enable {
            continue;
        }
        let exe = match (&p.exec, NativePlugin::from_name(name)) {
            (Some(e), _) => PathBuf::from(expand_home(e)),
            (None, Some(n)) => n.resolve(&me),
            (None, None) => {
                warn!("plugin `{name}` has no `exec` and is not a native plugin; skipped");
                continue;
            }
        };
        out.insert(name.clone(), exe);
    }
    out
}

fn expand_home(p: &str) -> String {
    match (p.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(h)) => format!("{h}/{rest}"),
        _ => p.to_string(),
    }
}

impl Bus {
    pub fn supervise(self: Arc<Self>) {
        let mut last_ping = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let Some(shared) = self.shared() else { return };
            if shared.shutdown.load(Ordering::Relaxed) {
                return;
            }
            self.reconcile(&shared);
            if last_ping.elapsed() >= PING_EVERY {
                last_ping = Instant::now();
                self.ping();
            }
        }
    }

    /// Start/stop/restart processes to match the config.
    fn reconcile(&self, shared: &Shared) {
        let cfg = shared.config.load_full();
        let want = wanted(&cfg);
        let mut st = lock(&self.state);
        // stop removed/disabled plugins
        let gone: Vec<String> = st
            .procs
            .keys()
            .filter(|k| !want.contains_key(*k))
            .cloned()
            .collect();
        for name in gone {
            if let Some(c) = st.conns.get(&name) {
                c.send(&DaemonToPlugin::Shutdown);
            }
            if let Some(mut p) = st.procs.remove(&name)
                && let Some(mut ch) = p.child.take()
            {
                stop_child(&mut ch);
            }
            info!("plugin `{name}` stopped (disabled in config)");
        }
        for (name, exe) in want {
            let p = st.procs.entry(name.clone()).or_insert_with(|| Proc {
                exe: exe.clone(),
                child: None,
                restarts: 0,
                next_start: Instant::now(),
                started_at: None,
                last_error: None,
            });
            p.exe = exe;
            // reap
            if let Some(ch) = p.child.as_mut() {
                match ch.try_wait() {
                    Ok(Some(status)) => {
                        let ran = p.started_at.map_or(Duration::ZERO, |t| t.elapsed());
                        if ran >= HEALTHY_AFTER {
                            p.restarts = 0;
                        }
                        p.restarts += 1;
                        let backoff = Duration::from_secs(1 << p.restarts.min(5)).min(BACKOFF_MAX);
                        warn!("plugin `{name}` exited ({status}); restarting in {backoff:?}");
                        p.last_error = Some(status.to_string());
                        p.next_start = Instant::now() + backoff;
                        p.child = None;
                    }
                    Ok(None) => {}
                    Err(e) => debug!("try_wait {name}: {e}"),
                }
            }
            if p.child.is_none() && Instant::now() >= p.next_start {
                match Command::new(&p.exe)
                    .env("DETENT_BUS", detent_bus::socket_path())
                    .env("DETENT_PLUGIN", &name)
                    .stdin(Stdio::null())
                    .spawn()
                {
                    Ok(ch) => {
                        debug!("plugin `{name}` started (pid {})", ch.id());
                        p.child = Some(ch);
                        p.started_at = Some(Instant::now());
                    }
                    Err(e) => {
                        p.restarts += 1;
                        let backoff = Duration::from_secs(1 << p.restarts.min(5)).min(BACKOFF_MAX);
                        warn!(
                            "plugin `{name}`: cannot start {} ({e}); retry in {backoff:?}",
                            p.exe.display()
                        );
                        p.last_error = Some(e.to_string());
                        p.next_start = Instant::now() + backoff;
                    }
                }
            }
        }
    }

    fn ping(&self) {
        let nonce = self.nonce.fetch_add(1, Ordering::Relaxed);
        let mut st = lock(&self.state);
        let mut dead = Vec::new();
        for (name, c) in st.conns.iter_mut() {
            c.missed_pings += 1;
            if c.missed_pings > MAX_MISSED_PINGS {
                dead.push(name.clone());
            } else {
                c.send(&DaemonToPlugin::Ping { nonce });
            }
        }
        for name in dead {
            warn!("plugin `{name}` stopped answering pings; restarting it");
            st.conns.remove(&name);
            if let Some(ch) = st.procs.get_mut(&name).and_then(|p| p.child.as_mut()) {
                let _ = ch.kill();
            }
        }
    }

    pub fn shutdown(&self) {
        let mut st = lock(&self.state);
        for c in st.conns.values() {
            c.send(&DaemonToPlugin::Shutdown);
        }
        let deadline = Instant::now() + Duration::from_millis(800);
        for p in st.procs.values_mut() {
            if let Some(ch) = p.child.as_mut() {
                while Instant::now() < deadline {
                    if matches!(ch.try_wait(), Ok(Some(_))) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                stop_child(ch);
            }
        }
        let _ = std::fs::remove_file(detent_bus::socket_path());
    }
}

pub(super) fn stop_child(ch: &mut Child) {
    if matches!(ch.try_wait(), Ok(Some(_))) {
        return;
    }
    let pid = rustix::process::Pid::from_raw(ch.id() as i32);
    if let Some(pid) = pid {
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
    }
    for _ in 0..25 {
        if matches!(ch.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = ch.kill();
    let _ = ch.wait();
}
