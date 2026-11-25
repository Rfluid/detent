//! HID++ frame layout:
//!
//! ```text
//! byte 0     report id      0x10 short (7 B) | 0x11 long (20 B) | 0x12 very long (64 B)
//! byte 1     device index   0xFF = direct (USB/Bluetooth), 1..=6 = Bolt/Unifying slot
//! byte 2     feature index  0xFF = error report
//! byte 3     fn << 4 | swid swid 0 = notification
//! byte 4..   params
//! ```

use crate::ProtoError;

pub const MAX_FRAME_LEN: usize = 64;
const HEADER_LEN: usize = 4;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportId {
    Short = 0x10,
    Long = 0x11,
    VeryLong = 0x12,
}

impl ReportId {
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x10 => Some(Self::Short),
            0x11 => Some(Self::Long),
            0x12 => Some(Self::VeryLong),
            _ => None,
        }
    }

    /// Total frame length on the wire, header included.
    pub const fn frame_len(self) -> usize {
        match self {
            Self::Short => 7,
            Self::Long => 20,
            Self::VeryLong => 64,
        }
    }

    pub const fn max_params(self) -> usize {
        self.frame_len() - HEADER_LEN
    }

    /// Smallest report that fits `n` parameter bytes.
    pub const fn for_params(n: usize) -> Option<Self> {
        if n <= Self::Short.max_params() {
            Some(Self::Short)
        } else if n <= Self::Long.max_params() {
            Some(Self::Long)
        } else if n <= Self::VeryLong.max_params() {
            Some(Self::VeryLong)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceIndex(pub u8);

impl DeviceIndex {
    /// Directly connected device (USB cable or Bluetooth). The MX Master 4 over BT uses this.
    pub const DIRECT: Self = Self(0xFF);
}

/// 4-bit software ID used to match responses to requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoftwareId(u8);

impl SoftwareId {
    /// The Linux kernel's `hid-logitech-hidpp` driver uses swid 1.
    pub const KERNEL: Self = Self(0x1);
    /// Detent's swid. Must never equal [`Self::KERNEL`].
    pub const DETENT: Self = Self(0xE);

    pub const fn new(v: u8) -> Result<Self, ProtoError> {
        if v == 0 || v > 0x0F {
            Err(ProtoError::InvalidSoftwareId(v))
        } else {
            Ok(Self(v))
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// A HID++ frame. Fixed-size, `Copy`, stack-allocated.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    buf: [u8; MAX_FRAME_LEN],
    len: u8,
}

impl Frame {
    /// Parse a frame read from hidraw. Validates report ID and length.
    pub fn parse(bytes: &[u8]) -> Result<Self, ProtoError> {
        if bytes.len() < HEADER_LEN {
            return Err(ProtoError::TooShort { len: bytes.len() });
        }
        let rid = ReportId::from_u8(bytes[0]).ok_or(ProtoError::UnknownReportId(bytes[0]))?;
        if bytes.len() != rid.frame_len() {
            return Err(ProtoError::LengthMismatch {
                report_id: bytes[0],
                expected: rid.frame_len(),
                got: bytes.len(),
            });
        }
        let mut buf = [0u8; MAX_FRAME_LEN];
        buf[..bytes.len()].copy_from_slice(bytes);
        Ok(Self {
            buf,
            len: bytes.len() as u8,
        })
    }

    /// Build a request. Picks the smallest report that fits `params`.
    pub fn request(
        device: DeviceIndex,
        feature_index: u8,
        function: u8,
        swid: SoftwareId,
        params: &[u8],
    ) -> Result<Self, ProtoError> {
        let rid = ReportId::for_params(params.len()).ok_or(ProtoError::ParamsTooLong {
            max: ReportId::VeryLong.max_params(),
            got: params.len(),
        })?;
        Self::request_with(rid, device, feature_index, function, swid, params)
    }

    /// Build a request with an explicit report ID (some devices only accept long reports).
    pub fn request_with(
        rid: ReportId,
        device: DeviceIndex,
        feature_index: u8,
        function: u8,
        swid: SoftwareId,
        params: &[u8],
    ) -> Result<Self, ProtoError> {
        if params.len() > rid.max_params() {
            return Err(ProtoError::ParamsTooLong {
                max: rid.max_params(),
                got: params.len(),
            });
        }
        let mut buf = [0u8; MAX_FRAME_LEN];
        buf[0] = rid as u8;
        buf[1] = device.0;
        buf[2] = feature_index;
        buf[3] = (function << 4) | swid.get();
        buf[HEADER_LEN..HEADER_LEN + params.len()].copy_from_slice(params);
        Ok(Self {
            buf,
            len: rid.frame_len() as u8,
        })
    }

    pub fn report_id(&self) -> ReportId {
        // Invariant: constructors only accept valid report IDs.
        ReportId::from_u8(self.buf[0]).unwrap_or(ReportId::Short)
    }

    pub fn device_index(&self) -> DeviceIndex {
        DeviceIndex(self.buf[1])
    }

    pub fn feature_index(&self) -> u8 {
        self.buf[2]
    }

    pub fn function(&self) -> u8 {
        self.buf[3] >> 4
    }

    /// Raw 4-bit software ID (0 means notification).
    pub fn swid_raw(&self) -> u8 {
        self.buf[3] & 0x0F
    }

    pub fn params(&self) -> &[u8] {
        &self.buf[HEADER_LEN..self.len as usize]
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len as usize]
    }
}

impl core::fmt::Debug for Frame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Frame {{ rid: {:?}, dev: 0x{:02X}, feat: 0x{:02X}, fn: {}, swid: {}, params: {:02X?} }}",
            self.report_id(),
            self.device_index().0,
            self.feature_index(),
            self.function(),
            self.swid_raw(),
            self.params()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrip() {
        let f = Frame::request(
            DeviceIndex::DIRECT,
            0x0B,
            0x3,
            SoftwareId::DETENT,
            &[0x00, 0xC3],
        )
        .unwrap();
        assert_eq!(f.report_id(), ReportId::Short);
        assert_eq!(f.as_bytes(), &[0x10, 0xFF, 0x0B, 0x3E, 0x00, 0xC3, 0x00]);
        let parsed = Frame::parse(f.as_bytes()).unwrap();
        assert_eq!(parsed, f);
        assert_eq!(parsed.function(), 3);
        assert_eq!(parsed.swid_raw(), 0xE);
    }

    #[test]
    fn picks_long_report_when_needed() {
        let f = Frame::request(DeviceIndex::DIRECT, 1, 0, SoftwareId::DETENT, &[0; 10]).unwrap();
        assert_eq!(f.report_id(), ReportId::Long);
        assert_eq!(f.as_bytes().len(), 20);
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(Frame::parse(&[0x10]), Err(ProtoError::TooShort { len: 1 }));
        assert_eq!(
            Frame::parse(&[0x20, 0, 0, 0]),
            Err(ProtoError::UnknownReportId(0x20))
        );
        assert!(matches!(
            Frame::parse(&[0x11, 0xFF, 0, 0, 0, 0, 0]),
            Err(ProtoError::LengthMismatch { .. })
        ));
        assert!(SoftwareId::new(0).is_err());
        assert!(SoftwareId::new(16).is_err());
        assert_ne!(SoftwareId::DETENT, SoftwareId::KERNEL);
    }
}
