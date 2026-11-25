//! The command tree (clap derive). Noun-verb: `detentctl <group> <verb> [args]`; a group
//! without a verb runs its read command (`devices` = `devices list`). Reference:
//! docs/user/cli.md. Hidden legacy spellings (`get`, `set`, `dump-features`, ...) keep
//! old scripts and muscle memory working.

use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};

pub const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (commit ",
    env!("DETENT_GIT_COMMIT"),
    ", ",
    env!("DETENT_TARGET"),
    ", ",
    env!("DETENT_PROFILE"),
    ")"
);

#[derive(Parser, Debug)]
#[command(
    name = "detentctl",
    version,
    long_version = LONG_VERSION,
    about = "Control the Detent mouse daemon (detentd)",
    long_about = "Control the Detent mouse daemon (detentd): device settings, remapping, \
                  config, plugins and live events.\n\nMost commands talk to the running \
                  daemon; `devices`, `settings get/list/set`, `config check`, `reset` and \
                  `debug` also work without it.",
    after_help = "Exit codes: 0 ok, 1 failure, 2 usage, 65 invalid value, 66 not found, \
                  69 daemon not running, 74 I/O, 76 protocol, 77 permission, 78 bad config.\n\
                  Reference: docs/user/cli.md"
)]
pub struct Cli {
    #[command(flatten)]
    pub global: Global,
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Args, Debug, Clone)]
pub struct Global {
    /// Machine-readable JSON on stdout (stable schema; events are NDJSON)
    #[arg(long, global = true)]
    pub json: bool,
    /// More detail; -vv also traces IPC requests on stderr
    #[arg(short, long, action = ArgAction::Count, global = true)]
    pub verbose: u8,
    /// Print only errors (and requested data)
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    pub quiet: bool,
    /// Daemon socket
    #[arg(long, global = true, env = "DETENT_SOCKET", value_name = "PATH")]
    pub socket: Option<PathBuf>,
    /// When to use colors
    #[arg(long, global = true, value_enum, default_value_t = ColorWhen::Auto, value_name = "WHEN")]
    pub color: ColorWhen,
    /// Device: hidraw node (`hidraw10`), device id or part of its name
    #[arg(short, long, global = true, value_name = "DEVICE")]
    pub device: Option<String>,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorWhen {
    Auto,
    Always,
    Never,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Daemon health: version, config, remapping, devices, plugins
    Status,
    /// Resume remapping (bindings, gestures) in the running daemon
    Enable,
    /// Pause remapping: the mouse acts as without Detent; settings and plugins keep running
    Disable,
    /// Pause or resume remapping
    Toggle,
    /// Start, stop or restart detentd (systemd user service)
    #[command(subcommand)]
    Daemon(DaemonCmd),
    /// Show the daemon log (journalctl --user -u detent.service)
    Logs(LogsArgs),
    /// Config file: path, show, edit, check, reload
    #[command(subcommand)]
    Config(ConfigCmd),
    /// Devices: list, info, battery
    Devices {
        #[command(subcommand)]
        cmd: Option<DevicesCmd>,
    },
    /// Device settings: list, get, set, unset (DPI, SmartShift, wheels, haptics)
    #[command(visible_alias = "setting")]
    Settings {
        #[command(subcommand)]
        cmd: Option<SettingsCmd>,
    },
    /// Show the bindings of the loaded config
    Bindings(BindingsArgs),
    /// Actions (built-in and from plugins): list, invoke
    Actions {
        #[command(subcommand)]
        cmd: Option<ActionsCmd>,
    },
    /// Plugins: list, status, restart
    Plugins {
        #[command(subcommand)]
        cmd: Option<PluginsCmd>,
    },
    /// Stream live events until Ctrl+C
    #[command(visible_alias = "monitor", alias = "watch")]
    Events(EventsArgs),
    /// Play a haptic waveform on the mouse
    Haptic(HapticArgs),
    /// Give the buttons back: undo diversion left by a crash or another tool (no daemon needed)
    Reset,
    /// Low-level device inspection over hidraw (no daemon needed)
    #[command(subcommand)]
    Debug(DebugCmd),
    /// Print a shell completion script
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Write man pages (detentctl.1 and one per subcommand) into a directory
    #[command(hide = true)]
    Man {
        #[arg(long, value_name = "DIR", default_value = "target/man")]
        out_dir: PathBuf,
    },
    /// Version and build info of detentctl and the running daemon
    Version,
    /// Release updates
    #[command(subcommand)]
    Update(UpdateCmd),
    /// Support Detent: GitHub Sponsors and Pix links (more in SPONSOR.md)
    Sponsor(SponsorArgs),

    // ── legacy spellings (hidden) ──────────────────────────────────────────────
    #[command(hide = true)]
    Get { path: Option<String> },
    #[command(hide = true)]
    Set(SetArgs),
    #[command(hide = true)]
    Check { file: Option<PathBuf> },
    #[command(hide = true)]
    Reload,
    #[command(hide = true, name = "dump-features")]
    DumpFeatures,
    #[command(hide = true, name = "dump-controls")]
    DumpControls,
    #[command(hide = true)]
    Invoke(InvokeArgs),
}

#[derive(Subcommand, Debug)]
pub enum DaemonCmd {
    /// Start the service and wait until the daemon answers
    Start(WaitArgs),
    /// Stop the service (the mouse's buttons are restored)
    Stop,
    /// Restart the service and wait until the daemon answers
    Restart(WaitArgs),
    /// Service state (systemctl --user status)
    Status,
}

#[derive(Args, Debug)]
pub struct WaitArgs {
    /// Return as soon as systemctl does
    #[arg(long)]
    pub no_wait: bool,
}

#[derive(Args, Debug)]
pub struct LogsArgs {
    /// Keep printing new lines
    #[arg(short, long)]
    pub follow: bool,
    /// Number of past lines
    #[arg(short = 'n', long, default_value_t = 50, value_name = "N")]
    pub lines: u32,
    /// Only since this time (journalctl syntax: "10 min ago", "today")
    #[arg(long, value_name = "TIME")]
    pub since: Option<String>,
    /// Only this plugin's lines (`radial`, `kde`), its restarts, and the compositor
    /// dropping a client (KWin's "error in client communication")
    #[arg(long, value_name = "NAME")]
    pub plugin: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
    /// Print the config file path
    Path,
    /// Print the config file (--json: parsed)
    Show,
    /// Open the config in $VISUAL/$EDITOR, then check it
    Edit,
    /// Validate a config file without applying it
    #[command(visible_alias = "validate")]
    Check {
        /// File to check (default: the daemon's config file)
        file: Option<PathBuf>,
    },
    /// Reload the config now (normally automatic on save)
    Reload,
}

#[derive(Subcommand, Debug)]
pub enum DevicesCmd {
    /// Logitech HID++ devices (hidraw) and whether the daemon has attached them
    List,
    /// Everything about one device: identity, battery, settings, diverted controls
    Info,
    /// Battery level and charging state
    Battery,
}

#[derive(Subcommand, Debug)]
pub enum SettingsCmd {
    /// Every setting this device supports, with value, range and pending changes
    List,
    /// One value: a setting (`dpi`) or any state path (`battery.percent`)
    Get { path: String },
    /// Change a setting live, e.g. `settings set dpi 1600 --persist`
    Set(SetArgs),
    /// Drop a live change: the config value applies again (needs the daemon)
    Unset { path: String },
}

#[derive(Args, Debug)]
pub struct SetArgs {
    /// Setting path (see `settings list`)
    pub path: String,
    pub value: String,
    /// Also save it in the config file
    #[arg(long)]
    pub persist: bool,
}

#[derive(Args, Debug)]
pub struct BindingsArgs {
    /// Only this profile (`base` or a profile name)
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Read this config file instead of asking the daemon
    #[arg(long, value_name = "PATH")]
    pub file: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum ActionsCmd {
    /// Every action you can bind or invoke
    List,
    /// Run an action, e.g. `actions invoke kde.overview`
    Invoke(InvokeArgs),
}

#[derive(Args, Debug)]
pub struct InvokeArgs {
    pub action: String,
    /// JSON parameters, e.g. '{"menu":"main"}'
    pub params: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum PluginsCmd {
    /// Running plugins
    List,
    /// One plugin's state, capabilities, restarts and last error
    Status { name: String },
    /// Restart a plugin process
    Restart { name: String },
}

#[derive(Args, Debug)]
pub struct EventsArgs {
    /// Topics to show (default: all)
    #[arg(value_parser = clap::builder::PossibleValuesParser::new(detent_ipc::TOPICS))]
    pub topics: Vec<String>,
    /// Exit after N events
    #[arg(short = 'n', long, value_name = "N")]
    pub count: Option<u64>,
}

#[derive(Args, Debug)]
pub struct HapticArgs {
    /// Waveform name or id (default: damp-state-change)
    pub waveform: Option<String>,
    /// List waveform names instead
    #[arg(long, conflicts_with = "waveform")]
    pub list: bool,
}

#[derive(Subcommand, Debug)]
pub enum DebugCmd {
    /// Every HID++ feature the device advertises
    Features,
    /// Every reprogrammable control (CID), its flags and current reporting
    Controls,
}

#[derive(Subcommand, Debug)]
pub enum UpdateCmd {
    /// Check whether a newer release exists
    Check,
}

#[derive(Args, Debug)]
pub struct SponsorArgs {
    /// Also open GitHub Sponsors in the default browser
    #[arg(long)]
    pub open: bool,
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::*;

