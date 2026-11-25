//! `events` (aliases `monitor`, `watch`): stream daemon events. `--json` prints NDJSON,
//! one compact object per line, flushed per event (pipe-friendly).

use std::io::Write;

use anyhow::Result;

use crate::cli::EventsArgs;
use crate::ctx::Ctx;
use crate::output::human;

pub fn run(ctx: &Ctx, a: &EventsArgs) -> Result<()> {
    let client = ctx.daemon()?.into_client();
    if !ctx.quiet && !ctx.json {
        let what = if a.topics.is_empty() {
            "all events".to_string()
        } else {
            a.topics.join(", ")
        };
        eprintln!(
            "{}",
            ctx.style.dim(&format!("watching {what} (Ctrl+C to stop)"))
        );
    }
    let mut seen = 0u64;
    let limit = a.count;
    client.watch(&a.topics, |ev| {
        if ctx.json {
            println!("{ev}");
        } else {
            human::event(&ctx.style, &ev);
        }
        let _ = std::io::stdout().flush();
        seen += 1;
        limit.is_none_or(|n| seen < n)
    })?;
    Ok(())
}
