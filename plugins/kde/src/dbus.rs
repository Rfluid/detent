//! Session D-Bus side of detent-kde, on its own thread (callbacks from the bus loop must
//! return quickly). Continuous input is coalesced: deltas are summed and flushed at most
//! every 16 ms (≈ 60 Hz), so D-Bus never sees per-frame traffic.

use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use detent_plugin::Sender as BusSender;
use zbus::blocking::Connection;
use zbus::interface;

pub enum Job {
    Shortcut {
        component: String,
        name: String,
    },
    /// Continuous: `(component, up, down)` and a signed number of steps.
    Steps {
        target: (&'static str, &'static str, &'static str),
        steps: i32,
    },
    SetContextEnabled(bool),
    /// Ask KWin for the pointer position; answer by invoking `reply` (validated).
    Cursor {
        reply: String,
    },
    /// Show Plasma's volume OSD (after `kde.volume.set` with `osd`).
    VolumeOsd {
        percent: i32,
        max: i32,
    },
    Stop,
}

const FLUSH: Duration = Duration::from_millis(16);
/// Cap per flush so a fast spin can't queue seconds of volume changes.
const MAX_STEPS_PER_FLUSH: i32 = 6;
const SERVICE: &str = "dev.detent.Kde";
const SCRIPT_NAME: &str = "detent-context";

fn invoke(conn: &Connection, component: &str, name: &str) -> zbus::Result<()> {
    conn.call_method(
        Some("org.kde.kglobalaccel"),
        format!("/component/{component}").as_str(),
        Some("org.kde.kglobalaccel.Component"),
        "invokeShortcut",
        &(name,),
    )
    .map(|_| ())
}

/// D-Bus object the KWin script calls back with the active window / desktop.
struct ContextIface {
    bus: BusSender,
}

#[interface(name = "dev.detent.Kde")]
impl ContextIface {
    /// `_title` is received but not published: profiles match on the app, and window
    /// titles can be private.
    fn activated(&self, app: String, _title: String) {
        let _ = self.bus.set_context("app", &app.to_lowercase());
    }

    fn desktop(&self, name: String) {
        let _ = self.bus.set_context("desktop", &name);
    }

    /// From the one-shot cursor script: logical px relative to `output`, and that output's
    /// logical size. Strings, because KWin's `callDBus` may marshal JS numbers as int or
    /// double.
    fn cursor(
        &self,
        x: String,
        y: String,
        output: String,
        width: String,
        height: String,
        reply: String,
    ) {
        let (Ok(x), Ok(y)) = (x.parse::<f64>(), y.parse::<f64>()) else {
            eprintln!("detent-kde: bad cursor position from KWin: {x},{y}");
            return;
        };
        if !crate::actions::valid_reply(&reply) {
            return;
        }
        let mut params = serde_json::json!({ "x": x, "y": y, "output": output });
        // The output's size lets the overlay open at it instead of 0×0 (see radial.md).
        if let (Ok(w), Ok(h)) = (width.parse::<f64>(), height.parse::<f64>())
            && w > 0.
            && h > 0.
        {
            params["width"] = w.into();
            params["height"] = h.into();
        }
        let _ = self.bus.invoke(&reply, &params.to_string());
    }
}

const CURSOR_SCRIPT_NAME: &str = "detent-cursor";

/// One-shot script: report `workspace.cursorPos` relative to the output under it, and
/// that output's logical size. API verified on KWin 6.8 (nested `kwin_wayland --virtual`,
/// 2026-11-23).
fn cursor_script(reply: &str) -> String {
    format!(
        r#"(function () {{
    var p = workspace.cursorPos;
    var o = workspace.screenAt(p);
    var g = o ? o.geometry : {{ x: 0, y: 0, width: 0, height: 0 }};
    callDBus("dev.detent.Kde", "/Context", "dev.detent.Kde", "Cursor",
             String(p.x - g.x), String(p.y - g.y), o ? String(o.name) : "",
             String(g.width), String(g.height), "{reply}");
}})();
"#
    )
}

fn run_script(conn: &Connection, file: &str, name: &str, body: &str) -> Result<(), String> {
    let path = script_path().with_file_name(file);
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, body).map_err(|e| e.to_string())?;
    let _ = conn.call_method(
        Some("org.kde.KWin"),
        "/Scripting",
        Some("org.kde.kwin.Scripting"),
        "unloadScript",
        &(name,),
    );
    let reply = conn
        .call_method(
            Some("org.kde.KWin"),
            "/Scripting",
            Some("org.kde.kwin.Scripting"),
            "loadScript",
            &(path.display().to_string(), name),
        )
        .map_err(|e| format!("loadScript: {e}"))?;
    let id: i32 = reply
        .body()
        .deserialize()
        .map_err(|e| format!("loadScript reply: {e}"))?;
    if id < 0 {
        return Err(format!("KWin refused the script (id {id})"));
    }
    conn.call_method(
        Some("org.kde.KWin"),
        format!("/Scripting/Script{id}").as_str(),
        Some("org.kde.kwin.Script"),
        "run",
        &(),
    )
    .map_err(|e| format!("run script: {e}"))?;
    Ok(())
}

const SCRIPT: &str = r#"
function detentSend() {
    var w = workspace.activeWindow;
    // The radial menu's overlay takes the keyboard while open (KWin activates it): not an
    // app the user switched to, so don't change profiles for it.
    if (w && String(w.resourceClass) === "detent-radial") {
        return;
    }
    callDBus("dev.detent.Kde", "/Context", "dev.detent.Kde", "Activated",
             w ? String(w.resourceClass) : "", w ? String(w.caption) : "");
}
function detentDesktop() {
    var d = workspace.currentDesktop;
    callDBus("dev.detent.Kde", "/Context", "dev.detent.Kde", "Desktop", d ? String(d.name) : "");
}
workspace.windowActivated.connect(detentSend);
workspace.currentDesktopChanged.connect(detentDesktop);
detentSend();
detentDesktop();
"#;

