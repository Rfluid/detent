//! Attach a hidraw node: discover over HID++, then hand the fds to a new data-plane
//! thread and start the device's worker.

use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::sync_channel;

use anyhow::{Context, Result, bail};
use detent_device::DirectRequester;
use detent_device::receiver::Paired;
use detent_device::state::{DeviceId, DeviceState, identify};
use detent_hidpp_features::root;
use detent_hidpp_proto::{DeviceIndex, FeatureId};
use detent_hidpp_transport::discover::DeviceInfo;
use detent_hidpp_transport::hidraw::HidrawTransport;
use evdev::raw_stream::RawDevice;
use rustix::event::{EventfdFlags, eventfd};
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use tracing::{info, warn};

use crate::dataplane::{DataPlane, Indexes, Stats};
use crate::msg::WorkerMsg;
use crate::shared::{DeviceHandle, Shared, lock};
use crate::uinput::Uinput;
use crate::worker::{Link, Worker};

/// What to attach: a hidraw node and the index the device answers at.
pub struct Target {
    pub info: DeviceInfo,
    /// 0xFF direct; 1–6 a receiver slot.
    pub index: DeviceIndex,
    /// The receiver's pairing info (receiver slots only): the device's wpid and unit id.
    pub paired: Option<Paired>,
}

impl Target {
    pub fn direct(info: DeviceInfo) -> Self {
        Self {
            info,
            index: DeviceIndex::DIRECT,
            paired: None,
        }
    }

    pub fn slot(&self) -> Option<u8> {
        (self.index != DeviceIndex::DIRECT).then_some(self.index.0)
    }
}

/// The first `event*` node under `<input dir>/input*/` that passes `keep`.
fn first_event(input_dir: &Path, keep: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    for input in std::fs::read_dir(input_dir).ok()?.flatten() {
        if !keep(&input.path()) {
            continue;
        }
        for e in std::fs::read_dir(input.path()).ok()?.flatten() {
            let n = e.file_name();
            if n.to_string_lossy().starts_with("event") {
                return Some(Path::new("/dev/input").join(n));
            }
        }
    }
    None
}

/// Does `/sys/.../input/inputN` report relative X and Y (bits 0 and 1 of `capabilities/rel`)?
fn has_rel_xy(input: &Path) -> bool {
    std::fs::read_to_string(input.join("capabilities/rel"))
        .ok()
        .and_then(|s| {
            let last = s.split_whitespace().last()?;
            u64::from_str_radix(last, 16).ok()
        })
        .is_some_and(|bits| bits & 0x3 == 0x3)
}

/// The mouse's evdev node. Direct device: `/sys/class/hidraw/hidrawN/device/input/input*/event*`.
/// Receiver: the HID++ interface has no input device; pointer events come from a sibling
/// interface of the same USB device ("Logitech USB Receiver Mouse" on Bolt, interface 1),
/// so take the sibling input that reports relative X/Y.
fn evdev_node(hidraw: &Path, receiver: bool) -> Option<PathBuf> {
    let hid = Path::new("/sys/class/hidraw")
        .join(hidraw.file_name()?)
        .join("device");
    if !receiver {
        return first_event(&hid.join("input"), |_| true);
    }
    let usb_dev = std::fs::canonicalize(&hid)
        .ok()?
        .parent()?
        .parent()?
        .to_path_buf();
    for iface in std::fs::read_dir(usb_dev).ok()?.flatten() {
        for hid_dev in std::fs::read_dir(iface.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            if !hid_dev.file_name().to_string_lossy().starts_with("0003:") {
                continue;
            }
            if let Some(ev) = first_event(&hid_dev.path().join("input"), has_rel_xy) {
                return Some(ev);
            }
        }
    }
    None
}

fn open_evdev(hidraw: &Path, receiver: bool) -> Option<RawDevice> {
    let node = evdev_node(hidraw, receiver)?;
    match RawDevice::open(&node) {
        Ok(d) => {
            let fl = fcntl_getfl(d.as_fd()).ok()?;
            fcntl_setfl(d.as_fd(), fl | OFlags::NONBLOCK).ok()?;
            Some(d)
        }
        Err(e) => {
            warn!(
                "cannot open {} ({e}): chords on left/right/wheel are disabled. Install the \
                 udev rules (docs/user/quickstart.md).",
                node.display()
            );
            None
        }
    }
}

