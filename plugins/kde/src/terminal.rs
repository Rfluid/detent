//! `kde.terminal`: start the terminal chosen in System Settings → Default Applications.
//!
//! Plasma stores it in `kdeglobals` `[General]`: `TerminalService` (a desktop-file id, Plasma
//! 5.25+) and `TerminalApplication` (a command). Neither is written while the default
//! (Konsole) is kept, as on the owner's Plasma 6.8 (checked 2026-11-24), hence the fallback.
//! Spec: docs/plugins/kde.md.

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// What to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminal {
    /// A desktop-file id, launched with `gtk-launch` (like radial's `app = ...`).
    Service(String),
    /// A command line, run with `/bin/sh -c`.
    Command(String),
}

fn kdeglobals() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|d| d.join("kdeglobals"))
}

/// The terminal configured in a `kdeglobals` text, else Konsole.
pub fn from_kdeglobals(text: &str) -> Terminal {
    let mut in_general = false;
    let (mut service, mut command) = (None, None);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_general = line == "[General]";
            continue;
        }
        if !in_general {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim();
            if v.is_empty() {
                continue;
            }
            match k.trim() {
                "TerminalService" => service = Some(v.trim_end_matches(".desktop").to_string()),
                "TerminalApplication" => command = Some(v.to_string()),
                _ => {}
            }
        }
    }
    service
        .map(Terminal::Service)
        .or(command.map(Terminal::Command))
        .unwrap_or_else(|| Terminal::Command("konsole".into()))
}

pub fn configured() -> Terminal {
    let text = kdeglobals()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    from_kdeglobals(&text)
}

/// Start it in the home directory, detached from the plugin's stdio; reaped on a thread.
pub fn launch() {
    let term = configured();
    let mut cmd = match &term {
        Terminal::Service(id) => {
            let mut c = Command::new("gtk-launch");
            c.arg(id);
            c
        }
        Terminal::Command(line) => {
            let mut c = Command::new("/bin/sh");
            c.arg("-c").arg(line);
            c
        }
    };
    if let Some(home) = std::env::var_os("HOME") {
        cmd.current_dir(home);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match cmd.spawn() {
        Ok(mut child) => {
            let _ = std::thread::Builder::new()
                .name("kde-terminal".into())
                .spawn(move || child.wait());
        }
        Err(e) => eprintln!("detent-kde: cannot start the terminal ({term:?}): {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_default_terminal() {
        assert_eq!(
            from_kdeglobals("[KDE]\nLookAndFeelPackage=x\n"),
            Terminal::Command("konsole".into())
        );
        assert_eq!(
            from_kdeglobals("[General]\nTerminalApplication=kitty\n"),
            Terminal::Command("kitty".into())
        );
        assert_eq!(
            from_kdeglobals(
                "[General]\nTerminalApplication=kitty\nTerminalService=kitty.desktop\n[KDE]\nTerminalService=no\n"
            ),
            Terminal::Service("kitty".into())
        );
        // a key in another group doesn't count
        assert_eq!(
            from_kdeglobals("[Other]\nTerminalApplication=xterm\n"),
            Terminal::Command("konsole".into())
        );
    }
}