fn script_path() -> std::path::PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("detent")
        .join("kde-context.js")
}

fn unload_script(conn: &Connection) {
    let _ = conn.call_method(
        Some("org.kde.KWin"),
        "/Scripting",
        Some("org.kde.kwin.Scripting"),
        "unloadScript",
        &(SCRIPT_NAME,),
    );
}

/// Load (or reload) the KWin script that reports the active window.
fn load_script(conn: &Connection) -> Result<(), String> {
    run_script(conn, "kde-context.js", SCRIPT_NAME, SCRIPT)
}

pub fn spawn(bus: BusSender, context: bool) -> Option<Sender<Job>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let ok = std::thread::Builder::new()
        .name("kde-dbus".into())
        .spawn(move || run(bus, rx, context))
        .is_ok();
    ok.then_some(tx)
}

#[expect(
    clippy::too_many_lines,
    reason = "predates the lint; split it when next changed (ADR 0008)"
)]
fn run(bus: BusSender, rx: Receiver<Job>, mut context: bool) {
    let conn = match zbus::blocking::connection::Builder::session()
        .and_then(|b| b.name(SERVICE))
        .and_then(|b| b.serve_at("/Context", ContextIface { bus: bus.clone() }))
        .and_then(|b| b.build())
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("detent-kde: session D-Bus unavailable ({e}); KDE actions disabled");
            // Keep draining so the plugin stays responsive.
            while let Ok(j) = rx.recv() {
                if matches!(j, Job::Stop) {
                    return;
                }
            }
            return;
        }
    };
    let kwin = conn
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "NameHasOwner",
            &("org.kde.KWin",),
        )
        .ok()
        .and_then(|r| r.body().deserialize::<bool>().ok())
        .unwrap_or(false);
    if !kwin {
        eprintln!(
            "detent-kde: KWin is not running; window/desktop actions and app context won't work"
        );
    }
    let mut script_loaded = false;
    if kwin && context {
        match load_script(&conn) {
            Ok(()) => script_loaded = true,
            Err(e) => eprintln!("detent-kde: app context unavailable: {e}"),
        }
    }

    let mut pending: BTreeMap<(&str, &str, &str), i32> = BTreeMap::new();
    let mut last_flush = Instant::now();
    loop {
        let timeout = if pending.is_empty() {
            Duration::from_secs(3600)
        } else {
            FLUSH.saturating_sub(last_flush.elapsed())
        };
        match rx.recv_timeout(timeout) {
            Ok(Job::Shortcut { component, name }) => {
                if let Err(e) = invoke(&conn, &component, &name) {
                    eprintln!("detent-kde: {component}/{name}: {e}");
                }
            }
            Ok(Job::Steps { target, steps }) => *pending.entry(target).or_insert(0) += steps,
            Ok(Job::SetContextEnabled(on)) if on != context => {
                context = on;
                if on && kwin {
                    script_loaded = load_script(&conn).is_ok();
                } else if script_loaded {
                    unload_script(&conn);
                    script_loaded = false;
                }
            }
            Ok(Job::SetContextEnabled(_)) => {}
            Ok(Job::Cursor { reply }) => {
                if !kwin {
                    eprintln!("detent-kde: kde.cursor needs KWin");
                } else if let Err(e) = run_script(
                    &conn,
                    "kde-cursor.js",
                    CURSOR_SCRIPT_NAME,
                    &cursor_script(&reply),
                ) {
                    eprintln!("detent-kde: cursor position unavailable: {e}");
                }
            }
            Ok(Job::VolumeOsd { percent, max }) => {
                // org.kde.osdService.volumeChanged(int percent, int maximumPercent), checked
                // with busctl introspect on Plasma 6.8 (2026-11-24).
                if let Err(e) = conn.call_method(
                    Some("org.kde.plasmashell"),
                    "/org/kde/osdService",
                    Some("org.kde.osdService"),
                    "volumeChanged",
                    &(percent, max),
                ) {
                    eprintln!("detent-kde: volume OSD: {e}");
                }
            }
            Ok(Job::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if !pending.is_empty() && last_flush.elapsed() >= FLUSH {
            for ((component, up, down), steps) in std::mem::take(&mut pending) {
                let name = if steps > 0 { up } else { down };
                for _ in 0..steps.abs().min(MAX_STEPS_PER_FLUSH) {
                    if let Err(e) = invoke(&conn, component, name) {
                        eprintln!("detent-kde: {component}/{name}: {e}");
                        break;
                    }
                }
            }
            last_flush = Instant::now();
        }
    }
    if script_loaded {
        unload_script(&conn);
    }
    let _ = conn.call_method(
        Some("org.kde.KWin"),
        "/Scripting",
        Some("org.kde.kwin.Scripting"),
        "unloadScript",
        &(CURSOR_SCRIPT_NAME,),
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn cursor_script_calls_back_with_the_reply() {
        let s = super::cursor_script("radial.cursor");
        assert!(s.contains(r#""dev.detent.Kde", "/Context", "dev.detent.Kde", "Cursor""#));
        assert!(s.contains("workspace.screenAt(p)"));
        assert!(s.trim_end().ends_with("\"radial.cursor\");\n})();"));
        assert!(super::SCRIPT.contains(r#"=== "detent-radial""#));
    }
}
