//! Receivers (Logi Bolt) in the hotplug thread: attach every paired device that answers,
//! and watch receivers whose paired devices are off, so one that wakes up attaches at once
//! instead of at the next 60 s rescan (docs/specs/hidpp.md, "Receivers").
//!
//! A device that powers on through the receiver sends a 0x1D4B notification at its slot
//! (verified on the owner's MX Master 4, 2026-11-25). The watch fd sees it; any frame from
//! a paired-but-unattached slot triggers a rescan.

use std::collections::BTreeMap;
use std::os::fd::BorrowedFd;
use std::path::PathBuf;
use std::sync::Arc;

use detent_device::receiver;
use detent_hidpp_proto::DeviceIndex;
use detent_hidpp_transport::discover::DeviceInfo;
use detent_hidpp_transport::hidraw::HidrawTransport;
use tracing::debug;

use crate::device::Target;
use crate::shared::{Shared, lock};

/// Slots attached on `node`.
fn attached_slots(shared: &Shared, node: &str) -> Vec<u8> {
    lock(&shared.devices)
        .values()
        .filter(|d| d.node == node)
        .filter_map(|d| d.slot)
        .collect()
}

/// Attach every paired device on this receiver that isn't attached yet. Returns the slots
/// still unattached (device off or out of range).
pub fn attach_slots(
    info: &DeviceInfo,
    shared: &Arc<Shared>,
    attach: impl Fn(&Target, &Arc<Shared>),
) -> Vec<u8> {
    let node = info.node.display().to_string();
    let paired = match HidrawTransport::open(&info.node)
        .map_err(Into::into)
        .and_then(|mut t| receiver::bolt_paired_all(&mut t))
    {
        Ok(p) => p,
        Err(e) => {
            debug!("{node}: reading the receiver's pairings failed: {e}");
            return Vec::new();
        }
    };
    let done = attached_slots(shared, &node);
    for p in paired.iter().filter(|p| !done.contains(&p.slot)) {
        debug!(
            "{node}: slot {} paired: {:04X} {} ({})",
            p.slot,
            p.wpid,
            p.serial,
            p.name.as_deref().unwrap_or("?")
        );
        attach(
            &Target {
                info: info.clone(),
                index: p.index(),
                paired: Some(p.clone()),
            },
            shared,
        );
    }
    let done = attached_slots(shared, &node);
    paired
        .into_iter()
        .map(|p| p.slot)
        .filter(|s| !done.contains(s))
        .collect()
}

/// Receivers with paired devices that aren't attached, held open to notice them wake up.
#[derive(Default)]
pub struct Watch {
    nodes: BTreeMap<PathBuf, (HidrawTransport, Vec<u8>)>,
}

impl Watch {
    /// Replace the watched set with `pending` (node → unattached slots). Keeps fds already
    /// open for nodes still pending.
    pub fn update(&mut self, pending: Vec<(PathBuf, Vec<u8>)>) {
        let mut next = BTreeMap::new();
        for (node, slots) in pending {
            let t = match self.nodes.remove(&node) {
                Some((t, _)) => t,
                None => match HidrawTransport::open(&node) {
                    Ok(t) => t,
                    Err(e) => {
                        debug!("{}: cannot watch: {e}", node.display());
                        continue;
                    }
                },
            };
            next.insert(node, (t, slots));
        }
        self.nodes = next;
    }

    pub fn fds(&self) -> impl Iterator<Item = BorrowedFd<'_>> {
        self.nodes.values().map(|(t, _)| t.fd())
    }

    /// Drain every watched node. `true`: a pending slot spoke (it woke up). A node that
    /// errors (receiver unplugged) is dropped; hotplug rescans on its own.
    pub fn drain(&mut self) -> bool {
        let mut woke = false;
        self.nodes.retain(|node, (t, slots)| {
            loop {
                match t.try_read() {
                    Ok(Some(f)) => {
                        let DeviceIndex(i) = f.device_index();
                        if slots.contains(&i) {
                            debug!("{}: slot {i} woke up", node.display());
                            woke = true;
                        }
                    }
                    Ok(None) => break true,
                    Err(_) => break false,
                }
            }
        });
        woke
    }
}
