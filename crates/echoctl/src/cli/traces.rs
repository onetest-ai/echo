//! Traces: what Echo ran, step by step, kept on this computer (`telemetry`).

use anyhow::Result;
use clap::{Args, Subcommand};

use super::print_json;
use crate::telemetry;

/// `echoctl traces`: which traces to list.
#[derive(Args)]
pub struct ListArgs {
    /// The last N days, today included: a run's detail is kept 7 days, its numbers 90
    #[arg(long, default_value_t = telemetry::KEEP_DAYS)]
    days: i64,
    /// Only this job's (`thread_sync`, `minutes`, a routine's `<plugin>.<routine>`, …)
    #[arg(long)]
    job: Option<String>,
    /// Only this kind: `job`, `claude` (a Claude Code run of no job), `request` (to another service), `model` (a model
    /// call without Claude Code), `transcription`, `command` (a CLI command that failed)
    #[arg(long)]
    kind: Option<String>,
    /// At most this many, newest first
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Subcommand)]
pub enum TracesCmd {
    /// One trace (JSON): its spans nested, each with its events in order — Echo's (the run's arguments, input and every
    /// stream event) and Claude Code's own (its model requests and tools)
    Show { id: String },
    /// Where traces are sent (JSON; set up in Settings → Activity): `echoctl traces export` shows it and how it goes,
    /// `echoctl traces export --off` stops sending
    Export {
        #[arg(long)]
        off: bool,
    },
}

pub fn traces(cmd: Option<TracesCmd>, list: ListArgs) -> Result<()> {
    match cmd {
        None => print_json(&telemetry::list(list.days, list.job.as_deref(), list.kind.as_deref(), list.limit)),
        Some(TracesCmd::Show { id }) => print_json(&telemetry::show(&id)?),
        Some(TracesCmd::Export { off }) => {
            if off {
                let mut v = serde_json::to_value(telemetry::export::load())?;
                v["on"] = serde_json::json!(false);
                v["keep_headers"] = serde_json::json!(true);
                telemetry::export::set(&v)?;
            }
            print_json(&telemetry::export::status())
        }
    }
    Ok(())
}
