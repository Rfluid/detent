use detent_hidpp_proto::{DeviceIndex, Frame, ReportId, SoftwareId};

use super::*;

const CH: u8 = 14;
const HI: u8 = 15;

/// A three-slot device: ChangeHost at 14, HostsInfo at 15 (as on the MX Master 4).
struct Fake {
    hosts_info: bool,
    caps: u8,
    current: u8,
    names: Vec<Vec<u8>>,
    paired: Vec<bool>,
    sent: Vec<(u8, u8, Vec<u8>)>,
}

impl Fake {
    fn new() -> Self {
        Self {
            hosts_info: true,
            caps: caps::READ_NAMES | caps::WRITE_NAMES,
            current: 0,
            names: vec![
                b"fedora".to_vec(),
                b"a-rather-long-macbook-name".to_vec(),
                Vec::new(),
            ],
            paired: vec![true, true, false],
            sent: Vec::new(),
        }
    }

    fn reply(params: &[u8]) -> Frame {
        Frame::request_with(
            ReportId::Long,
            DeviceIndex::DIRECT,
            0,
            0,
            SoftwareId::DETENT,
            params,
        )
        .unwrap()
    }
}

impl Requester for Fake {
    fn feature_index(&self, id: FeatureId) -> Option<u8> {
        match id {
            FeatureId::CHANGE_HOST => Some(CH),
            FeatureId::HOSTS_INFO if self.hosts_info => Some(HI),
            _ => None,
        }
    }

    fn request(&mut self, fi: u8, func: u8, p: &[u8]) -> Result<Frame, RequestError> {
        self.sent.push((fi, func, p.to_vec()));
        let r = match (fi, func) {
            (CH, 0) => vec![3, self.current, 0x01],
            (HI, 0) => vec![self.caps, 0, 3, self.current],
            (HI, 1) => {
                let s = p[0] as usize;
                vec![
                    p[0],
                    u8::from(self.paired[s]),
                    0,
                    0,
                    self.names[s].len() as u8,
                    32,
                ]
            }
            (HI, 3) => {
                let (s, off) = (p[0] as usize, p[1] as usize);
                let mut v = vec![p[0], p[1]];
                v.extend(self.names[s].iter().skip(off).take(NAME_CHUNK));
                v
            }
            (HI, 4) => {
                let (s, off) = (p[0] as usize, p[1] as usize);
                let n = &mut self.names[s];
                n.truncate(off);
                n.extend_from_slice(&p[2..]);
                vec![p[0]]
            }
            _ => return Err(RequestError::Device(0x07)),
        };
        Ok(Self::reply(&r))
    }

    fn send(&mut self, fi: u8, func: u8, p: &[u8]) -> Result<(), RequestError> {
        self.sent.push((fi, func, p.to_vec()));
        Ok(())
    }
}

#[test]
fn reads_slots_with_names() {
    let mut d = Fake::new();
    let st = read(&mut d).unwrap();
    assert_eq!(
        (st.count, st.current, st.enhanced, st.can_rename),
        (3, 0, true, true)
    );
    let names: Vec<Option<&str>> = st.slots.iter().map(|s| s.name.as_deref()).collect();
    assert_eq!(
        names,
        [Some("fedora"), Some("a-rather-long-macbook-name"), None]
    );
    assert_eq!(st.slots[2].paired, Some(false));
    assert_eq!(st.slots[0].name_max, Some(32));
    // the 25-byte name took two 14-byte reads, at offsets 0 and 14
    let reads: Vec<&[u8]> = d
        .sent
        .iter()
        .filter(|(f, n, p)| *f == HI && *n == 3 && p[0] == 1)
        .map(|(_, _, p)| p.as_slice())
        .collect();
    assert_eq!(reads, [&[1, 0][..], &[1, 14][..]]);
}

#[test]
fn without_hosts_info_slots_have_no_names() {
    let mut d = Fake::new();
    d.hosts_info = false;
    let st = read(&mut d).unwrap();
    assert_eq!(st.slots.len(), 3);
    assert!(
        st.slots
            .iter()
            .all(|s| s.name.is_none() && s.paired.is_none())
    );
    assert!(!st.can_rename);
}

#[test]
fn unreadable_names_are_left_out() {
    let mut d = Fake::new();
    d.caps = 0;
    let st = read(&mut d).unwrap();
    assert!(st.slots.iter().all(|s| s.name.is_none()));
    assert_eq!(st.slots[1].paired, Some(true));
    assert!(!st.can_rename);
}

#[test]
fn targets_resolve_and_wrap() {
    assert_eq!(Target::parse("2"), Some(Target::Slot(1)));
    assert_eq!(Target::parse(" Next "), Some(Target::Next));
    assert_eq!(Target::parse("previous"), Some(Target::Prev));
    assert_eq!(Target::parse("0"), None);
    assert_eq!(Target::parse("x"), None);
    assert_eq!(Target::Next.resolve(3, 2), Some(0));
    assert_eq!(Target::Prev.resolve(3, 0), Some(2));
    assert_eq!(Target::Slot(3).resolve(3, 0), None);
    assert_eq!(Target::Next.resolve(0, 0), None);
}

#[test]
fn switch_is_fire_and_forget_and_skips_current() {
    let mut d = Fake::new();
    assert_eq!(switch(&mut d, Target::Slot(0)).unwrap(), None);
    assert!(!d.sent.iter().any(|(f, n, _)| *f == CH && *n == 1));
    assert_eq!(switch(&mut d, Target::Prev).unwrap(), Some(2));
    assert_eq!(d.sent.last(), Some(&(CH, 1, vec![2])));
    assert!(matches!(
        switch(&mut d, Target::Slot(5)),
        Err(RequestError::OutOfRange(_))
    ));
}

#[test]
fn rename_writes_chunks_and_reads_back() {
    let mut d = Fake::new();
    let st = rename(&mut d, 0, "workstation-at-the-office").unwrap();
    assert_eq!(
        st.slots[0].name.as_deref(),
        Some("workstation-at-the-office")
    );
    let writes: Vec<(u8, u8)> = d
        .sent
        .iter()
        .filter(|(f, n, _)| *f == HI && *n == 4)
        .map(|(_, _, p)| (p[0], p[1]))
        .collect();
    assert_eq!(writes, [(0, 0), (0, 14)]);
}

#[test]
fn rename_refuses_bad_names_and_read_only_devices() {
    let mut d = Fake::new();
    assert!(rename(&mut d, 0, "  ").is_err());
    assert!(rename(&mut d, 0, &"x".repeat(33)).is_err());
    assert!(rename(&mut d, 0, "tab\there").is_err());
    assert!(rename(&mut d, 7, "ok").is_err());
    d.caps = caps::READ_NAMES;
    assert!(rename(&mut d, 0, "ok").is_err());
    assert!(!d.sent.iter().any(|(f, n, _)| *f == HI && *n == 4));
}
