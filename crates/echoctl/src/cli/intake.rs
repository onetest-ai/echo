//! The Brain: `echoctl intake …` and the top-level `echoctl mcp`.

use anyhow::Result;
use clap::Subcommand;

use super::print_json;
use crate::{ai, intake, notes};

#[derive(Subcommand)]
pub enum BrainCmd {
    /// What the Brain holds, and its settings
    Status,
    /// Bring it up to date with the calls (only what changed; vectors for sections without one)
    Sync,
    /// Search it: keywords in any language, re-ranked by meaning when an embedding server is set
    Search {
        query: String,
        #[arg(long, default_value_t = 8)]
        limit: usize,
        /// Only sources whose id contains this (e.g. call/2026-10)
        #[arg(long)]
        within: Option<String>,
        /// Only one client's sections (its id; "-": calls with no client)
        #[arg(long, conflicts_with = "thread")]
        client: Option<String>,
        /// Only one thread's sections (its id; "-": calls in no thread)
        #[arg(long)]
        thread: Option<String>,
        /// Only these kinds of source: call, mail, channel, document (comma-separated)
        #[arg(long, value_delimiter = ',')]
        kind: Vec<String>,
        /// Only these kinds of section: summary, decisions, action_items, open_questions, minutes (any other minutes
        /// section), transcript, chat, mail, channel, document, card (comma-separated)
        #[arg(long, value_delimiter = ',')]
        section: Vec<String>,
        /// Only sections dated on or after this day (YYYY-MM-DD); without --since/--until a date in the query is used
        #[arg(long)]
        since: Option<String>,
        /// Only sections dated on or before this day (YYYY-MM-DD)
        #[arg(long)]
        until: Option<String>,
        /// Only what this person said or wrote (a part of the speaker's or author's name; minutes and documents have none)
        #[arg(long)]
        person: Option<String>,
    },
    /// The clients and threads it covers, with their documents and sections
    Scopes,
    /// Who may speak into the Brain through mail and channels, per client: trusted, ignored, waiting.
    /// A decision drops what sources gave and reads them again at the next sync
    Senders {
        /// The client (a thread with none: "-")
        #[arg(long)]
        client: Option<String>,
        /// Trust this sender (an address, or teams:<id> as listed)
        #[arg(long, requires = "client", conflicts_with_all = ["ignore", "forget"])]
        trust: Option<String>,
        /// Never read this sender
        #[arg(long, requires = "client", conflicts_with = "forget")]
        ignore: Option<String>,
        /// Forget a decision (the sender waits again)
        #[arg(long, requires = "client")]
        forget: Option<String>,
    },
    /// The thread folders' files: what was read, how, and why the others weren't
    Files,
    /// (dev) What the Brain's readers make of files: method, pages, date
    #[command(hide = true)]
    ReadFile {
        paths: Vec<std::path::PathBuf>,
        /// Print each page's text too
        #[arg(long)]
        text: bool,
    },
    /// Serve it to Claude Code as an MCP server over stdio (read-only; Brain's tool names). Only one AI
    /// profile's sources (--profile; default: the default profile's), narrowed for a call's session
    /// to its client (--client), else its threads (--thread), else the call itself (--call)
    Mcp {
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        client: Option<String>,
        #[arg(long)]
        thread: Vec<String>,
        #[arg(long)]
        call: Option<String>,
        /// Who runs it (folder | call | routine), for the usage counts in Settings → Brain
        #[arg(long)]
        who: Option<String>,
    },
    /// The taxonomy: topics and terms (it supersedes thread glossaries)
    Taxonomy {
        #[command(subcommand)]
        cmd: TaxonomyCmd,
    },
    /// The embedding server (Settings → Brain): Ollama or OpenAI-compatible
    Set {
        #[arg(long)]
        endpoint: Option<String>,
        /// ollama | openai
        #[arg(long)]
        api: Option<String>,
        #[arg(long)]
        model: Option<String>,
        /// A bearer token for an OpenAI-compatible server ("" removes it)
        #[arg(long)]
        key: Option<String>,
        /// The Brain at all: false turns it off (nothing read, indexed or offered to Claude; the store stays)
        #[arg(long)]
        enabled: Option<bool>,
    },
}

#[derive(Subcommand)]
pub enum TaxonomyCmd {
    /// Every topic and term, with its sections and threads, and the coverage
    Show,
    /// The user's change (JSON: {op: accept|add|rename|merge|move|remove|set, label|name, term?, to?, into?, parent?, …})
    Edit { json: String },
}

