//! Commands that talk to the mouse directly (no daemon needed): hidraw on Linux, hidapi on
//! macOS and Windows (`OsTransport`, ADR 0010). `set` and `reset` write device settings;
//! everything else only reads.

use std::path::Path;

use anyhow::{Context, Result, bail};
use detent_device::DirectRequester;
use detent_device::receiver::{self, Paired};
use detent_device::requester::error_name;
use detent_hidpp_features::{RequestError, device_info, reprog_controls, root};
use detent_hidpp_proto::{DeviceIndex, FeatureId};
use detent_hidpp_transport::OsTransport;
use detent_hidpp_transport::discover::{self, DeviceInfo};
use serde::Serialize;

use crate::exit::{self, Exit};
use crate::names;

pub type Dev = DirectRequester<OsTransport>;

/// One device to talk to: a node plus the index it answers at (0xFF direct, 1–6 behind a
/// receiver, with the receiver's pairing info).
#[derive(Debug, Clone)]
pub struct Endpoint {
    pub info: DeviceInfo,
    pub index: DeviceIndex,
    pub paired: Option<Paired>,
}

impl Endpoint {
    /// The device's name: the receiver's pairing name for a slot, else the HID name.
    pub fn name(&self) -> &str {
        self.paired
            .as_ref()
            .and_then(|p| p.name.as_deref())
            .unwrap_or(&self.info.name)
    }

    pub fn slot(&self) -> Option<u8> {
        self.paired.as_ref().map(|p| p.slot)
    }

    /// Product id of the device itself (the wpid behind a receiver).
    pub fn product(&self) -> u32 {
        self.paired
            .as_ref()
            .map_or(self.info.product, |p| u32::from(p.wpid))
    }

    /// `/dev/hidraw10` or, behind a receiver, `/dev/hidraw10 #2`.
    pub fn label(&self) -> String {
        match self.slot() {
            Some(s) => format!("{} #{s}", self.info.node.display()),
            None => self.info.node.display().to_string(),
        }
    }

    fn matches(&self, s: &str) -> bool {
        let node = self.info.node.to_string_lossy();
        node == s
            || Path::new(&*node).file_name().is_some_and(|f| f == s)
            || self.name().to_lowercase().contains(&s.to_lowercase())
    }
}

/// Every Logitech device: direct nodes as they are, receivers expanded into their paired
/// devices (Bolt pairing registers; read-only). A receiver whose pairings can't be read is
/// listed as itself.
pub fn endpoints() -> Result<Vec<Endpoint>> {
    let devs = discover::logitech().context("listing Logitech HID devices")?;
    let mut out = Vec::new();
    for info in devs {
        let paired = if info.receiver {
            OsTransport::open(&info.node)
                .ok()
                .and_then(|mut t| receiver::bolt_paired_all(&mut t).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if paired.is_empty() {
            out.push(Endpoint {
                info,
                index: DeviceIndex::DIRECT,
                paired: None,
            });
            continue;
        }
        for p in paired {
            out.push(Endpoint {
                info: info.clone(),
                index: p.index(),
                paired: Some(p),
            });
        }
    }
    Ok(out)
}

/// Pick a device: `sel` matches a node path (`/dev/hidraw10`), a node name (`hidraw10`)
/// or a case-insensitive substring of the device name. Default: first device that
/// answers HID++ 2.0 (receiver slots at their index, everything else at 0xFF).
pub fn open(sel: Option<&str>) -> Result<(Endpoint, Dev)> {
    let devs = endpoints()?;
    if devs.is_empty() {
        return Err(Exit::new(
            exit::NOINPUT,
            "no Logitech HID++ devices found (is the mouse connected?)",
        )
        .into());
    }
    let candidates: Vec<Endpoint> = match sel {
        Some(s) => devs.into_iter().filter(|d| d.matches(s)).collect(),
        None => devs,
    };
    let mut last_err = None;
    for ep in candidates {
        match try_open(&ep) {
            Ok(dev) => return Ok((ep, dev)),
            Err(e) => last_err = Some(e.context(ep.label())),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        Exit::new(
            exit::NOINPUT,
            format!("no device matches `{}`", sel.unwrap_or("")),
        )
        .into()
    }))
}

/// What to do about "permission denied" on this OS.
const PERMISSION_HINT: &str = if cfg!(target_os = "linux") {
    "install the udev rules (see docs/user/quickstart.md)"
} else {
    "another program may hold the device exclusively (see docs/specs/transport.md)"
};

fn try_open(ep: &Endpoint) -> Result<Dev> {
    let info = &ep.info;
    let t = OsTransport::open(&info.node).map_err(|e| -> anyhow::Error {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            Exit::new(
                exit::NOPERM,
                format!(
                    "permission denied opening {}: {PERMISSION_HINT}",
                    info.node.display()
                ),
            )
            .into()
        } else {
            e.into()
        }
    })?;
    let mut dev = DirectRequester::new(t, ep.index);
    let (major, minor) = root::protocol_version(&mut dev).map_err(|e| match e {
        // The receiver answers with an error when nothing is connected at the slot.
        RequestError::Device(_) if ep.slot().is_some() => {
            anyhow::anyhow!("not connected (off, asleep or out of range)")
        }
        e => req_err(e),
    })?;
    if major < 2 {
        bail!("speaks HID++ {major}.{minor}; only 2.0+ is supported");
    }
    dev.load_features().map_err(req_err)?;
    Ok(dev)
}

