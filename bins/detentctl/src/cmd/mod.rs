//! One module per command group. Each gathers data (daemon, device or file), then hands
//! it to `Ctx::emit` with a printer from `output::human`.

pub mod config;
pub mod daemon;
pub mod devices;
pub mod events;
pub mod generate;
pub mod plugins;
pub mod settings;
pub mod sponsor;
pub mod status;

use anyhow::Result;

use crate::cli::{ActionsCmd, Cli, Cmd, ConfigCmd, DebugCmd, DevicesCmd, PluginsCmd, SettingsCmd};
use crate::ctx::Ctx;
use crate::direct;

pub fn run(cli: Cli) -> Result<()> {
    let ctx = Ctx::new(&cli.global);
    let sel = ctx.device.clone();
    let sel = sel.as_deref();
    match cli.cmd {
        Cmd::Status => status::status(&ctx),
        Cmd::Enable => status::remap(&ctx, Some(true)),
        Cmd::Disable => status::remap(&ctx, Some(false)),
        Cmd::Toggle => status::remap(&ctx, None),
        Cmd::Version => status::version(&ctx),
        Cmd::Update(_) => status::update_check(&ctx),
        Cmd::Sponsor(a) => sponsor::run(&ctx, a.open),
        Cmd::Daemon(c) => daemon::run(&ctx, c),
        Cmd::Logs(a) => daemon::logs(&ctx, &a),
        Cmd::Config(c) => config::run(&ctx, c),
        Cmd::Check { file } => config::run(&ctx, ConfigCmd::Check { file }),
        Cmd::Reload => config::run(&ctx, ConfigCmd::Reload),
        Cmd::Devices { cmd } => devices::run(&ctx, cmd.unwrap_or(DevicesCmd::List)),
        Cmd::Haptic(a) => devices::haptic(&ctx, &a),
        Cmd::Settings { cmd } => settings::run(&ctx, cmd.unwrap_or(SettingsCmd::List)),
        Cmd::Get { path: Some(p) } => settings::run(&ctx, SettingsCmd::Get { path: p }),
        Cmd::Get { path: None } => devices::run(&ctx, DevicesCmd::Info),
        Cmd::Set(a) => settings::run(&ctx, SettingsCmd::Set(a)),
        Cmd::Bindings(a) => plugins::bindings(&ctx, &a),
        Cmd::Actions { cmd } => plugins::actions(&ctx, cmd.unwrap_or(ActionsCmd::List)),
        Cmd::Invoke(a) => plugins::actions(&ctx, ActionsCmd::Invoke(a)),
        Cmd::Plugins { cmd } => plugins::plugins(&ctx, cmd.unwrap_or(PluginsCmd::List)),
        Cmd::Events(a) => events::run(&ctx, &a),
        Cmd::Reset => direct::reset(sel, ctx.quiet || ctx.json),
        Cmd::Debug(DebugCmd::Features) | Cmd::DumpFeatures => direct::dump_features(sel, ctx.json),
        Cmd::Debug(DebugCmd::Controls) | Cmd::DumpControls => direct::dump_controls(sel, ctx.json),
        Cmd::Completions { shell } => generate::completions(shell),
        Cmd::Man { out_dir } => generate::man(&ctx, &out_dir),
    }
}
