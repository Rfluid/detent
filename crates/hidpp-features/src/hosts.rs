//! Easy-Switch hosts: ChangeHost (0x1814) and HostsInfo (0x1815).
//!
//! 0x1814 (logiops `ChangeHost.cpp`, Solaar `ChangeHost` setting):
//! - fn 0 getHostInfo → `count u8, current u8 (0-based), flags u8 (bit0 enhanced host switch)`
//! - fn 1 setCurrentHost(slot): **no reply**, the device drops this link at once
//!
//! 0x1815 (Solaar `get_host_names` / `set_host_name`; layouts marked ⚠ in
//! docs/research/hidpp-layouts-device.md until checked on the device):
//! - fn 0 getFeatureInfo → `caps u8 (bit0 names readable, bit1 writable), ?, count, current`
//! - fn 1 getHostInfo(slot) → `slot, status (0 empty, else paired), ?, ?, nameLen, nameMaxLen`
//! - fn 3 getHostFriendlyName(slot, offset) → `slot, offset, up to 14 name bytes`
//! - fn 4 setHostFriendlyName(slot, offset, up to 14 bytes)
//!
//! Switching is a command, never desired state: it isn't re-applied on reconnect
//! (Solaar: "persisting this setting is harmful"). Names are written only on an
//! explicit request (never the hostname behind the user's back, as Solaar does).

use detent_hidpp_proto::FeatureId;
use serde::{Deserialize, Serialize};

use crate::{RequestError, Requester};

/// Name bytes per getHostFriendlyName / setHostFriendlyName call.
const NAME_CHUNK: usize = 14;

mod caps {
    pub const READ_NAMES: u8 = 0x01;
    pub const WRITE_NAMES: u8 = 0x02;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSlot {
    /// 0-based slot (Easy-Switch channel - 1).
    pub slot: u8,
    /// `None` when the device has no HostsInfo or didn't answer.
    pub paired: Option<bool>,
    /// The name the device stores (usually set by Logi Options+ or Solaar).
    pub name: Option<String>,
    /// Longest name the slot accepts.
    pub name_max: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostsState {
    pub count: u8,
    /// 0-based current slot (Easy-Switch channel - 1).
    pub current: u8,
    /// ChangeHost flags bit0 ("enhanced host switch"; meaning not documented).
    pub enhanced: bool,
    /// HostsInfo says names can be written.
    pub can_rename: bool,
    /// One entry per slot, in order. Names are empty without HostsInfo.
    pub slots: Vec<HostSlot>,
}

/// Where to switch: a 0-based slot, or relative to the current one (wrapping).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Slot(u8),
    Next,
    Prev,
}

impl Target {
    /// The 0-based slot this target means on a device with `count` slots, now on
    /// `current`. `None` when the slot doesn't exist.
    pub fn resolve(self, count: u8, current: u8) -> Option<u8> {
        if count == 0 {
            return None;
        }
        let s = match self {
            Self::Slot(s) => s,
            Self::Next => (current % count + 1) % count,
            Self::Prev => (current % count + count - 1) % count,
        };
        (s < count).then_some(s)
    }

    /// `1..=N` (1-based, as printed on the mouse), `next` or `prev`/`previous`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "next" => Some(Self::Next),
            "prev" | "previous" => Some(Self::Prev),
            n => n
                .parse::<u8>()
                .ok()
                .filter(|&n| n >= 1)
                .map(|n| Self::Slot(n - 1)),
        }
    }
}

fn idx(dev: &dyn Requester, id: FeatureId) -> Result<u8, RequestError> {
    dev.feature_index(id).ok_or(RequestError::Unsupported(id))
}

/// ChangeHost fn 0: `(count, current, enhanced)`.
pub fn info(dev: &mut dyn Requester) -> Result<(u8, u8, bool), RequestError> {
    let i = idx(dev, FeatureId::CHANGE_HOST)?;
    let r = dev.request(i, 0, &[])?;
    let p = r.params();
    if p.len() < 2 {
        return Err(RequestError::Malformed);
    }
    Ok((p[0], p[1], p.get(2).is_some_and(|f| f & 0x01 != 0)))
}