/// Stable identity. Receiver slots use the pairing info (wpid + unit id), direct devices
/// the HID product and `HID_UNIQ`.
fn device_id(target: &Target) -> DeviceId {
    match &target.paired {
        Some(p) => DeviceId {
            product: u32::from(p.wpid),
            serial: p.serial.clone(),
        },
        None => DeviceId {
            product: target.info.product,
            serial: target.info.uniq.clone().unwrap_or_default(),
        },
    }
}

/// Probe and attach. Errors mean "not a HID++ 2.0 device we can use" (or no access, or
/// a receiver slot whose device is off).
pub fn attach(target: &Target, shared: &Arc<Shared>) -> Result<()> {
    let info = &target.info;
    let node = info.node.display().to_string();
    let t = HidrawTransport::open(&info.node).with_context(|| format!("open {node}"))?;
    let mut dev = DirectRequester::new(t, target.index);
    let (major, _) = root::protocol_version(&mut dev).context("HID++ ping")?;
    if major < 2 {
        bail!("HID++ {major}.x device: unsupported");
    }
    dev.load_features().context("feature enumeration")?;
    // Shown to the user; `detentctl` labels receiver slots the same way.
    let label = match target.slot() {
        Some(s) => format!("{node} #{s}"),
        None => node.clone(),
    };
    let identity = identify(&mut dev, &label);
    let features = dev.features().clone();
    let id = device_id(target);
    let key = match (id.serial.is_empty(), target.slot()) {
        (false, _) => format!("{:04X}:{}", id.product, id.serial),
        (true, Some(s)) => format!("{node}#{s}"),
        (true, None) => node.clone(),
    };
    if lock(&shared.devices).contains_key(&key) {
        return Ok(());
    }
    let idx = Indexes {
        reprog: features
            .index_of(FeatureId::REPROG_CONTROLS_V4)
            .unwrap_or(0),
    };
    let state = DeviceState::new(id, identity.clone(), features);
    let hidraw = dev.into_transport();

    let uinput = Uinput::create().context(
        "creating the virtual input device (/dev/uinput): install the udev rules \
         (docs/user/quickstart.md)",
    )?;
    let evdev = open_evdev(&info.node, info.receiver);
    let wake = eventfd(0, EventfdFlags::NONBLOCK | EventfdFlags::CLOEXEC)?;
    let wake_dp = wake.try_clone()?;
    let (ring_tx, ring_rx) = rtrb::RingBuffer::new(64);
    let (tx, rx) = sync_channel::<WorkerMsg>(256);
    let stats = Arc::new(Stats::default());

    let dp = DataPlane {
        hidraw,
        device: target.index,
        evdev,
        uinput,
        wake: wake_dp,
        rx: ring_rx,
        tx: tx.clone(),
        dispatch: shared.dispatch.clone(),
        // The data plane runs the remap view (bindings stripped while paused).
        config: shared.active.clone(),
        idx,
        stats: stats.clone(),
    };
    let dp_tx = tx.clone();
    let dp_handle = std::thread::Builder::new()
        .name(format!("detent-dp-{}", info.node.display()))
        .spawn(move || {
            let reason = dp.run();
            let _ = dp_tx.send(WorkerMsg::Dp(crate::msg::FromDp::Gone(reason)));
            reason
        })?;

    let worker = Worker::new(
        key.clone(),
        Link {
            tx: ring_tx,
            wake,
            device: target.index,
        },
        rx,
        state,
        shared.clone(),
        dp_handle,
    );
    lock(&shared.devices).insert(
        key.clone(),
        DeviceHandle {
            tx,
            name: identity.name.clone(),
            node: node.clone(),
            slot: target.slot(),
            stats,
        },
    );
    std::thread::Builder::new()
        .name(format!("detent-worker-{key}"))
        .spawn(move || worker.run())?;
    info!("attached {} at {node} ({key})", identity.name);
    Ok(())
}
