//! The plugin proper (Linux: GPUI layer-shell overlay + plugin bus): manifest, bus thread,
//! the `Ui` driver and `main`. `main.rs` declares the modules and picks this or the
//! unsupported-OS exit (ADR 0010).

use std::process::ExitCode;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use detent_plugin::{
    ButtonEdge, Capability, DaemonToPlugin, Event, LeaseRevokeReason, LeaseTarget, Manifest,
    PluginToDaemon, Sender, Topic,
};
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use gpui::{App, QuitMode, WindowHandle};

use crate::menu::{Settings, Step};
use crate::state::{Effect, Input, Radial};
use crate::view::MenuView;
use crate::{preview, replies, state, view};

/// How long to wait for `kde.cursor` before showing without it (the overlay then centres
/// on the compositor's pointer `enter` / first motion). The KWin round trip measures well
/// under this; the window itself takes ~140 ms to appear.
const LOCATE_TIMEOUT: Duration = Duration::from_millis(200);
/// Before running `keys` after closing: KWin gives the keyboard back to the previous
/// window when the overlay's surface goes away.
const FOCUS_RETURN: Duration = Duration::from_millis(60);

fn manifest() -> Manifest {
    detent_plugin::manifest(
        "radial",
        &[
            (
                "open",
                "Open a radial menu at the cursor: params { menu }",
                false,
            ),
            (
                "cursor",
                "Pointer position reply (from kde.cursor): params { x, y, output }",
                false,
            ),
            (
                "level",
                "Adjustable-item reply (from <source>.get/.set/.mute): params { source, seq, \
                 percent, muted, max, label } or { source, seq, error }",
                false,
            ),
            (
                "targets",
                "Adjustable-item targets reply (from <source>.targets): params { source, seq, \
                 targets: [{ id, name, icon, percent, muted, current }] } or { source, seq, \
                 error }",
                false,
            ),
        ],
        &[
            // the trigger control, so its release reaches us (hold-and-release)
            Capability::LeaseButtons,
            Capability::DeviceHaptics,
            Capability::ActionsInvoke,
            Capability::InputEmit,
            // `app` context for per-application rings
            Capability::EventsSubscribe,
        ],
    )
}

pub enum Msg {
    Daemon(DaemonToPlugin),
    /// `[plugins.radial]`, parsed (and icons resolved) on the bus thread.
    Settings(Box<Settings>),
    /// From the overlay or a timer.
    Input(Input),
    /// A step of a selected item, due now: `(action, params JSON)`.
    Invoke(String, String),
    Closed,
}

/// Bus thread: blocking receive loop → GPUI channel.
fn bus_thread(mut conn: detent_plugin::Connection, tx: UnboundedSender<Msg>) {
    loop {
        match conn.recv() {
            Ok(DaemonToPlugin::Ping { nonce }) => {
                let _ = conn.send(&PluginToDaemon::Pong { nonce });
            }
            Ok(DaemonToPlugin::Config { settings_json }) => match Settings::parse(&settings_json) {
                Ok(s) => {
                    if tx.unbounded_send(Msg::Settings(Box::new(s))).is_err() {
                        return;
                    }
                }
                Err(e) => eprintln!("detent-radial: bad [plugins.radial] settings: {e}"),
            },
            // Target lists name icons: resolve them here, off the UI thread.
            Ok(DaemonToPlugin::ActionInvoked {
                action,
                params_json,
                ..
            }) if action == "targets" => {
                let params = serde_json::from_str(&params_json).unwrap_or_default();
                if let Some(i) = replies::targets_input(&params)
                    && tx.unbounded_send(Msg::Input(i)).is_err()
                {
                    return;
                }
            }
            Ok(m) => {
                if tx.unbounded_send(Msg::Daemon(m)).is_err() {
                    return;
                }
            }
            Err(e) => {
                eprintln!("detent-radial: bus closed: {e}");
                let _ = tx.unbounded_send(Msg::Closed);
                return;
            }
        }
    }
}

/// What the effects talk to: the daemon, or nothing (`--preview`).
pub trait Backend {
    fn lease(&mut self, control: &str);
    fn release(&mut self);
    /// Ask where the pointer is (reply arrives as `Input::Cursor`).
    fn locate(&mut self, tx: &UnboundedSender<Msg>);
    fn haptic(&mut self, waveform: u8);
    fn invoke(&mut self, action: &str, params_json: &str);
    /// Called after a selection ran or the ring closed (preview quits).
    fn closed(&mut self, _cx: &mut App) {}
    /// Is this lease message about our current lease? (Stale grants/revokes are ignored.)
    fn is_current_lease(&self, _id: u32) -> bool {
        true
    }
}

