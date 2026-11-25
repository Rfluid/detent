use core::fmt;

/// Errors from parsing or building HID++ frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtoError {
    /// The buffer is empty or shorter than the header.
    TooShort { len: usize },
    /// The first byte is not a known HID++ report ID.
    UnknownReportId(u8),
    /// The buffer length does not match the report ID's fixed length.
    LengthMismatch {
        report_id: u8,
        expected: usize,
        got: usize,
    },
    /// Parameters do not fit in the chosen report.
    ParamsTooLong { max: usize, got: usize },
    /// Software IDs are 4 bits and 0 is reserved for notifications.
    InvalidSoftwareId(u8),
}

impl fmt::Display for ProtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { len } => write!(f, "frame too short ({len} bytes)"),
            Self::UnknownReportId(id) => write!(f, "unknown HID++ report id 0x{id:02X}"),
            Self::LengthMismatch {
                report_id,
                expected,
                got,
            } => write!(
                f,
                "report 0x{report_id:02X} must be {expected} bytes, got {got}"
            ),
            Self::ParamsTooLong { max, got } => {
                write!(f, "params too long: max {max} bytes, got {got}")
            }
            Self::InvalidSoftwareId(v) => write!(f, "invalid software id {v} (must be 1..=15)"),
        }
    }
}

impl core::error::Error for ProtoError {}
