use detent_hidpp_proto::FeatureId;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FeatureEntry {
    #[serde(serialize_with = "ser_id")]
    pub id: FeatureId,
    pub index: u8,
    /// Bit 7 obsolete, bit 6 hidden, bit 5 engineering.
    pub flags: u8,
    pub version: u8,
}

fn ser_id<S: serde::Serializer>(id: &FeatureId, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&format!("0x{:04X}", id.0))
}

/// The device's feature map. Small (≈30 entries): a sorted Vec beats a HashMap.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FeatureTable {
    entries: Vec<FeatureEntry>,
}

impl FeatureTable {
    pub fn insert(&mut self, entry: FeatureEntry) {
        match self.entries.binary_search_by_key(&entry.id, |e| e.id) {
            Ok(i) => self.entries[i] = entry,
            Err(i) => self.entries.insert(i, entry),
        }
    }

    pub fn index_of(&self, id: FeatureId) -> Option<u8> {
        self.entries
            .binary_search_by_key(&id, |e| e.id)
            .ok()
            .map(|i| self.entries[i].index)
    }

    pub fn iter(&self) -> impl Iterator<Item = &FeatureEntry> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup() {
        let mut t = FeatureTable::default();
        t.insert(FeatureEntry {
            id: FeatureId::REPROG_CONTROLS_V4,
            index: 9,
            flags: 0,
            version: 5,
        });
        t.insert(FeatureEntry {
            id: FeatureId::ROOT,
            index: 0,
            flags: 0,
            version: 0,
        });
        assert_eq!(t.index_of(FeatureId::REPROG_CONTROLS_V4), Some(9));
        assert_eq!(t.index_of(FeatureId::THUMBWHEEL), None);
        assert_eq!(t.len(), 2);
    }
}