pub fn req_err(e: RequestError) -> anyhow::Error {
    match e {
        RequestError::Device(code) => {
            anyhow::anyhow!("device error 0x{code:02X} ({})", error_name(code))
        }
        other => other.into(),
    }
}

#[derive(Serialize)]
struct FeatureRow {
    index: u8,
    id: String,
    name: Option<&'static str>,
    version: u8,
    flags: Vec<&'static str>,
}

#[derive(Serialize)]
struct FeatureDump {
    node: String,
    name: Option<String>,
    kind: Option<&'static str>,
    protocol: (u8, u8),
    firmware: Vec<device_info::Firmware>,
    features: Vec<FeatureRow>,
}

fn flag_names(flags: u8) -> Vec<&'static str> {
    let mut v = Vec::new();
    if flags & 0x80 != 0 {
        v.push("obsolete");
    }
    if flags & 0x40 != 0 {
        v.push("hidden");
    }
    if flags & 0x20 != 0 {
        v.push("engineering");
    }
    if flags & 0x10 != 0 {
        v.push("manufacturing-deactivatable");
    }
    if flags & 0x08 != 0 {
        v.push("compliance-deactivatable");
    }
    v
}

pub fn dump_features(sel: Option<&str>, json: bool) -> Result<()> {
    let (ep, mut dev) = open(sel)?;
    let protocol = root::protocol_version(&mut dev).map_err(req_err)?;
    let name = device_info::name(&mut dev).ok();
    let kind = device_info::device_type(&mut dev).ok();
    let firmware = device_info::firmware(&mut dev).unwrap_or_default();
    let features = dev
        .features()
        .iter()
        .map(|e| FeatureRow {
            index: e.index,
            id: format!("0x{:04X}", e.id.0),
            name: e.id.name(),
            version: e.version,
            flags: flag_names(e.flags),
        })
        .collect::<Vec<_>>();
    let mut features = features;
    features.sort_by_key(|f| f.index);
    let dump = FeatureDump {
        node: ep.label(),
        name,
        kind,
        protocol,
        firmware,
        features,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&dump)?);
        return Ok(());
    }
    println!(
        "{} ({}) at {}  HID++ {}.{}",
        dump.name.as_deref().unwrap_or(ep.name()),
        dump.kind.unwrap_or("?"),
        dump.node,
        protocol.0,
        protocol.1
    );
    for fw in &dump.firmware {
        println!("  firmware  {:<18} {}", fw.kind, fw.version);
    }
    println!("\n  idx  id      ver  name");
    for f in &dump.features {
        println!(
            "  {:>3}  {}  {:>3}  {}{}",
            f.index,
            f.id,
            f.version,
            f.name.unwrap_or("(unknown)"),
            if f.flags.is_empty() {
                String::new()
            } else {
                format!("  [{}]", f.flags.join(", "))
            }
        );
    }
    Ok(())
}

