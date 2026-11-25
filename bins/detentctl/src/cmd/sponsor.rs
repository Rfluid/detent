//! `sponsor [--open]`: where to support Detent. Works on every OS and needs no daemon.

use std::process::{Command, Stdio};

use anyhow::Result;
use detent_update::sponsor::{PIX_URL, SPONSOR_DOC_URL, SPONSOR_URL};
use serde::Serialize;

use crate::ctx::Ctx;
use crate::exit::{self, Exit};

/// `sponsor --json`.
#[derive(Serialize)]
pub struct SponsorOut {
    pub github_sponsors: &'static str,
    pub pix: &'static str,
    /// SPONSOR.md: every option, including Bitcoin and Lightning.
    pub more: &'static str,
    /// Whether `--open` launched the browser (`false` without `--open`).
    pub opened: bool,
}

/// The command that opens `url` in the default browser on this OS: `open` on macOS,
/// `cmd /C start "" URL` on Windows (`start` is a cmd built-in; `""` is its window
/// title), `xdg-open` elsewhere.
fn opener(url: &str) -> Command {
    if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(url);
        c
    } else if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "", url]);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    }
}

fn open(url: &str) -> Result<()> {
    let mut c = opener(url);
    let prog = c.get_program().to_string_lossy().into_owned();
    let status = c
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(Exit::new(exit::FAILURE, format!("{prog} {url}: {s}")).into()),
        Err(e) => Err(Exit::new(exit::FAILURE, format!("{prog} {url}: {e}")).into()),
    }
}

pub fn run(ctx: &Ctx, open_browser: bool) -> Result<()> {
    let mut out = SponsorOut {
        github_sponsors: SPONSOR_URL,
        pix: PIX_URL,
        more: SPONSOR_DOC_URL,
        opened: false,
    };
    // Print the links first, so they're there even if no browser opens.
    if !ctx.json {
        let st = &ctx.style;
        println!("Detent is free and built by one person. Thank you for considering it!");
        println!("  {}  {SPONSOR_URL}", st.bold("GitHub Sponsors"));
        println!("  {}        {PIX_URL}", st.bold("Pix (BRL)"));
        println!("Bitcoin, Lightning and other ways: SPONSOR.md ({SPONSOR_DOC_URL})");
    }
    if open_browser {
        open(SPONSOR_URL)?;
        out.opened = true;
        ctx.done(format_args!("Opened {SPONSOR_URL}"));
    }
    if ctx.json {
        ctx.emit(&out, |_| {})?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opener_per_os() {
        let c = opener("https://x");
        let argv: Vec<String> = std::iter::once(c.get_program())
            .chain(c.get_args())
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        let want: &[&str] = if cfg!(target_os = "macos") {
            &["open", "https://x"]
        } else if cfg!(windows) {
            &["cmd", "/C", "start", "", "https://x"]
        } else {
            &["xdg-open", "https://x"]
        };
        assert_eq!(argv, want);
    }

    #[test]
    fn json_shape() {
        let v = serde_json::to_value(SponsorOut {
            github_sponsors: SPONSOR_URL,
            pix: PIX_URL,
            more: SPONSOR_DOC_URL,
            opened: false,
        })
        .unwrap();
        assert_eq!(v["github_sponsors"], "https://github.com/sponsors/Rfluid");
        assert_eq!(v["pix"], "https://livepix.gg/rfluid");
        assert!(v["more"].as_str().unwrap().ends_with("/SPONSOR.md"));
        assert_eq!(v["opened"], false);
    }
}
