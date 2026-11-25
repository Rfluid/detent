//! Hot reload: watch the config file's directory (editors replace files atomically, so
//! watching the file itself misses saves). A change is debounced, compiled, and only
//! installed if valid; otherwise the running config stays and the error is reported.

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use serde_json::json;
use tracing::{error, info, warn};

use crate::msg::Cmd;
use crate::shared::{Shared, lock};

const DEBOUNCE: Duration = Duration::from_millis(150);

/// Load the file now and install it if valid.
pub fn reload(shared: &Shared) -> Result<(), String> {
    match crate::app::load_config(&shared.config_path) {
        Ok(c) => {
            let c = Arc::new(c);
            shared.install_config(c.clone());
            if let Some(s) = lock(&shared.actions).clone() {
                s.config_changed(&c);
            }
            *lock(&shared.config_error) = None;
            info!("config reloaded");
            shared.publish("config", json!({"ok": true}));
            shared.broadcast(|| Cmd::Reload);
            shared.update_profile();
            Ok(())
        }
        Err(e) => {
            error!("config rejected, keeping the previous one: {e}");
            shared.publish("config", json!({"ok": false, "error": e}));
            *lock(&shared.config_error) = Some(e.clone());
            Err(e)
        }
    }
}

pub fn watch(shared: Arc<Shared>) {
    let path = shared.config_path.clone();
    let Some(dir) = path.parent().map(Path::to_path_buf) else {
        return;
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!("config dir {}: {e}; hot reload disabled", dir.display());
        return;
    }
    let (tx, rx) = channel();
    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            warn!("config watcher unavailable ({e}); use `detentctl reload`");
            return;
        }
    };
    if let Err(e) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
        warn!(
            "cannot watch {} ({e}); use `detentctl reload`",
            dir.display()
        );
        return;
    }
    let name = path.file_name().map(|n| n.to_os_string());
    loop {
        let ev: notify::Result<notify::Event> = match rx.recv() {
            Ok(e) => e,
            Err(_) => return,
        };
        let ours = ev.is_ok_and(|e| {
            !e.kind.is_access() && e.paths.iter().any(|p| p.file_name() == name.as_deref())
        });
        if !ours {
            continue;
        }
        // Debounce: editors write in several steps.
        loop {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(_) => continue,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        let _ = reload(&shared);
    }
}
