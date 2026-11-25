//! Build info for `detentctl version` / `--version`: git commit, target, profile.
//! Everything is optional: a source tarball without git builds fine ("unknown").

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn main() {
    let commit = git(&["rev-parse", "--short=10", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some();
    println!(
        "cargo:rustc-env=DETENT_GIT_COMMIT={commit}{}",
        if dirty { "-dirty" } else { "" }
    );
    for (k, env) in [("DETENT_TARGET", "TARGET"), ("DETENT_PROFILE", "PROFILE")] {
        let v = std::env::var(env).unwrap_or_else(|_| "unknown".into());
        println!("cargo:rustc-env={k}={v}");
    }
    // Rebuild when HEAD moves (works in worktrees: --git-dir points at the worktree's dir).
    if let Some(dir) = git(&["rev-parse", "--git-dir"]) {
        println!("cargo:rerun-if-changed={dir}/HEAD");
        if let Some(r) = git(&["symbolic-ref", "-q", "HEAD"])
            && let Some(common) = git(&["rev-parse", "--git-common-dir"])
        {
            println!("cargo:rerun-if-changed={common}/{r}");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
