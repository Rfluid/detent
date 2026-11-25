//! `exec` actions: run through `sh -c` in their own process group, detached from the
//! daemon's stdio; a short-lived thread reaps each child. Never on the data plane.

use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use tracing::{debug, warn};

pub fn spawn(cmd: &str) {
    let child = Command::new("/bin/sh")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
    match child {
        Ok(mut c) => {
            debug!("exec `{cmd}` (pid {})", c.id());
            let cmd = cmd.to_string();
            let _ = std::thread::Builder::new()
                .name("detent-reap".into())
                .spawn(move || match c.wait() {
                    Ok(s) if !s.success() => warn!("`{cmd}` exited with {s}"),
                    Ok(_) => {}
                    Err(e) => warn!("waiting for `{cmd}`: {e}"),
                });
        }
        Err(e) => warn!("could not run `{cmd}`: {e}"),
    }
}