    fn parse(args: &[&str]) -> Cli {
        let mut v = vec!["detentctl"];
        v.extend_from_slice(args);
        Cli::try_parse_from(v).unwrap_or_else(|e| panic!("{args:?}: {e}"))
    }

    #[test]
    fn command_tree_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn globals_anywhere() {
        let c = parse(&[
            "settings", "list", "--json", "-d", "hidraw9", "--color", "never",
        ]);
        assert!(c.global.json);
        assert_eq!(c.global.device.as_deref(), Some("hidraw9"));
        assert_eq!(c.global.color, ColorWhen::Never);
        let c = parse(&["-vv", "--socket", "/tmp/s", "status"]);
        assert_eq!(c.global.verbose, 2);
        assert_eq!(c.global.socket, Some(PathBuf::from("/tmp/s")));
        assert!(Cli::try_parse_from(["detentctl", "-q", "-v", "status"]).is_err());
    }

    #[test]
    fn groups_and_verbs() {
        assert!(matches!(
            parse(&["devices"]).cmd,
            Cmd::Devices { cmd: None }
        ));
        assert!(matches!(
            parse(&["devices", "battery"]).cmd,
            Cmd::Devices {
                cmd: Some(DevicesCmd::Battery)
            }
        ));
        match parse(&["settings", "set", "dpi", "1600", "--persist"]).cmd {
            Cmd::Settings {
                cmd: Some(SettingsCmd::Set(a)),
            } => {
                assert_eq!(
                    (a.path.as_str(), a.value.as_str(), a.persist),
                    ("dpi", "1600", true)
                )
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            parse(&["config", "validate"]).cmd,
            Cmd::Config(ConfigCmd::Check { file: None })
        ));
        assert!(matches!(
            parse(&["daemon", "restart", "--no-wait"]).cmd,
            Cmd::Daemon(DaemonCmd::Restart(WaitArgs { no_wait: true }))
        ));
        match parse(&["logs", "-f", "-n", "10"]).cmd {
            Cmd::Logs(l) => assert!(l.follow && l.lines == 10),
            other => panic!("{other:?}"),
        }
        assert!(matches!(parse(&["disable"]).cmd, Cmd::Disable));
        assert!(matches!(
            parse(&["completions", "zsh"]).cmd,
            Cmd::Completions { .. }
        ));
        assert!(matches!(
            parse(&["update", "check"]).cmd,
            Cmd::Update(UpdateCmd::Check)
        ));
        assert!(matches!(
            parse(&["sponsor"]).cmd,
            Cmd::Sponsor(SponsorArgs { open: false })
        ));
        assert!(matches!(
            parse(&["sponsor", "--open"]).cmd,
            Cmd::Sponsor(SponsorArgs { open: true })
        ));
    }

