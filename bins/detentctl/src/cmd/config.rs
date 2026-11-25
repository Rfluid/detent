//! `config path|show|edit|check|reload`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use detent_ipc::method;
use detent_ipc::types::Status;
use serde::Serialize;
use serde_json::json;

use crate::cli::ConfigCmd;
use crate::ctx::Ctx;
use crate::exit::{self, Exit};

/// Seed for `config edit` when no file exists yet.
const EXAMPLE: &str = include_str!("../../../../config/detent.example.toml");

#[derive(Serialize)]
pub struct PathOut {
    pub path: String,
    pub exists: bool,
    /// `daemon` (the running daemon's file) or `default` (XDG default).
    pub source: &'static str,
}

/// The daemon's config path if it runs, else the XDG default.
fn config_path(ctx: &Ctx) -> Result<(PathBuf, &'static str)> {
    if let Some(mut d) = ctx.try_daemon()? {
        let s: Status = d.call_as(method::STATUS, json!({}))?;
        return Ok((PathBuf::from(s.config_path), "daemon"));
    }
    Ok((detent_config::default_path(), "default"))
}

#[derive(Serialize)]
pub struct CheckOut {
    pub path: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub bindings: usize,
    pub profiles: usize,
    pub plugins: usize,
}

pub fn check_text(path: &Path, text: &str) -> CheckOut {
    let path = path.display().to_string();
    match detent_config::load(text) {
        Ok(c) => CheckOut {
            path,
            ok: true,
            error: None,
            bindings: detent_config::listing::bindings(&c.source).len(),
            profiles: c.profiles.len(),
            plugins: c.source.plugins.len(),
        },
        Err(e) => CheckOut {
            path,
            ok: false,
            error: Some(e.to_string()),
            bindings: 0,
            profiles: 0,
            plugins: 0,
        },
    }
}

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Exit::new(
                exit::NOINPUT,
                format!(
                    "no config file at {} (Detent runs without bindings; `detentctl config edit` creates one)",
                    path.display()
                ),
            )
            .into()
        } else {
            anyhow::Error::from(e).context(path.display().to_string())
        }
    })
}

fn check(ctx: &Ctx, path: &Path) -> Result<()> {
    let out = check_text(path, &read(path)?);
    ctx.emit(&out, |st| {
        if out.ok {
            println!(
                "{}: {} ({} bindings, {} profiles, {} plugins)",
                out.path,
                st.green("ok"),
                out.bindings,
                out.profiles,
                out.plugins
            );
        }
    })?;
    match out.error {
        Some(e) => Err(Exit::new(exit::CONFIG, format!("{}: {e}", out.path)).into()),
        None => Ok(()),
    }
}

fn edit(ctx: &Ctx, path: &Path) -> Result<()> {
    if !path.exists() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| dir.display().to_string())?;
        }
        std::fs::write(path, EXAMPLE).with_context(|| path.display().to_string())?;
        ctx.done(format!(
            "created {} from the example config",
            path.display()
        ));
    }
    let editor = ["VISUAL", "EDITOR"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.trim().is_empty()))
        .unwrap_or_else(|| "vi".into());
    // $EDITOR may carry arguments ("code --wait"): run through the shell, like git does.
    let st = Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("sh")
        .arg(path)
        .status()
        .with_context(|| format!("running {editor}"))?;
    if !st.success() {
        return Err(Exit::new(exit::FAILURE, format!("{editor} exited with {st}")).into());
    }
    check(ctx, path)?;
    ctx.done("saved; detentd applies it automatically");
    Ok(())
}

pub fn run(ctx: &Ctx, cmd: ConfigCmd) -> Result<()> {
    match cmd {
        ConfigCmd::Path => {
            let (p, source) = config_path(ctx)?;
            let out = PathOut {
                path: p.display().to_string(),
                exists: p.exists(),
                source,
            };
            ctx.emit(&out, |_| println!("{}", out.path))
        }
        ConfigCmd::Show => {
            let (p, _) = config_path(ctx)?;
            let text = read(&p)?;
            if ctx.json {
                let v: toml::Table = toml::from_str(&text)
                    .map_err(|e| Exit::new(exit::CONFIG, format!("{}: {e}", p.display())))?;
                return ctx.emit(&v, |_| {});
            }
            print!("{text}");
            Ok(())
        }
        ConfigCmd::Edit => {
            let (p, _) = config_path(ctx)?;
            edit(ctx, &p)
        }
        ConfigCmd::Check { file } => {
            let p = match file {
                Some(f) => f,
                None => config_path(ctx)?.0,
            };
            check(ctx, &p)
        }
        ConfigCmd::Reload => {
            let mut d = ctx.daemon()?;
            d.call(method::CONFIG_RELOAD, json!({}))?;
            if ctx.json {
                return ctx.emit(&json!({"ok": true}), |_| {});
            }
            ctx.done("config reloaded");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_reports_counts_and_errors() {
        let ok = check_text(Path::new("/x.toml"), EXAMPLE);
        assert!(ok.ok && ok.bindings > 0 && ok.profiles == 1);
        let v = serde_json::to_value(&ok).unwrap();
        assert!(v.get("error").is_none());

        let bad = check_text(
            Path::new("/x.toml"),
            "[[bind]]\nwhen = \"pinky\"\ndo = { keys = \"a\" }\n",
        );
        assert!(!bad.ok);
        assert!(bad.error.unwrap().contains("unknown control"));
    }
}