struct Bus {
    bus: Sender,
    lease_id: u32,
}

static NEXT_LEASE: AtomicU32 = AtomicU32::new(1);
/// Set when the overlay can't be opened any more; the process then exits non-zero.
static FAILED: AtomicBool = AtomicBool::new(false);

fn control_name(s: &str) -> LeaseTarget {
    let mut b = [0u8; 16];
    let n = s.len().min(16);
    b[..n].copy_from_slice(&s.as_bytes()[..n]);
    LeaseTarget::Control(detent_bus::ControlName(b))
}

impl Backend for Bus {
    fn lease(&mut self, control: &str) {
        self.lease_id = NEXT_LEASE.fetch_add(1, Ordering::Relaxed);
        let _ = self.bus.send(&PluginToDaemon::LeaseRequest {
            id: self.lease_id,
            targets: vec![control_name(control)],
            max_ms: 10_000,
        });
    }
    fn release(&mut self) {
        let _ = self
            .bus
            .send(&PluginToDaemon::LeaseRelease { id: self.lease_id });
    }
    fn locate(&mut self, _tx: &UnboundedSender<Msg>) {
        // detent-kde asks KWin and answers with `radial.cursor`. Without it the timeout
        // fires and the overlay uses the compositor's pointer position instead.
        let _ = self
            .bus
            .invoke("kde.cursor", r#"{"reply":"radial.cursor"}"#);
    }
    fn haptic(&mut self, waveform: u8) {
        let _ = self.bus.haptic(waveform);
    }
    fn invoke(&mut self, action: &str, params_json: &str) {
        let _ = self.bus.invoke(action, params_json);
    }
    fn is_current_lease(&self, id: u32) -> bool {
        id == self.lease_id
    }
}

pub struct Ui<B: Backend> {
    pub radial: Radial,
    pub backend: B,
    window: Option<WindowHandle<MenuView>>,
    /// The previous overlay, collapsing into the centre (takes no input); removed after
    /// `view::close_total_ms`, or at once when a new one opens.
    closing: Option<WindowHandle<MenuView>>,
    /// How the next `Hide` closes (decided from the whole batch of effects it came in).
    next_close: view::Close,
    tx: UnboundedSender<Msg>,
    /// Preview only: forwarded to the view.
    pub backdrop: Option<u32>,
}

impl<B: Backend + 'static> Ui<B> {
    pub fn new(radial: Radial, backend: B, tx: UnboundedSender<Msg>) -> Self {
        Self {
            radial,
            backend,
            window: None,
            closing: None,
            next_close: view::Close::Collapse,
            tx,
            backdrop: None,
        }
    }

