//! Exit codes (sysexits.h, as systemctl-adjacent tools and BSD utilities use) and the
//! mapping from errors to codes and hints. Documented in docs/user/cli.md ("Exit codes").

use detent_ipc::{ClientError, code};

pub const OK: u8 = 0;
/// Generic failure (an external command failed, an action was refused, ...).
pub const FAILURE: u8 = 1;
/// Bad command line. clap exits with it itself; kept here so the table is complete
/// (a test pins clap's value).
#[allow(dead_code)] // reason: documentation of clap's exit code, asserted in tests
pub const USAGE: u8 = 2;
/// A value is invalid (setting out of range, bad JSON params).
pub const DATAERR: u8 = 65;
/// Not found: no such device, config file or path.
pub const NOINPUT: u8 = 66;
/// detentd is not running (or not supported on this OS yet), or a feature is unavailable
/// (update checks).
pub const UNAVAILABLE: u8 = 69;
/// I/O error talking to the daemon or the device.
pub const IOERR: u8 = 74;
/// The daemon or device answered something unexpected (version mismatch?).
pub const PROTOCOL: u8 = 76;
/// Permission denied (udev rules missing).
pub const NOPERM: u8 = 77;
/// The config file is invalid.
pub const CONFIG: u8 = 78;

/// An error that carries its exit code.
#[derive(Debug)]
pub struct Exit {
    pub code: u8,
    pub msg: String,
}

impl Exit {
    pub fn new(code: u8, msg: impl Into<String>) -> Self {
        Self {
            code,
            msg: msg.into(),
        }
    }
}

impl std::fmt::Display for Exit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for Exit {}

fn rpc_code(c: i32) -> u8 {
    match c {
        code::INVALID_PARAMS => DATAERR,
        code::METHOD_NOT_FOUND => PROTOCOL,
        code::DEVICE => NOINPUT,
        code::HIDPP => IOERR,
        code::CONFIG => CONFIG,
        _ => FAILURE,
    }
}

/// The exit code for an error: the first cause that says something specific wins.
pub fn code_for(e: &anyhow::Error) -> u8 {
    for cause in e.chain() {
        if let Some(x) = cause.downcast_ref::<Exit>() {
            return x.code;
        }
        if let Some(c) = cause.downcast_ref::<ClientError>() {
            return match c {
                ClientError::NotRunning(_) | ClientError::Unsupported(_) => UNAVAILABLE,
                ClientError::Io(io) if io.kind() == std::io::ErrorKind::PermissionDenied => NOPERM,
                ClientError::Io(_) => IOERR,
                ClientError::Protocol(_) => PROTOCOL,
                ClientError::Rpc(r) => rpc_code(r.code),
            };
        }
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            return match io.kind() {
                std::io::ErrorKind::PermissionDenied => NOPERM,
                std::io::ErrorKind::NotFound => NOINPUT,
                _ => IOERR,
            };
        }
    }
    FAILURE
}

/// A next step to print under the error, if there is an obvious one.
pub fn hint(e: &anyhow::Error) -> Option<&'static str> {
    e.chain().find_map(|cause| match cause.downcast_ref::<ClientError>()? {
        ClientError::NotRunning(_) => Some(
            "start it with `detentctl daemon start` (systemd user service) or run `detentd` in a terminal",
        ),
        ClientError::Rpc(r) if r.code == code::METHOD_NOT_FOUND => Some(
            "the running detentd is older than this detentctl: `detentctl daemon restart`",
        ),
        ClientError::Rpc(r) if r.code == code::DEVICE => {
            Some("check the mouse is on and connected: `detentctl devices`")
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use detent_ipc::RpcError;

    use super::*;

    fn rpc(c: i32) -> anyhow::Error {
        ClientError::Rpc(RpcError {
            code: c,
            message: "x".into(),
        })
        .into()
    }

    #[test]
    fn codes() {
        assert_eq!(
            code_for(&ClientError::NotRunning("/s".into()).into()),
            UNAVAILABLE
        );
        assert_eq!(
            code_for(&ClientError::Unsupported("x".into()).into()),
            UNAVAILABLE
        );
        assert_eq!(code_for(&rpc(code::INVALID_PARAMS)), DATAERR);
        assert_eq!(code_for(&rpc(code::CONFIG)), CONFIG);
        assert_eq!(code_for(&rpc(code::DEVICE)), NOINPUT);
        assert_eq!(code_for(&rpc(4242)), FAILURE);
        assert_eq!(code_for(&anyhow::anyhow!("plain")), FAILURE);
        let e = anyhow::Error::from(Exit::new(CONFIG, "bad")).context("while checking");
        assert_eq!(code_for(&e), CONFIG, "codes survive context");
        let io = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert_eq!(code_for(&io.into()), NOPERM);
    }

    #[test]
    fn clap_usage_code_matches() {
        use clap::Parser;
        let e = crate::cli::Cli::try_parse_from(["detentctl", "bogus"]).unwrap_err();
        assert_eq!(e.exit_code(), i32::from(USAGE));
    }

    #[test]
    fn hints() {
        assert!(
            hint(&ClientError::NotRunning("/s".into()).into())
                .is_some_and(|h| h.contains("daemon start"))
        );
        assert!(hint(&rpc(code::METHOD_NOT_FOUND)).is_some_and(|h| h.contains("restart")));
        assert!(hint(&anyhow::anyhow!("plain")).is_none());
    }
}
