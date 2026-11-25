//! detentctl: the Detent CLI. Every user-facing feature is reachable here; the command
//! reference is docs/user/cli.md and the RPCs behind each command are in docs/specs/ipc.md.
//!
//! Layout: `cli` (clap tree) → `cmd::<group>` (logic) → `output` (human printers; `--json`
//! prints the same typed values). Errors go to stderr with a sysexits code (`exit`).

mod cli;
mod cmd;
mod ctx;
mod direct;
mod exit;
mod names;
mod output;

use std::io::IsTerminal;
use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    // Rust ignores SIGPIPE, so `detentctl actions | head` would panic on a closed pipe.
    // Restore the default (die quietly), as Unix CLIs do. Windows has no SIGPIPE.
    // SAFETY: called first thing in main, before any thread exists; SIG_DFL is a valid handler.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let cli = cli::Cli::parse();
    let color = ctx::color_enabled(cli.global.color, std::io::stderr().is_terminal());
    match cmd::run(cli) {
        Ok(()) => ExitCode::from(exit::OK),
        Err(e) => {
            let st = output::Style::new(color);
            eprintln!("{} {e:#}", st.red(&st.bold("error:")));
            if let Some(h) = exit::hint(&e) {
                eprintln!("{} {h}", st.dim("hint:"));
            }
            ExitCode::from(exit::code_for(&e))
        }
    }
}