    pub fn input(&mut self, input: Input, cx: &mut App) {
        let effects = self.radial.handle(input);
        self.next_close = view::Close::for_effects(&effects);
        for e in effects {
            self.apply(e, cx);
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "predates the lint; split it when next changed (ADR 0008)"
    )]
    fn apply(&mut self, e: Effect, cx: &mut App) {
        match e {
            Effect::RequestLease { control } => self.backend.lease(&control),
            Effect::ReleaseLease => self.backend.release(),
            Effect::LocateCursor { seq } => {
                self.backend.locate(&self.tx);
                let tx = self.tx.clone();
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(LOCATE_TIMEOUT).await;
                    let _ = tx.unbounded_send(Msg::Input(Input::LocateTimeout(seq)));
                })
                .detach();
            }
            Effect::Show { ring, at } => {
                self.hide(cx);
                let t = std::time::Instant::now();
                let tx = self.tx.clone();
                let emit: view::Emit = Rc::new(move |i| {
                    let _ = tx.unbounded_send(Msg::Input(i));
                });
                let located = at.is_some();
                match view::open(cx, ring, at, &self.radial.settings, emit) {
                    Ok(w) => {
                        if std::env::var_os("DETENT_RADIAL_DEBUG").is_some() {
                            eprintln!(
                                "detent-radial: window opened in {:?} (cursor from KWin: {located})",
                                t.elapsed()
                            );
                        }
                        if let Some(b) = self.backdrop {
                            let _ = w.update(cx, |v, _, _| v.backdrop = Some(b));
                        }
                        self.window = Some(w)
                    }
                    Err(err) => {
                        eprintln!("detent-radial: cannot show the menu: {err}");
                        // Give the input back right away.
                        self.input(Input::Revoked { close: true }, cx);
                        self.backend.release();
                        // Anything but a missing layer-shell means this process can't open
                        // windows any more: KWin drops a client's connection on a protocol
                        // error and GPUI keeps running without it (every later surface then
                        // has no formats: "Adapter … is not compatible with the display
                        // surface"). Exit; the daemon restarts us with a fresh connection.
                        if !err.is::<gpui::layer_shell::LayerShellNotSupportedError>() {
                            eprintln!(
                                "detent-radial: exiting so the daemon restarts it with a new \
                                 compositor connection"
                            );
                            FAILED.store(true, Ordering::Relaxed);
                            self.hide(cx);
                            cx.quit();
                        }
                    }
                }
            }
            Effect::Highlight(h) => {
                if let Some(w) = self.window {
                    let _ = w.update(cx, |v, _, cx| {
                        v.set_highlight(h);
                        cx.notify();
                    });
                }
            }
            Effect::Navigate(ring) => {
                if let Some(w) = self.window {
                    let _ = w.update(cx, |v, _, cx| {
                        v.navigate(ring);
                        cx.notify();
                    });
                }
            }
            Effect::Hide => {
                self.close(self.next_close, cx);
                self.backend.closed(cx);
            }
            Effect::Haptic(w) => self.backend.haptic(w),
            Effect::Gauge(g) => {
                if let Some(w) = self.window {
                    let _ = w.update(cx, |v, _, cx| {
                        v.set_gauge(g);
                        cx.notify();
                    });
                }
            }
            Effect::After { ms, input } => {
                let tx = self.tx.clone();
                cx.spawn(async move |cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(u64::from(ms)))
                        .await;
                    let _ = tx.unbounded_send(Msg::Input(*input));
                })
                .detach();
            }
            Effect::Run(steps) => {
                let tx = self.tx.clone();
                let wait_focus = matches!(steps.first(), Some(Step::Invoke(a, _)) if a == "keys");
                cx.spawn(async move |cx| {
                    if wait_focus {
                        cx.background_executor().timer(FOCUS_RETURN).await;
                    }
                    for step in steps {
                        match step {
                            // Invokes go through the backend, on the UI thread.
                            Step::Invoke(action, params) => {
                                let _ = tx.unbounded_send(Msg::Invoke(action, params));
                            }
                            Step::DelayMs(ms) => {
                                cx.background_executor()
                                    .timer(Duration::from_millis(u64::from(ms)))
                                    .await;
                            }
                        }
                    }
                })
                .detach();
            }
        }
    }

    /// Remove the overlay (and one still collapsing) at once.
    pub fn hide(&mut self, cx: &mut App) {
        for w in [self.window.take(), self.closing.take()]
            .into_iter()
            .flatten()
        {
            let _ = w.update(cx, |_, window, _| window.remove_window());
        }
    }

    /// Close the overlay: collapse it into the centre, then remove it. The pointer goes
    /// through it at once (empty input region) and it ignores keys meanwhile. `Close::Now`
    /// (a `keys` step: the keyboard must come back first) and `animation_ms = 0` remove
    /// it right away.
    fn close(&mut self, how: view::Close, cx: &mut App) {
        let ms = view::close_total_ms(self.radial.settings.animation_ms);
        if how == view::Close::Now || ms == 0 {
            self.hide(cx);
            return;
        }
        let Some(w) = self.window.take() else { return };
        if let Some(old) = self.closing.replace(w) {
            let _ = old.update(cx, |_, window, _| window.remove_window());
        }
        let _ = w.update(cx, |v, window, cx| {
            v.start_close(how == view::Close::Select);
            window.set_input_region(Some(&[]));
            cx.notify();
        });
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(u64::from(ms)))
                .await;
            // Already gone if a new overlay replaced it (`hide`): then this fails, harmlessly.
            let _ = w.update(cx, |_, window, _| window.remove_window());
        })
        .detach();
    }

    /// One message from the channel. Returns `false` to stop.
    pub fn on_msg(&mut self, msg: Msg, cx: &mut App) -> bool {
        match msg {
            Msg::Daemon(m) => return self.on_daemon(m, cx),
            Msg::Settings(s) => self.radial.settings = *s,
            Msg::Input(i) => self.input(i, cx),
            Msg::Invoke(action, params) => self.backend.invoke(&action, &params),
            Msg::Closed => {
                self.hide(cx);
                cx.quit();
                return false;
            }
        }
        true
    }

    fn on_daemon(&mut self, m: DaemonToPlugin, cx: &mut App) -> bool {
        match m {
            // Our own overlay (focused while open) is never "the app".
            DaemonToPlugin::Event(Event::Context { key, value })
                if key == "app" && value != "detent-radial" =>
            {
                self.input(Input::App(value), cx)
            }
            DaemonToPlugin::ActionInvoked {
                action,
                params_json,
                trigger,
                ..
            } => {
                let params: serde_json::Value =
                    serde_json::from_str(&params_json).unwrap_or_default();
                match action.as_str() {
                    "open" => {
                        let menu = params
                            .get("menu")
                            .and_then(|m| m.as_str())
                            .unwrap_or("main")
                            .to_string();
                        let control = state::trigger_control(&trigger);
                        self.input(Input::Open { menu, control }, cx);
                    }
                    "cursor" => {
                        if let Some(at) = replies::cursor_at(&params) {
                            self.input(Input::Cursor(at), cx);
                        }
                    }
                    "level" => {
                        if let Some(i) = replies::level_input(&params) {
                            self.input(i, cx);
                        }
                    }
                    _ => {}
                }
            }
            DaemonToPlugin::LeaseGranted { id } if self.backend.is_current_lease(id) => {
                self.input(Input::LeaseGranted, cx)
            }
            DaemonToPlugin::LeaseDenied { id, reason } if self.backend.is_current_lease(id) => {
                eprintln!("detent-radial: lease denied: {reason}");
                self.input(Input::LeaseDenied, cx);
            }
            DaemonToPlugin::LeaseRevoked { id, reason } if self.backend.is_current_lease(id) => {
                self.input(
                    Input::Revoked {
                        close: !matches!(reason, LeaseRevokeReason::Expired),
                    },
                    cx,
                )
            }
            DaemonToPlugin::Event(Event::Button { control, edge, .. }) => self.input(
                Input::Button {
                    control,
                    press: edge == ButtonEdge::Press,
                },
                cx,
            ),
            DaemonToPlugin::Shutdown => {
                self.hide(cx);
                cx.quit();
                return false;
            }
            _ => {}
        }
        true
    }
}