    #[test]
    fn events_aliases_and_topic_validation() {
        for name in ["events", "monitor", "watch"] {
            match parse(&[name, "remap", "link", "-n", "3"]).cmd {
                Cmd::Events(e) => {
                    assert_eq!(e.topics, ["remap", "link"]);
                    assert_eq!(e.count, Some(3));
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(Cli::try_parse_from(["detentctl", "events", "bogus"]).is_err());
    }

    #[test]
    fn legacy_spellings_still_parse() {
        assert!(matches!(
            parse(&["get", "dpi"]).cmd,
            Cmd::Get { path: Some(_) }
        ));
        assert!(matches!(parse(&["set", "dpi", "800"]).cmd, Cmd::Set(_)));
        assert!(matches!(parse(&["dump-features"]).cmd, Cmd::DumpFeatures));
        assert!(matches!(parse(&["reset", "--quiet"]).cmd, Cmd::Reset));
        assert!(matches!(
            parse(&["invoke", "kde.overview"]).cmd,
            Cmd::Invoke(_)
        ));
    }

    #[test]
    fn haptic_list_conflicts_with_waveform() {
        assert!(parse(&["haptic", "--list"]).cmd_is_haptic_list());
        assert!(Cli::try_parse_from(["detentctl", "haptic", "wave", "--list"]).is_err());
    }

    impl Cli {
        fn cmd_is_haptic_list(&self) -> bool {
            matches!(&self.cmd, Cmd::Haptic(h) if h.list)
        }
    }
}
