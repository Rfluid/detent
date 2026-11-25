//! FeatureSet (0x0001): enumerate every feature the device advertises.
//! fn 0 getCount() -> count (excluding root)
//! fn 1 getFeatureId(index) -> (featureId, type, version)

use detent_hidpp_proto::FeatureId;

use crate::table::{FeatureEntry, FeatureTable};
use crate::{RequestError, Requester, root};

/// Build the full feature table. Phase 0's `detentctl dump-features` prints this.
pub fn enumerate(dev: &mut dyn Requester) -> Result<FeatureTable, RequestError> {
    let fs = root::get_feature(dev, FeatureId::FEATURE_SET)?
        .ok_or(RequestError::Unsupported(FeatureId::FEATURE_SET))?;
    let count = *dev
        .request(fs.index, 0, &[])?
        .params()
        .first()
        .ok_or(RequestError::Malformed)?;

    let mut table = FeatureTable::default();
    table.insert(FeatureEntry {
        id: FeatureId::ROOT,
        index: 0,
        flags: 0,
        version: 0,
    });
    for index in 1..=count {
        let resp = dev.request(fs.index, 1, &[index])?;
        let p = resp.params();
        if p.len() < 4 {
            return Err(RequestError::Malformed);
        }
        table.insert(FeatureEntry {
            id: FeatureId(u16::from_be_bytes([p[0], p[1]])),
            index,
            flags: p[2],
            version: p[3],
        });
    }
    Ok(table)
}
