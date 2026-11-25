//! Attach a hidraw node: discover over HID++, then hand the fds to a new data-plane
//! thread and start the device's worker.

use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::sync_channel;

use anyhow::{Context, Result, bail};
use detent_device::DirectRequester;
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

/// The mouse's evdev node: `/sys/class/hidraw/hidrawN/device/input/input*/event*`.
fn evdev_node(hidraw: &Path) -> Option<PathBuf> {
    let name = hidraw.file_name()?;
    let input_dir = Path::new("/sys/class/hidraw")
        .join(name)
        .join("device/input");
    for input in std::fs::read_dir(input_dir).ok()?.flatten() {
        for e in std::fs::read_dir(input.path()).ok()?.flatten() {
            let n = e.file_name();
            if n.to_string_lossy().starts_with("event") {
                return Some(Path::new("/dev/input").join(n));
            }
        }
    }
    None
}

fn open_evdev(hidraw: &Path) -> Option<RawDevice> {
    let node = evdev_node(hidraw)?;
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

/// Probe and attach. Errors mean "not a HID++ 2.0 device we can use" (or no access).
pub fn attach(info: &DeviceInfo, shared: &Arc<Shared>) -> Result<()> {
    let node = info.node.display().to_string();
    let t = HidrawTransport::open(&info.node).with_context(|| format!("open {node}"))?;
    let mut dev = DirectRequester::new(t, DeviceIndex::DIRECT);
    let (major, _) = root::protocol_version(&mut dev).context("HID++ ping")?;
    if major < 2 {
        bail!("HID++ {major}.x device: unsupported");
    }
    dev.load_features().context("feature enumeration")?;
    let identity = identify(&mut dev, &node);
    let features = dev.features().clone();
    let id = DeviceId {
        product: info.product,
        serial: info.uniq.clone().unwrap_or_default(),
    };
    let key = if id.serial.is_empty() {
        node.clone()
    } else {
        format!("{:04X}:{}", id.product, id.serial)
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
    let evdev = open_evdev(&info.node);
    let wake = eventfd(0, EventfdFlags::NONBLOCK | EventfdFlags::CLOEXEC)?;
    let wake_dp = wake.try_clone()?;
    let (ring_tx, ring_rx) = rtrb::RingBuffer::new(64);
    let (tx, rx) = sync_channel::<WorkerMsg>(256);
    let stats = Arc::new(Stats::default());

    let dp = DataPlane {
        hidraw,
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
        Link { tx: ring_tx, wake },
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
            stats,
        },
    );
    std::thread::Builder::new()
        .name(format!("detent-worker-{key}"))
        .spawn(move || worker.run())?;
    info!("attached {} at {node} ({key})", identity.name);
    Ok(())
}
