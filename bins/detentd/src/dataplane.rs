//! The per-device data-plane thread (docs/architecture/data-plane.md). Sole owner of the
//! hidraw fd, the mouse's evdev fd and the uinput devices. Sleeps only in `poll`.
//!
//! Per event: read → classify → engine → uinput write and/or a `try_send` to the worker.
//! No allocation, no locks (config is one `ArcSwap::load`), never blocks on the control
//! plane: if the worker's queue is full, the message is dropped and counted.

use std::os::fd::{AsFd, OwnedFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::SyncSender;
use std::time::Instant;

use arc_swap::ArcSwap;
use detent_config::CompiledConfig;
use detent_engine::out::code;
use detent_engine::{ControlOut, Engine, GrabChange, Out};
use detent_hidpp_proto::{DeviceIndex, Frame, Kind, SoftwareId, classify};
use detent_hidpp_transport::Transport;
use detent_hidpp_transport::hidraw::HidrawTransport;
use evdev::raw_stream::RawDevice;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use tracing::{debug, warn};

use crate::msg::{FromDp, ToDp, WorkerMsg};
use crate::uinput::Uinput;

/// Feature indexes the data plane decodes itself (0 = absent).
#[derive(Debug, Clone, Copy, Default)]
pub struct Indexes {
    pub reprog: u8,
}

#[derive(Debug, Default)]
pub struct Stats {
    pub frames: AtomicU64,
    pub dropped_to_worker: AtomicU64,
    pub dropped_out: AtomicU64,
}

pub struct DataPlane {
    pub hidraw: HidrawTransport,
    /// 0xFF for a direct device; 1–6 for a receiver slot (the node carries every slot's
    /// traffic, so frames for other slots are skipped).
    pub device: DeviceIndex,
    pub evdev: Option<RawDevice>,
    pub uinput: Uinput,
    pub wake: OwnedFd,
    pub rx: rtrb::Consumer<ToDp>,
    pub tx: SyncSender<WorkerMsg>,
    pub dispatch: SyncSender<ControlOut>,
    pub config: Arc<ArcSwap<CompiledConfig>>,
    pub idx: Indexes,
    pub stats: Arc<Stats>,
}

enum Flow {
    Continue,
    Stop,
}

impl DataPlane {
    fn send(&self, m: FromDp) {
        if self.tx.try_send(WorkerMsg::Dp(m)).is_err() {
            self.stats.dropped_to_worker.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn flush(&mut self, out: &mut Out) {
        if !out.events().is_empty()
            && let Err(e) = self.uinput.write(out.events())
        {
            debug!("uinput write failed: {e}");
        }
        for c in out.controls() {
            match c {
                // Device commands need this device's worker (HID++ round trips).
                ControlOut::Device(_) => self.send(FromDp::Control(c)),
                // Everything else goes to the dispatch thread so it never waits behind one.
                _ => {
                    if self.dispatch.try_send(c).is_err() {
                        self.stats.dropped_to_worker.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
        if out.dropped > 0 {
            self.stats
                .dropped_out
                .fetch_add(u64::from(out.dropped), Ordering::Relaxed);
            out.dropped = 0;
        }
        out.clear();
    }

    fn apply_grab(&mut self, engine: &mut Engine, cfg: &CompiledConfig, out: &mut Out) {
        let Some(change) = engine.grab_change(&cfg.engine) else {
            return;
        };
        let Some(dev) = self.evdev.as_mut() else {
            return;
        };
        match change {
            GrabChange::Grab => match dev.grab() {
                Ok(()) => {
                    // Events queued before the grab were also delivered to the desktop:
                    // consume them in tracking mode only.
                    if let Ok(evs) = dev.fetch_events() {
                        for ev in evs {
                            engine.on_evdev(
                                &cfg.engine,
                                ev.event_type().0,
                                ev.code(),
                                ev.value(),
                                Instant::now(),
                                out,
                            );
                        }
                    }
                    engine.set_grabbed(true);
                    self.send(FromDp::Grabbed(true));
                }
                Err(e) => debug!("EVIOCGRAB failed: {e}"),
            },
            GrabChange::Ungrab => {
                if let Err(e) = dev.ungrab() {
                    debug!("ungrab failed: {e}");
                }
                engine.set_grabbed(false);
                self.send(FromDp::Grabbed(false));
            }
        }
    }

    fn on_frame(&mut self, f: Frame, engine: &mut Engine, cfg: &CompiledConfig, out: &mut Out) {
        self.stats.frames.fetch_add(1, Ordering::Relaxed);
        let d = f.device_index();
        // Direct devices sometimes answer at index 0x00 (docs/specs/hidpp.md).
        if d != self.device && !(self.device == DeviceIndex::DIRECT && d.0 == 0) {
            return;
        }
        match classify(&f, SoftwareId::DETENT) {
            Kind::Response | Kind::Error { .. } => self.send(FromDp::Response(f)),
            Kind::Notification => {
                if self.idx.reprog != 0 && f.feature_index() == self.idx.reprog {
                    let p = f.params();
                    match f.function() {
                        0 if p.len() >= 8 => {
                            let cid = |i: usize| u16::from_be_bytes([p[i], p[i + 1]]);
                            let cids = [cid(0), cid(2), cid(4), cid(6)];
                            engine.on_diverted_buttons(&cfg.engine, cids, Instant::now(), out);
                        }
                        1 if p.len() >= 4 => {
                            let dx = i16::from_be_bytes([p[0], p[1]]);
                            let dy = i16::from_be_bytes([p[2], p[3]]);
                            engine.on_raw_xy(&cfg.engine, dx, dy, out);
                        }
                        _ => {}
                    }
                } else {
                    self.send(FromDp::Notification(f));
                }
            }
            Kind::Foreign => {
                // The kernel (swid 1) re-sets the wheel mode on reconnect; the worker
                // re-applies our wheel settings when it sees that.
                if f.swid_raw() == SoftwareId::KERNEL.get() {
                    self.send(FromDp::KernelTraffic(f));
                }
            }
        }
    }

    fn drain_commands(&mut self, engine: &mut Engine, out: &mut Out) -> Flow {
        let mut buf = [0u8; 8];
        let _ = rustix::io::read(&self.wake, &mut buf);
        while let Ok(cmd) = self.rx.pop() {
            match cmd {
                ToDp::Send(f) => {
                    if let Err(e) = self.hidraw.send(&f) {
                        debug!("hidraw write failed: {e}");
                    }
                }
                ToDp::SetDpi(d) => engine.set_dpi(d),
                ToDp::SetProfile(p) => engine.set_profile(p),
                ToDp::ReleaseAll => {
                    engine.release_all(out);
                }
                ToDp::EmitKeys(k) => engine.emit_keys(k, out),
                ToDp::Lease(l) => engine.set_lease(l),
                ToDp::Stop => {
                    engine.release_all(out);
                    if engine.is_grabbed()
                        && let Some(dev) = self.evdev.as_mut()
                    {
                        let _ = dev.ungrab();
                        engine.set_grabbed(false);
                    }
                    return Flow::Stop;
                }
            }
        }
        Flow::Continue
    }

    /// Run until stopped or the device disappears. Returns the reason.
    #[expect(
        clippy::too_many_lines,
        reason = "predates the lint; split it when next changed (ADR 0008)"
    )]
    pub fn run(mut self) -> &'static str {
        let mut engine = Engine::default();
        let mut out = Out::default();
        let mut ev_buf_ok = true;
        loop {
            let cfg_guard = self.config.load();
            let cfg: &CompiledConfig = &cfg_guard;
            let timeout = engine.next_deadline(&cfg.engine).map(|d| {
                let left = d.saturating_duration_since(Instant::now());
                Timespec {
                    tv_sec: left.as_secs() as _,
                    tv_nsec: left.subsec_nanos() as _,
                }
            });
            let (hid_ready, ev_ready, wake_ready) = {
                let hid = self.hidraw.fd();
                let wake = self.wake.as_fd();
                let mut fds = [
                    PollFd::new(&hid, PollFlags::IN),
                    PollFd::new(&wake, PollFlags::IN),
                    PollFd::new(&wake, PollFlags::empty()),
                ];
                let ev_fd = self.evdev.as_ref().filter(|_| ev_buf_ok).map(|d| d.as_fd());
                let n = if let Some(efd) = ev_fd.as_ref() {
                    fds[2] = PollFd::new(efd, PollFlags::IN);
                    3
                } else {
                    2
                };
                match poll(&mut fds[..n], timeout.as_ref()) {
                    Ok(_) => {}
                    Err(rustix::io::Errno::INTR) => continue,
                    Err(e) => {
                        warn!("poll failed: {e}");
                        return "poll failed";
                    }
                }
                let hid_ev = fds[0].revents();
                if hid_ev.intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL) {
                    engine.release_all(&mut out);
                    self.flush(&mut out);
                    return "device disconnected";
                }
                (
                    hid_ev.contains(PollFlags::IN),
                    n == 3 && !fds[2].revents().is_empty(),
                    fds[1].revents().contains(PollFlags::IN),
                )
            };

            if wake_ready && let Flow::Stop = self.drain_commands(&mut engine, &mut out) {
                self.flush(&mut out);
                return "stopped";
            }

            if hid_ready {
                loop {
                    match self.hidraw.try_read() {
                        Ok(Some(f)) => self.on_frame(f, &mut engine, cfg, &mut out),
                        Ok(None) => break,
                        Err(_) => {
                            engine.release_all(&mut out);
                            self.flush(&mut out);
                            return "device disconnected";
                        }
                    }
                }
            }

            if ev_ready && let Some(dev) = self.evdev.as_mut() {
                let mut dropped = false;
                let r = dev.fetch_events().map(|evs| {
                    let now = Instant::now();
                    for ev in evs {
                        let (ty, c, v) = (ev.event_type().0, ev.code(), ev.value());
                        if ty == code::EV_SYN && c == code::SYN_DROPPED {
                            dropped = true;
                            continue;
                        }
                        engine.on_evdev(&cfg.engine, ty, c, v, now, &mut out);
                    }
                });
                if dropped && let Ok(keys) = dev.get_key_state() {
                    let mut held = 0u16;
                    for k in keys.iter() {
                        if (code::BTN_LEFT..=code::BTN_TASK).contains(&k.code()) {
                            held |= 1 << (k.code() - code::BTN_LEFT);
                        }
                    }
                    engine.resync_buttons(held);
                }
                match r {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => {
                        // The evdev node vanished (device going away) or broke: keep
                        // HID++ working without chords rather than failing entirely.
                        debug!("evdev read failed: {e}");
                        ev_buf_ok = false;
                    }
                }
            }

            if engine
                .next_deadline(&cfg.engine)
                .is_some_and(|d| d <= Instant::now())
            {
                engine.tick(&cfg.engine, Instant::now(), &mut out);
            }
            self.apply_grab(&mut engine, cfg, &mut out);
            self.flush(&mut out);
        }
    }
}
