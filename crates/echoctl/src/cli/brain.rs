//! The Brain: `echoctl brain …` and the top-level `echoctl mcp`.

use anyhow::Result;
use clap::Subcommand;

use super::print_json;
use crate::{ai, brain, notes};

#[derive(Subcommand)]
pub enum BrainCmd {
    /// What the Brain holds, and its settings
    Status,
    /// Bring it up to date with the calls (only what changed; vectors for sections without one)
    Sync,
    /// Search it: keywords in any language and meaning (when an embedding server is set), fused
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
    /// The numbers lane: governed metrics from a client's reporting workbooks
    Numbers {
        #[command(subcommand)]
        cmd: NumbersCmd,
    },
    /// (dev) What the Brain's readers make of files: method, pages, routing (why a page goes to the page reader), date
    #[command(hide = true)]
    ReadFile {
        paths: Vec<std::path::PathBuf>,
        /// Print each page's text too
        #[arg(long)]
        text: bool,
    },
    /// (dev) Where a work item comes up elsewhere in its thread: the Work tab's "Mentioned also in"
    #[command(hide = true)]
    Mentions {
        #[arg(long)]
        thread: String,
        #[arg(long)]
        title: String,
        /// JSON [[call stamp, quote], …]
        #[arg(long, default_value = "[]")]
        quotes: String,
    },
    /// A client's or a thread's insights (--run: refresh them now, Claude on the default profile)
    Insights {
        #[arg(long, conflicts_with = "thread")]
        client: Option<String>,
        #[arg(long)]
        thread: Option<String>,
        #[arg(long)]
        run: bool,
    },
    /// The sections nearest to one, from other sources
    Related {
        chunk: i64,
        #[arg(long, default_value_t = 6)]
        limit: usize,
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
pub enum NumbersCmd {
    /// Profile the client's workbooks (its threads' folders, or --root): sheets, headers, samples
    Profile {
        client: String,
        /// A folder to read instead of the client's threads' (testing)
        #[arg(long)]
        root: Option<std::path::PathBuf>,
    },
    /// Claude proposes the families and metrics from the profiles (saved as proposed)
    Propose {
        client: String,
        #[arg(long)]
        root: Option<std::path::PathBuf>,
    },
    /// Use a families config written by hand or by Brain (families.<corpus>.json), and its metrics.json
    Import {
        client: String,
        families: std::path::PathBuf,
        #[arg(long)]
        metrics: Option<std::path::PathBuf>,
        /// Accept it at once
        #[arg(long)]
        accept: bool,
    },
    /// Accept the proposed families (the build reads them from now on)
    Accept { client: String },
    /// Build the client's facts from its accepted families
    Build {
        client: String,
        #[arg(long)]
        root: Option<std::path::PathBuf>,
        /// Build a config that isn't accepted yet (testing)
        #[arg(long)]
        force: bool,
    },
    /// Every client's families, catalog and last build
    Show,
    /// The governed metrics (MCP list_metrics)
    List {
        #[arg(long)]
        client: Option<String>,
    },
    /// A metric's values (MCP get_metric)
    Get {
        name: String,
        #[arg(long)]
        client: Option<String>,
        #[arg(long)]
        grain: Option<String>,
        #[arg(long)]
        entity: Option<String>,
        #[arg(long)]
        month: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum TaxonomyCmd {
    /// Every topic and term, with its sections and threads, and the coverage
    Show,
    /// Build it from the calls (Claude, on the default AI profile): every layer, or one (--layer shared | a client's id);
    /// a provisional version to review, then the layer's sections tagged
    Build {
        #[arg(long)]
        layer: Option<String>,
    },
    /// Whether each layer's topics still fit its calls (no model): new sections fitting no topic, one
    /// topic taking most, calls outgrowing its build (JSON)
    Drift,
    /// Topics that look in the wrong place (no model): clients' topics naming one thing → up to the
    /// practice; a practice topic only one client uses → down to it. --apply KEY does one, --dismiss KEY
    /// keeps it where it is
    Moves {
        #[arg(long)]
        apply: Option<String>,
        #[arg(long)]
        dismiss: Option<String>,
    },
    /// Split a topic that is too broad into 2–6 subtopics (Claude, on the default AI profile), then tag only
    /// its sections again (layer: shared | a client's id)
    Split { layer: String, topic: String },
    /// Tag sections not tagged yet (--again: all of them; --layer: only one layer's sections)
    Classify {
        #[arg(long)]
        again: bool,
        #[arg(long)]
        layer: Option<String>,
    },
    /// The user's change (JSON: {op: accept|add|rename|merge|move|remove|set, label|name, term?, to?, into?, parent?, …})
    Edit { json: String },
}

pub fn mcp(profile: Option<String>, client: Option<String>, thread: Vec<String>, call: Option<String>, who: Option<String>) -> Result<()> {
    let access = brain::access::Access { profile: profile.unwrap_or_else(ai::default_id), client, threads: thread, call };
    brain::mcp::serve(Some(access), who)
}

pub fn brain_command(cmd: BrainCmd) -> Result<()> {
    // Reading is allowed anywhere (a session Echo started reads it through MCP); changing it isn't.
    if let BrainCmd::Mcp { profile, client, thread, call, who } = cmd {
        let access = brain::access::Access { profile: profile.unwrap_or_else(ai::default_id), client, threads: thread, call };
        return brain::mcp::serve(Some(access), who);
    }
    if notes::headless() {
        anyhow::bail!("the Brain isn't changed from a run Echo started");
    }
    match cmd {
        BrainCmd::Mcp { .. } => unreachable!(),
        BrainCmd::Taxonomy { cmd } => {
            let progress = |m: &str| eprintln!("{m}");
            match cmd {
                TaxonomyCmd::Show => print_json(&brain::taxonomy::view()?),
                TaxonomyCmd::Drift => print_json(&serde_json::json!(brain::taxonomy::drift()?)),
                TaxonomyCmd::Split { layer, topic } => print_json(&brain::taxonomy::split(&layer, &topic, &progress)?),
                TaxonomyCmd::Moves { apply, dismiss } => {
                    if let Some(k) = apply {
                        println!("{}", brain::taxonomy::apply_move(&k)?);
                    }
                    if let Some(k) = dismiss {
                        brain::taxonomy::dismiss_move(&k)?;
                    }
                    print_json(&serde_json::json!(brain::taxonomy::moves()?))
                }
                TaxonomyCmd::Build { layer } => print_json(&match layer {
                    Some(l) => brain::taxonomy::induce(&l, &progress)?,
                    None => brain::taxonomy::induce_all(&progress)?,
                }),
                TaxonomyCmd::Classify { again, layer } => print_json(&serde_json::json!({"classified": brain::taxonomy::classify(layer.as_deref(), again, &progress)?})),
                TaxonomyCmd::Edit { json } => {
                    let op: serde_json::Value = serde_json::from_str(&json).map_err(|e| anyhow::anyhow!("not JSON: {e}"))?;
                    brain::taxonomy::edit(&op)?;
                    print_json(&brain::taxonomy::view()?)
                }
            }
        }
        BrainCmd::Status => print_json(&brain::status()?),
        BrainCmd::Sync => print_json(&serde_json::to_value(brain::sync()?)?),
        BrainCmd::Search { query, limit, within, client, thread } => {
            let scope = client.as_deref().map(|c| ("client", c)).or(thread.as_deref().map(|t| ("thread", t)));
            print_json(&brain::search(&query, limit, within.as_deref(), scope)?)
        }
        BrainCmd::Scopes => print_json(&brain::store::scopes(&brain::store_path())?),
        BrainCmd::Senders { client, trust, ignore, forget } => {
            let pick = trust.map(|k| ("trust", k)).or(ignore.map(|k| ("ignore", k))).or(forget.map(|k| ("forget", k)));
            if let (Some(c), Some((d, k))) = (&client, pick) {
                brain::senders::decide(c, &k, d)?;
            }
            let mut v = brain::senders::view();
            if let Some(c) = &client {
                v["clients"] = serde_json::json!({c.clone(): v["clients"][c].clone()});
            }
            print_json(&v)
        }
        BrainCmd::Files => print_json(&brain::files::folder::list_all()?),
        BrainCmd::Numbers { cmd } => {
            use brain::numbers as n;
            let root = |r: Option<std::path::PathBuf>| r.map(|r| vec![(r, vec![])]);
            match cmd {
                NumbersCmd::Profile { client, root: r } => print_json(&n::profile::run(&client, root(r))?["summary"]),
                NumbersCmd::Propose { client, root: r } => print_json(&n::profile::propose(&client, root(r))?),
                NumbersCmd::Import { client, families, metrics, accept } => {
                    let c = n::import_config(&client, &families, accept)?;
                    if let Some(m) = metrics {
                        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&m)?)?;
                        let cat: std::collections::BTreeMap<String, serde_json::Value> = serde_json::from_value(v.get("metrics").cloned().unwrap_or(v))?;
                        n::save_catalog(&client, &cat)?;
                    }
                    print_json(&serde_json::json!({"families": c.families.len(), "accepted": c.accepted()}));
                }
                NumbersCmd::Accept { client } => {
                    let mut c = n::load_config(&client).ok_or_else(|| anyhow::anyhow!("no families for {client}: propose or import them first"))?;
                    c.check()?;
                    c._echo = Some(serde_json::json!({"status": "accepted", "at": chrono::Utc::now().to_rfc3339(), "was": c._echo}));
                    n::save_config(&client, &c)?;
                    print_json(&serde_json::json!({"accepted": c.families.len()}));
                }
                NumbersCmd::Build { client, root: r, force } => {
                    let c = n::load_config(&client).ok_or_else(|| anyhow::anyhow!("no families for {client}: propose or import them first"))?;
                    anyhow::ensure!(force || c.accepted(), "the families for {client} are proposed, not accepted: accept them first (or --force)");
                    print_json(&n::build::run(&client, &c, root(r))?);
                }
                NumbersCmd::Show => print_json(&n::overview()?),
                NumbersCmd::List { client } => print_json(&n::list_metrics(client.as_deref())?),
                NumbersCmd::Get { name, client, grain, entity, month } => print_json(&n::get_metric(&serde_json::json!({"name": name, "client": client, "grain": grain, "entity": entity, "month": month}))?),
            }
        }
        BrainCmd::ReadFile { paths, text } => {
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            for p in paths {
                match brain::files::read(&p) {
                    Ok(r) => {
                        let body = r.md.clone().unwrap_or_else(|| r.pages.iter().map(|x| x.text.clone()).collect::<Vec<_>>().join("\n"));
                        let (date, from) = brain::files::exact_date(&body, &p, &today);
                        let pages: Vec<serde_json::Value> = r
                            .pages
                            .iter()
                            .map(|x| {
                                let mut v = serde_json::json!({"n": x.n, "title": x.title, "chars": x.text.chars().count(), "tables": x.tables.len(), "drawings": x.drawings,
                                    "pictures": x.pictures, "connectors": x.connectors, "charts": x.charts, "smartart": x.smartart, "route": brain::files::route(x)});
                                if text {
                                    v["text"] = serde_json::json!(x.text);
                                }
                                v
                            })
                            .collect();
                        let flagged = r.pages.iter().filter(|x| brain::files::route(x).is_some()).count();
                        print_json(&serde_json::json!({"file": p, "method": r.method, "fidelity": r.fidelity, "skip": r.skip, "date": date, "date_from": from,
                            "chars": r.md.as_ref().map(|m| m.chars().count()), "figures": r.figures, "pages": r.pages.len(), "flagged": flagged,
                            "page_list": pages, "md": if text { r.md } else { None }}));
                    }
                    Err(e) => print_json(&serde_json::json!({"file": p, "error": format!("{e:#}")})),
                }
            }
        }
        BrainCmd::Mentions { thread, title, quotes } => {
            let q: Vec<(String, String)> = serde_json::from_str(&quotes).map_err(|e| anyhow::anyhow!("--quotes: {e}"))?;
            print_json(&brain::views::mentions(&thread, &title, &q)?)
        }
        BrainCmd::Insights { client, thread, run } => {
            let scope = client.as_deref().map(|c| ("client", c)).or(thread.as_deref().map(|t| ("thread", t))).ok_or_else(|| anyhow::anyhow!("--client or --thread"))?;
            if run {
                print_json(&brain::insights::run(scope, &|m: &str| eprintln!("{m}"))?);
            }
            print_json(&brain::insights::view(scope)?)
        }
        BrainCmd::Related { chunk, limit } => print_json(&brain::store::related(&brain::store_path(), chunk, limit)?),
        BrainCmd::Set { endpoint, api, model, key, enabled } => {
            let mut s = brain::settings();
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
            brain::save_settings(&s)?;
            print_json(&brain::status()?)
        }
    }
    Ok(())
}
