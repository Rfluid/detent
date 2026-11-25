//! Release asset names: which archive of a release belongs to this machine.
//!
//! Contract: docs/dev/release.md ("Release assets"). `detent-<tag>-<target>.tar.gz` on
//! Linux and macOS, `detent-<tag>-<target>.zip` on Windows, each with a `.sha256` sibling
//! in `sha256sum` format. The remote installers (`install.sh`, `scripts/install.ps1`)
//! build the same names; keep the three in step.

/// The Rust target triples a release is built for (the release.yml matrix).
pub const TARGETS: [&str; 5] = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
];

/// `std::env::consts::{OS, ARCH}` → the release target built for it, if any.
pub fn target_for(os: &str, arch: &str) -> Option<&'static str> {
    Some(match (os, arch) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => return None,
    })
}

/// The release target of this build's platform. `None`: no release is built for it
/// (build from source).
pub fn host_target() -> Option<&'static str> {
    target_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// Archive extension for a target: `zip` on Windows, `tar.gz` elsewhere.
pub fn archive_ext(target: &str) -> &'static str {
    if target.contains("-windows-") {
        "zip"
    } else {
        "tar.gz"
    }
}

/// `detent-v0.1.0-x86_64-unknown-linux-gnu.tar.gz`. `tag` keeps its `v`.
pub fn asset_name(tag: &str, target: &str) -> String {
    format!("detent-{tag}-{target}.{}", archive_ext(target))
}

/// The checksum file next to [`asset_name`]: the same name plus `.sha256`.
pub fn checksum_name(tag: &str, target: &str) -> String {
    format!("{}.sha256", asset_name(tag, target))
}

/// One file attached to a release.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Asset {
    pub name: String,
    /// `browser_download_url`: a plain HTTPS download, no API token needed.
    #[serde(rename = "browser_download_url")]
    pub url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_matrix_target_maps_from_os_and_arch() {
        let pairs = [
            ("linux", "x86_64"),
            ("linux", "aarch64"),
            ("macos", "x86_64"),
            ("macos", "aarch64"),
            ("windows", "x86_64"),
        ];
        let mapped: Vec<_> = pairs
            .iter()
            .map(|(os, arch)| target_for(os, arch).unwrap())
            .collect();
        assert_eq!(mapped, TARGETS);
        assert_eq!(target_for("windows", "aarch64"), None, "not built yet");
        assert_eq!(target_for("freebsd", "x86_64"), None);
        assert_eq!(target_for("linux", "riscv64"), None);
    }

    #[test]
    fn asset_names_follow_the_contract() {
        let names: Vec<_> = TARGETS.iter().map(|t| asset_name("v0.1.0", t)).collect();
        assert_eq!(
            names,
            [
                "detent-v0.1.0-x86_64-unknown-linux-gnu.tar.gz",
                "detent-v0.1.0-aarch64-unknown-linux-gnu.tar.gz",
                "detent-v0.1.0-x86_64-apple-darwin.tar.gz",
                "detent-v0.1.0-aarch64-apple-darwin.tar.gz",
                "detent-v0.1.0-x86_64-pc-windows-msvc.zip",
            ]
        );
        assert_eq!(
            checksum_name("v0.1.0", "x86_64-pc-windows-msvc"),
            "detent-v0.1.0-x86_64-pc-windows-msvc.zip.sha256"
        );
        assert_eq!(
            checksum_name("v1.0.0-rc1", "aarch64-apple-darwin"),
            "detent-v1.0.0-rc1-aarch64-apple-darwin.tar.gz.sha256"
        );
    }

    #[test]
    fn this_build_has_a_target_on_release_platforms() {
        let expected = cfg!(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(
                target_os = "macos",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "windows", target_arch = "x86_64")
        ));
        assert_eq!(host_target().is_some(), expected);
    }
}