/// Everything about the slots. HostsInfo is best effort: without it (or if it fails)
/// the slots come back with no names, so switching keeps working.
pub fn read(dev: &mut dyn Requester) -> Result<HostsState, RequestError> {
    let (count, current, enhanced) = info(dev)?;
    let mut st = HostsState {
        count,
        current,
        enhanced,
        can_rename: false,
        slots: (0..count)
            .map(|slot| HostSlot {
                slot,
                paired: None,
                name: None,
                name_max: None,
            })
            .collect(),
    };
    if dev.feature_index(FeatureId::HOSTS_INFO).is_some() {
        match read_names(dev, count) {
            Ok((can_rename, slots)) => {
                st.can_rename = can_rename;
                st.slots = slots;
            }
            Err(RequestError::Io(e)) => return Err(RequestError::Io(e)),
            Err(_) => {} // names are a nicety; the device may not support reading them
        }
    }
    Ok(st)
}

/// HostsInfo: `(names writable, slots)`.
fn read_names(dev: &mut dyn Requester, count: u8) -> Result<(bool, Vec<HostSlot>), RequestError> {
    let i = idx(dev, FeatureId::HOSTS_INFO)?;
    let r = dev.request(i, 0, &[])?;
    let c = *r.params().first().ok_or(RequestError::Malformed)?;
    let mut slots = Vec::with_capacity(count as usize);
    for slot in 0..count {
        let r = dev.request(i, 1, &[slot])?;
        let p = r.params();
        if p.len() < 6 {
            return Err(RequestError::Malformed);
        }
        let (paired, len, max) = (p[1] != 0, p[4], p[5]);
        let name = if c & caps::READ_NAMES != 0 && len > 0 {
            Some(read_name(dev, i, slot, len)?)
        } else {
            None
        };
        slots.push(HostSlot {
            slot,
            paired: Some(paired),
            name,
            name_max: Some(max),
        });
    }
    Ok((c & caps::WRITE_NAMES != 0, slots))
}

fn read_name(dev: &mut dyn Requester, i: u8, slot: u8, len: u8) -> Result<String, RequestError> {
    let len = len as usize;
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let r = dev.request(i, 3, &[slot, out.len() as u8])?;
        let p = r.params();
        let take = p
            .len()
            .saturating_sub(2)
            .min(NAME_CHUNK)
            .min(len - out.len());
        if take == 0 {
            return Err(RequestError::Malformed);
        }
        out.extend_from_slice(&p[2..2 + take]);
    }
    Ok(String::from_utf8_lossy(&out)
        .trim_end_matches('\0')
        .to_string())
}

/// Move the mouse to another slot (fire and forget: the link drops at once). Returns the
/// 0-based slot it switched to, or `Ok(None)` when it's already there.
pub fn switch(dev: &mut dyn Requester, to: Target) -> Result<Option<u8>, RequestError> {
    let (count, current, _) = info(dev)?;
    let slot = to.resolve(count, current).ok_or_else(|| {
        RequestError::OutOfRange(format!("host must be 1..={count}, next or prev"))
    })?;
    if slot == current {
        return Ok(None);
    }
    let i = idx(dev, FeatureId::CHANGE_HOST)?;
    dev.send(i, 1, &[slot])?;
    Ok(Some(slot))
}

/// Check a new slot name before touching the device.
pub fn validate_name(name: &str, max: Option<u8>) -> Result<(), RequestError> {
    let n = name.len();
    let max = max.map_or(usize::from(u8::MAX), usize::from);
    if name.trim().is_empty() {
        return Err(RequestError::OutOfRange("the name is empty".into()));
    }
    if n > max {
        return Err(RequestError::OutOfRange(format!(
            "the name is {n} bytes; this slot takes at most {max}"
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(RequestError::OutOfRange(
            "the name has control characters".into(),
        ));
    }
    Ok(())
}

/// Store `name` as the slot's friendly name (HostsInfo fn 4, in 14-byte chunks), then read
/// the slots back so the caller shows what the device kept.
pub fn rename(dev: &mut dyn Requester, slot: u8, name: &str) -> Result<HostsState, RequestError> {
    let i = idx(dev, FeatureId::HOSTS_INFO)?;
    let before = read(dev)?;
    if !before.can_rename {
        return Err(RequestError::OutOfRange(
            "this device doesn't let software rename its hosts".into(),
        ));
    }
    let s = before
        .slots
        .get(slot as usize)
        .ok_or_else(|| RequestError::OutOfRange(format!("host must be 1..={}", before.count)))?;
    validate_name(name, s.name_max)?;
    for (n, chunk) in name.as_bytes().chunks(NAME_CHUNK).enumerate() {
        let mut params = Vec::with_capacity(2 + chunk.len());
        params.extend_from_slice(&[slot, (n * NAME_CHUNK) as u8]);
        params.extend_from_slice(chunk);
        dev.request(i, 4, &params)?;
    }
    read(dev)
}

#[cfg(test)]
mod tests;