/// Drive a `Ui` from the channel on the GPUI executor.
pub fn drive<B: Backend + 'static>(mut ui: Ui<B>, mut rx: UnboundedReceiver<Msg>, cx: &mut App) {
    cx.spawn(async move |cx| {
        while let Some(msg) = rx.next().await {
            if !cx.update(|cx| ui.on_msg(msg, cx)) {
                break;
            }
        }
    })
    .detach();
}

fn run_ui(conn: detent_plugin::Connection) {
    let bus = conn.sender();
    // Per-app rings follow the `app` context (published by detent-kde). Without the
    // capability the daemon refuses this and the top-level rings are used everywhere.
    let _ = bus.send(&PluginToDaemon::Subscribe(vec![Topic::Context]));
    let (tx, rx): (UnboundedSender<Msg>, UnboundedReceiver<Msg>) = unbounded();
    let bus_tx = tx.clone();
    if std::thread::Builder::new()
        .name("radial-bus".into())
        .spawn(move || bus_thread(conn, bus_tx))
        .is_err()
    {
        eprintln!("detent-radial: cannot start the bus thread");
        return;
    }
    gpui_platform::application()
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx: &mut App| {
            let ui = Ui::new(
                Radial::new(Settings::default()),
                Bus { bus, lease_id: 0 },
                tx,
            );
            view::warm_up(cx);
            drive(ui, rx, cx);
        });
}

pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match preview::Opts::from_args(&args) {
        Ok(Some(o)) => return preview::run(o),
        Ok(None) => {}
        Err(e) => {
            eprintln!("detent-radial: {e}");
            return ExitCode::FAILURE;
        }
    }
    let conn = match detent_plugin::connect(&manifest()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("detent-radial: {e}");
            return ExitCode::FAILURE;
        }
    };
    run_ui(conn);
    if FAILED.load(Ordering::Relaxed) {
        // Don't wait for GPUI's teardown on a dead compositor connection.
        std::process::exit(1);
    }
    ExitCode::SUCCESS
}