/// Clear temporary diversion / raw XY on every control: the mouse behaves as with no
/// software running. Persistent settings are not touched.
pub fn reset(sel: Option<&str>, quiet: bool) -> Result<()> {
    let (ep, mut dev) = open(sel)?;
    let controls = reprog_controls::controls(&mut dev).map_err(req_err)?;
    let mut n = 0;
    for c in controls.iter().filter(|c| c.divertable) {
        let r = reprog_controls::reporting(&mut dev, c.cid).map_err(req_err)?;
        if r.diverted || r.raw_xy {
            reprog_controls::set_reporting(&mut dev, c.cid, false, false).map_err(req_err)?;
            if !quiet {
                println!("restored 0x{:04X} {}", c.cid, names::cid_name(c.cid));
            }
            n += 1;
        }
    }
    if !quiet {
        println!(
            "{}: {n} control(s) restored to default reporting",
            ep.name()
        );
    }
    Ok(())
}

/// Apply one setting directly (daemon not running). Same paths as `device.set`.
pub fn set(sel: Option<&str>, path: &str, value: &str) -> Result<()> {
    use detent_hidpp_features::{Feature, adjustable_dpi, haptics, smartshift, wheel};
    // Validate before touching the device (same catalogue as the daemon).
    let setting = detent_ipc::settings::find(path)
        .ok_or_else(|| Exit::new(exit::DATAERR, detent_ipc::settings::unknown(path)))?;
    let v = setting
        .parse(value)
        .map_err(|e| Exit::new(exit::DATAERR, e))?;
    let num = v.as_u64().unwrap_or(0);
    let flag = v.as_bool().unwrap_or(false);
    let (_, mut dev) = open(sel)?;
    match path {
        "dpi" => {
            let cur = adjustable_dpi::AdjustableDpi::read(&mut dev).map_err(req_err)?;
            if !cur.accepts(num as u16) {
                return Err(Exit::new(
                    exit::DATAERR,
                    format!(
                        "dpi must be {}..={} in steps of {}",
                        cur.min, cur.max, cur.step
                    ),
                )
                .into());
            }
            let mut d = cur.clone();
            d.dpi = num as u16;
            adjustable_dpi::AdjustableDpi::apply(&mut dev, &d, &cur).map_err(req_err)?;
        }
        "smartshift.mode" | "smartshift.auto_threshold" | "smartshift.torque" => {
            let cur = smartshift::SmartShift::read(&mut dev).map_err(req_err)?;
            let mut d = cur.clone();
            match path {
                "smartshift.mode" => {
                    d.mode = smartshift::WheelMode::parse(value.trim())
                        .ok_or_else(|| anyhow::anyhow!("mode is `ratchet` or `free-spin`"))?
                }
                "smartshift.auto_threshold" => d.auto_threshold = num as u8,
                _ => d.torque = Some(num as u8),
            }
            smartshift::SmartShift::apply(&mut dev, &d, &cur).map_err(req_err)?;
        }
        "haptics.enabled" | "haptics.intensity" => {
            let cur = haptics::Haptics::read(&mut dev).map_err(req_err)?;
            let mut d = cur.clone();
            if path == "haptics.enabled" {
                d.enabled = flag;
            } else {
                d.intensity = num as u8;
            }
            haptics::Haptics::apply(&mut dev, &d, &cur).map_err(req_err)?;
        }
        "thumbwheel.invert" => {
            let cur = wheel::Thumbwheel::read(&mut dev).map_err(req_err)?;
            let mut d = cur.clone();
            d.invert = flag;
            wheel::Thumbwheel::apply(&mut dev, &d, &cur).map_err(req_err)?;
        }
        "wheel.invert" => {
            let cur = wheel::HiResWheel::read(&mut dev).map_err(req_err)?;
            let mut d = cur.clone();
            d.invert = flag;
            wheel::HiResWheel::apply(&mut dev, &d, &cur).map_err(req_err)?;
        }
        other => bail!("internal: `{other}` is in the catalogue but has no direct path"),
    }
    Ok(())
}

