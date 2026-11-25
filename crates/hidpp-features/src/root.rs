//! Root (0x0000). Always at feature index 0.
//! fn 0 getFeature(featureId) -> (index, type, version)
//! fn 1 getProtocolVersion / ping

use detent_hidpp_proto::FeatureId;

use crate::{RequestError, Requester};

pub const INDEX: u8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureInfo {
    pub index: u8,
    pub flags: u8,
    pub version: u8,
}

/// Look up a feature's index. Returns `Ok(None)` if the device doesn't have it.
pub fn get_feature(
    dev: &mut dyn Requester,
    id: FeatureId,
) -> Result<Option<FeatureInfo>, RequestError> {
    let resp = dev.request(INDEX, 0, &id.bytes())?;
    let p = resp.params();
    let (index, flags, version) = (
        *p.first().ok_or(RequestError::Malformed)?,
        p.get(1).copied().unwrap_or(0),
        p.get(2).copied().unwrap_or(0),
    );
    Ok(
        (index != 0 || id == FeatureId::ROOT).then_some(FeatureInfo {
            index,
            flags,
            version,
        }),
    )
}

/// HID++ protocol version (major, minor). Also serves as a ping.
pub fn protocol_version(dev: &mut dyn Requester) -> Result<(u8, u8), RequestError> {
    let resp = dev.request(INDEX, 1, &[0, 0, 0x5A])?;
    let p = resp.params();
    Ok((
        p.first().copied().unwrap_or(0),
        p.get(1).copied().unwrap_or(0),
    ))
}
