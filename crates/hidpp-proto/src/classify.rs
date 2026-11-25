//! First decision the data plane makes for every inbound frame.
//! Must stay branch-light and allocation-free.

use crate::{Frame, SoftwareId};

/// Feature index used by HID++ 2.0 error reports.
pub const ERROR_FEATURE_INDEX: u8 = 0xFF;
/// Sub-ID used by HID++ 1.0 error reports (receivers).
pub const HIDPP10_ERROR: u8 = 0x8F;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Reply to one of our requests (swid matches ours).
    Response,
    /// Unsolicited event from the device (swid 0): diverted buttons, raw XY, battery, ...
    Notification,
    /// Error reply to one of our requests.
    Error {
        feature_index: u8,
        function: u8,
        code: u8,
    },
    /// Traffic that belongs to someone else (e.g. the kernel driver). Ignore.
    Foreign,
}

pub fn classify(frame: &Frame, ours: SoftwareId) -> Kind {
    let feat = frame.feature_index();
    if feat == ERROR_FEATURE_INDEX || feat == HIDPP10_ERROR {
        let b = frame.as_bytes();
        // Error layout: [rid, dev, 0xFF, orig feature index, orig fn<<4|swid, error code, ...]
        let (orig_feat, orig_fn_swid, code) = (
            b.get(3).copied().unwrap_or(0),
            b.get(4).copied().unwrap_or(0),
            b.get(5).copied().unwrap_or(0),
        );
        return if orig_fn_swid & 0x0F == ours.get() {
            Kind::Error {
                feature_index: orig_feat,
                function: orig_fn_swid >> 4,
                code,
            }
        } else {
            Kind::Foreign
        };
    }
    match frame.swid_raw() {
        0 => Kind::Notification,
        s if s == ours.get() => Kind::Response,
        _ => Kind::Foreign,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_all_kinds() {
        let ours = SoftwareId::DETENT;
        let resp = Frame::parse(&[0x10, 0xFF, 0x05, 0x1E, 1, 2, 3]).unwrap();
        let notif = Frame::parse(&[0x10, 0xFF, 0x05, 0x00, 0, 0xC3, 0]).unwrap();
        let kernel = Frame::parse(&[0x10, 0xFF, 0x05, 0x11, 0, 0, 0]).unwrap();
        let err = Frame::parse(&[0x10, 0xFF, 0xFF, 0x05, 0x3E, 0x02, 0]).unwrap();
        let foreign_err = Frame::parse(&[0x10, 0xFF, 0xFF, 0x05, 0x31, 0x02, 0]).unwrap();

        assert_eq!(classify(&resp, ours), Kind::Response);
        assert_eq!(classify(&notif, ours), Kind::Notification);
        assert_eq!(classify(&kernel, ours), Kind::Foreign);
        assert_eq!(
            classify(&err, ours),
            Kind::Error {
                feature_index: 0x05,
                function: 3,
                code: 0x02
            }
        );
        assert_eq!(classify(&foreign_err, ours), Kind::Foreign);
    }
}