/// One-shot state read straight from the device (used when the daemon isn't running).
pub fn read_state(sel: Option<&str>) -> Result<detent_device::DeviceState> {
    let (ep, mut dev) = open(sel)?;
    let identity = detent_device::state::identify(&mut dev, &ep.label());
    // Same identity the daemon uses (bins/detentd/src/device.rs): wpid + unit id behind a
    // receiver, HID product + HID_UNIQ for a direct device.
    let id = detent_device::DeviceId {
        product: ep.product(),
        serial: match &ep.paired {
            Some(p) => p.serial.clone(),
            None => ep.info.uniq.clone().unwrap_or_default(),
        },
    };
    let mut st = detent_device::DeviceState::new(id, identity, dev.features().clone());
    st.read_all(&mut dev);
    Ok(st)
}

#[derive(Serialize)]
struct ControlRow {
    index: usize,
    cid: String,
    name: &'static str,
    task: String,
    divertable: bool,
    persistently_divertable: bool,
    virtual_: bool,
    raw_xy: bool,
    raw_wheel: bool,
    pos: u8,
    group: u8,
    gmask: String,
    flags: String,
    flags2: String,
    reporting: Option<reprog_controls::Reporting>,
}

pub fn dump_controls(sel: Option<&str>, json: bool) -> Result<()> {
    let (ep, mut dev) = open(sel)?;
    if dev
        .features()
        .index_of(FeatureId::REPROG_CONTROLS_V4)
        .is_none()
    {
        bail!("{} has no ReprogControlsV4 (0x1B04)", ep.name());
    }
    let controls = reprog_controls::controls(&mut dev).map_err(req_err)?;
    let mut rows = Vec::new();
    for (i, c) in controls.iter().enumerate() {
        let reporting = reprog_controls::reporting(&mut dev, c.cid).ok();
        rows.push(ControlRow {
            index: i,
            cid: format!("0x{:04X}", c.cid),
            name: names::cid_name(c.cid),
            task: format!("0x{:04X}", c.task_id),
            divertable: c.divertable,
            persistently_divertable: c.persistently_divertable,
            virtual_: c.virtual_,
            raw_xy: c.raw_xy_capable,
            raw_wheel: c.raw_wheel_capable,
            pos: c.pos,
            group: c.group,
            gmask: format!("0x{:02X}", c.gmask),
            flags: format!("0x{:02X}", c.flags),
            flags2: format!("0x{:02X}", c.flags2),
            reporting,
        });
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    println!("{} at {}: {} controls\n", ep.name(), ep.label(), rows.len());
    println!("  cid     task    name                      div  pers virt rawXY  reporting");
    for r in &rows {
        let rep = match &r.reporting {
            Some(rep) => {
                let mut s = Vec::new();
                if rep.diverted {
                    s.push("diverted".to_string());
                }
                if rep.raw_xy {
                    s.push("rawXY".to_string());
                }
                if rep.persist {
                    s.push("persist".to_string());
                }
                if rep.remap != 0 && rep.remap != u16::from_str_radix(&r.cid[2..], 16).unwrap_or(0)
                {
                    s.push(format!("remap→0x{:04X}", rep.remap));
                }
                if s.is_empty() {
                    "default".to_string()
                } else {
                    s.join(",")
                }
            }
            None => "?".to_string(),
        };
        let yn = |b: bool| if b { "yes" } else { "-" };
        println!(
            "  {}  {}  {:<24}  {:<3}  {:<4} {:<4} {:<5}  {}",
            r.cid,
            r.task,
            r.name,
            yn(r.divertable),
            yn(r.persistently_divertable),
            yn(r.virtual_),
            yn(r.raw_xy),
            rep
        );
    }
    Ok(())
}