pub fn mcp(profile: Option<String>, client: Option<String>, thread: Vec<String>, call: Option<String>, who: Option<String>) -> Result<()> {
    let access = intake::access::Access { profile: profile.unwrap_or_else(ai::default_id), client, threads: thread, call };
    intake::mcp::serve(Some(access), who)
}

pub fn brain_command(cmd: BrainCmd) -> Result<()> {
    // Reading is allowed anywhere (a session Echo started reads it through MCP); changing it isn't.
    if let BrainCmd::Mcp { profile, client, thread, call, who } = cmd {
        let access = intake::access::Access { profile: profile.unwrap_or_else(ai::default_id), client, threads: thread, call };
        return intake::mcp::serve(Some(access), who);
    }
    if notes::run_limited() {
        anyhow::bail!("the Brain isn't changed from a run Echo started");
    }
    match cmd {
        BrainCmd::Mcp { .. } => unreachable!(),
        BrainCmd::Taxonomy { cmd } => {
            match cmd {
                TaxonomyCmd::Show => print_json(&intake::taxonomy::view()?),
                TaxonomyCmd::Edit { json } => {
                    let op: serde_json::Value = serde_json::from_str(&json).map_err(|e| anyhow::anyhow!("not JSON: {e}"))?;
                    intake::taxonomy::edit(&op)?;
                    print_json(&intake::taxonomy::view()?)
                }
            }
        }
        BrainCmd::Status => print_json(&intake::status()?),
        BrainCmd::Sync => print_json(&serde_json::to_value(intake::sync()?)?),
        BrainCmd::Search { query, limit, within, client, thread, kind, section, since, until, person } => {
            let scope = client.as_deref().map(|c| ("client", c)).or(thread.as_deref().map(|t| ("thread", t)));
            let filters = intake::store::Filters { kinds: kind, sections: section, since, until, person };
            print_json(&intake::search(&query, limit, within.as_deref(), scope, &filters)?)
        }
        BrainCmd::Scopes => print_json(&intake::store::scopes(&intake::store_path())?),
        BrainCmd::Senders { client, trust, ignore, forget } => {
            let pick = trust.map(|k| ("trust", k)).or(ignore.map(|k| ("ignore", k))).or(forget.map(|k| ("forget", k)));
            if let (Some(c), Some((d, k))) = (&client, pick) {
                intake::senders::decide(c, &k, d)?;
            }
            let mut v = intake::senders::view();
            if let Some(c) = &client {
                v["clients"] = serde_json::json!({c.clone(): v["clients"][c].clone()});
            }
            print_json(&v)
        }
        BrainCmd::Files => print_json(&intake::files::folder::list_all()?),
        BrainCmd::ReadFile { paths, text } => {
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            for p in paths {
                match intake::files::read(&p) {
                    Ok(r) => {
                        let body = r.md.clone().unwrap_or_else(|| r.pages.iter().map(|x| x.text.clone()).collect::<Vec<_>>().join("\n"));
                        let (date, from) = intake::files::exact_date(&body, &p, &today);
                        let pages: Vec<serde_json::Value> = r
                            .pages
                            .iter()
                            .map(|x| {
                                let mut v = serde_json::json!({"n": x.n, "title": x.title, "chars": x.text.chars().count(), "tables": x.tables.len(), "drawings": x.drawings,
                                    "pictures": x.pictures, "connectors": x.connectors, "charts": x.charts, "smartart": x.smartart});
                                if text {
                                    v["text"] = serde_json::json!(x.text);
                                }
                                v
                            })
                            .collect();
                        print_json(&serde_json::json!({"file": p, "method": r.method, "fidelity": r.fidelity, "skip": r.skip, "date": date, "date_from": from,
                            "chars": r.md.as_ref().map(|m| m.chars().count()), "figures": r.figures, "pages": r.pages.len(),
                            "page_list": pages, "md": if text { r.md } else { None }}));
                    }
                    Err(e) => print_json(&serde_json::json!({"file": p, "error": format!("{e:#}")})),
                }
            }
        }
        BrainCmd::Set { endpoint, api, model, key, enabled } => {
            let mut s = intake::settings();
            if let Some(on) = enabled {
                s.enabled = Some(on);
            }
            if let Some(e) = endpoint {
                s.endpoint = e.trim().to_string();
            }
            if let Some(a) = api {
                s.api = a.trim().to_string();
            }
            if let Some(m) = model {
                s.model = m.trim().to_string();
            }
            if let Some(k) = key {
                s.key = (!k.is_empty()).then_some(k);
            }
            intake::save_settings(&s)?;
            print_json(&intake::status()?)
        }
    }
    Ok(())
}
