//! `completions <shell>` (clap_complete) and the hidden `man` (clap_mangen). Generated at
//! runtime from the same clap tree, so they can't drift from the binary; `just man` and
//! `just completions` wrap them (decision recorded in docs/user/cli.md).

use std::path::Path;

use anyhow::{Context, Result};
use clap::CommandFactory;

use crate::cli::Cli;
use crate::ctx::Ctx;

pub fn completions(shell: clap_complete::Shell) -> Result<()> {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "detentctl", &mut std::io::stdout());
    Ok(())
}

pub fn man(ctx: &Ctx, out_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(out_dir).with_context(|| out_dir.display().to_string())?;
    clap_mangen::generate_to(Cli::command(), out_dir)
        .with_context(|| format!("writing man pages to {}", out_dir.display()))?;
    ctx.done(format!("man pages written to {}", out_dir.display()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_completions_and_man_pages() {
        let mut buf = Vec::new();
        clap_complete::generate(
            clap_complete::Shell::Bash,
            &mut Cli::command(),
            "detentctl",
            &mut buf,
        );
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("settings") && s.contains("--json"));

        let dir = std::env::temp_dir().join(format!("detentctl-man-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        clap_mangen::generate_to(Cli::command(), &dir).unwrap();
        assert!(dir.join("detentctl.1").exists());
        assert!(dir.join("detentctl-settings-set.1").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
