//! Which OS runs what (ADR 0010). The daemon, tray and native plugins are Linux-only for
//! now; `detentctl`'s direct device commands work everywhere. One place for the message
//! every Linux-only program prints elsewhere, so they all point at the same issue list.

/// Whether `detentd` (and so the client IPC socket) exists on this OS.
pub const DAEMON_SUPPORTED: bool = cfg!(target_os = "linux");

/// Open issues for one OS: label `platform:<os>` (`macos`, `windows`, as
/// `std::env::consts::OS` spells them).
pub fn issues_url(os: &str) -> String {
    format!(
        "https://github.com/Rfluid/detent/issues?q=is%3Aissue+is%3Aopen+label%3Aplatform%3A{os}"
    )
}

/// The OS's usual name (`macOS`, `Windows`), else the `std::env::consts::OS` spelling.
pub fn os_name(os: &str) -> &str {
    match os {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    }
}

/// `"<what> is not yet supported on macOS; see https://…label%3Aplatform%3Amacos"`.
pub fn unsupported(what: &str) -> String {
    let os = std::env::consts::OS;
    format!(
        "{what} is not yet supported on {}; see {}",
        os_name(os),
        issues_url(os)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_names_os_and_label() {
        assert_eq!(
            issues_url("windows"),
            "https://github.com/Rfluid/detent/issues?q=is%3Aissue+is%3Aopen+label%3Aplatform%3Awindows"
        );
        assert_eq!(os_name("macos"), "macOS");
        let m = unsupported("detentd");
        assert!(m.starts_with("detentd is not yet supported on "));
        assert!(m.ends_with(&format!("platform%3A{}", std::env::consts::OS)));
    }
}
