//! Knowledge vaults: `echoctl knowledge …`.

use anyhow::Result;
use clap::Subcommand;
use crate::{calls, knowledge};

#[derive(Subcommand)]
pub enum KnowledgeCmd {
    /// The vaults Echo knows (JSON)
    List,
    /// What connecting a folder would find; changes nothing (JSON)
    Detect { folder: std::path::PathBuf },
    /// Connect a folder of notes as it is; nothing in it changes
    Add {
        folder: std::path::PathBuf,
        #[arg(long)]
        name: Option<String>,
        /// The AI profile that reads it (default: the default profile)
        #[arg(long)]
        profile: Option<String>,
        /// Save calls' minutes here: auto, ask or off
        #[arg(long)]
        minutes: Option<String>,
    },
    /// Change a vault's setting: name, profile, minutes, scope, clients, threads, tag, transcript, chat, commit, or
    /// pages / index / notes / naming / templates / sources ("" takes an override back)
    Set { vault: String, key: String, value: String },
    /// Detect the vault's structure again (your changes stay)
    Reread { vault: String },
    /// Forget a vault (its folder is left as it is)
    Remove { vault: String },
    /// Start a new vault from Echo's twin starter (in Echo's data folder unless --folder)
    New {
        name: String,
        #[arg(long)]
        folder: Option<std::path::PathBuf>,
        /// A plain folder, no git repository
        #[arg(long)]
        no_git: bool,
        /// Save calls' minutes here: auto, ask or off
        #[arg(long, default_value = "auto")]
        minutes: String,
    },
    /// A vault's pages, or the ones matching --q (title, path, #tag, words) (JSON)
    Pages {
        vault: String,
        #[arg(long)]
        q: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// One page: properties, links, backlinks (JSON)
    Read { vault: String, path: String },
    /// The vault's checks and Health score: pages not in the index, broken links, isolated pages, pages without
    /// properties, unknown tags, index rows to missing pages, usual properties missing, duplicate titles, an oversized
    /// index (JSON)
    Check { vault: String },
    /// AI checks: Claude Code reads the vault's pages (all, or those changed in the last 30 days and the pages they
    /// link to, up to 300) and Echo keeps the findings whose quotes are on their pages; waits, then prints the check,
    /// what it kept and dropped, and its cost (JSON). Nothing in the vault changes
    Lint { vault: String },
    /// Add index rows for pages the index doesn't list (vault-relative paths)
    IndexAdd { vault: String, paths: Vec<String> },
    /// Save a call's minutes into a vault (its note and index row). Without --vault: the vaults whose rules take it
    SaveMinutes {
        call: String,
        #[arg(long)]
        vault: Option<String>,
        /// A new note beside one you changed
        #[arg(long)]
        new: bool,
        /// Write over the note even though you changed it
        #[arg(long)]
        replace: bool,
        /// Save it even though the vault is read by another AI profile than the call's
        #[arg(long)]
        anyway: bool,
    },
    /// Ingest: add sources to the vault's sources folder exactly as they came, prepare changes from New ones with the
    /// vault's ingest skill on a copy of the vault, write the approved ones, or discard a run (JSON)
    Ingest {
        vault: String,
        #[command(subcommand)]
        cmd: IngestCmd,
    },
    /// The twin skill: add it to a vault that has none, or (--global) install it for all your Claude Code sessions
    Twin {
        vault: String,
        /// Install it in ~/.claude/skills for every Claude Code session
        #[arg(long)]
        global: bool,
        /// Take Echo's copy out of ~/.claude/skills
        #[arg(long)]
        remove: bool,
        /// The global skill's name (default: the vault's id)
        #[arg(long)]
        name: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum IngestCmd {
    /// Add sources, each as it came: a file (copied, up to 50 MB), an https page (its main text), `call:<stamp>` (a
    /// call's or memo's minutes), or the flags' --call, --mail, --teams, --url (JSON). Nothing is processed by adding
    Add {
        /// Files, https addresses and `call:<stamp>`s
        items: Vec<String>,
        /// A call's or memo's minutes (its folder, its stamp, or `latest`)
        #[arg(long)]
        call: Option<String>,
        /// With the call's transcript
        #[arg(long)]
        transcript: bool,
        /// With the call's meeting chat
        #[arg(long)]
        chat: bool,
        /// Add the call even though the vault is read by another AI profile than the call's
        #[arg(long)]
        anyway: bool,
        /// A mail thread, by one of its messages' Microsoft 365 id (Echo's sign-in)
        #[arg(long)]
        mail: Option<String>,
        /// A Teams chat, by its id (Echo's sign-in), from --since to --until (YYYY-MM-DD; default the last 7 days)
        #[arg(long)]
        teams: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        until: Option<String>,
        /// A web page, by its https address: its main text as Markdown
        #[arg(long)]
        url: Option<String>,
    },
    /// Prepare changes: the vault's ingest skill run by Claude Code on a copy of the vault, on these New sources
    /// (vault-relative; default every New one, the first 20); waits for the run and prints its changes (JSON)
    Run { sources: Vec<String> },
    /// Write a run's changes into the vault — every change not rejected, or only these paths (JSON)
    Apply {
        run: String,
        #[arg(long)]
        only: Vec<String>,
    },
    /// Drop a run: its copy and its changes (JSON)
    Discard { run: String },
}

/// One thing `ingest add` was given.
enum IngestItem {
    File(std::path::PathBuf),
    Web(String),
    Call(String),
    Mail(String),
    Teams(String),
}

/// A call named on the command line: its folder (or a path, or `latest`), else its stamp.
fn ingest_call(name: &str) -> Result<std::path::PathBuf> {
    calls::resolve_call(name).or_else(|e| calls::by_stamp(&calls::stamp_of(std::path::Path::new(name))).ok_or(e))
}

/// `ingest run|apply|discard` (spec K15-12): the app's lock, the app's checks.
fn ingest_run_command(v: &knowledge::Vault, cmd: IngestCmd) -> Result<serde_json::Value> {
    use knowledge::ingest;
    let summary = |run: &str| -> Result<serde_json::Value> {
        let mut st = ingest::status(v, run, u64::MAX)?;
        if let Some(o) = st.as_object_mut() {
            o.remove("events");
            o.remove("next");
        }
        let changes = if st["state"] == "review" { ingest::changes(v, run)? } else { vec![] };
        st["changes"] = serde_json::json!(changes.iter().map(|c| serde_json::json!({ "path": c.path, "kind": c.kind, "plus": c.plus, "minus": c.minus, "adds_only": c.adds_only, "state": c.state, "quotes": c.quotes.len(), "quote_note": c.quote_note })).collect::<Vec<_>>());
        Ok(st)
    };
    match cmd {
        IngestCmd::Run { sources } => {
            // Ctrl-C (or a TERM) stops the run's Claude Code with this CLI, never leaves it running.
            let _ = ctrlc::set_handler(|| {
                ingest::stop_all("Stopped from the command line");
            });
            let started = ingest::start(v, &sources)?;
            // Until it ends; what it waits for (a call's transcription, X-S2) is said as it waits.
            let mut said: Option<String> = None;
            while !ingest::wait(&started.run, std::time::Duration::from_secs(2)) {
                let waiting = ingest::status(v, &started.run, u64::MAX)?["waiting"].as_str().map(str::to_owned);
                if let Some(w) = waiting.as_deref().filter(|w| said.as_deref() != Some(*w)) {
                    eprintln!("{w}");
                }
                said = waiting;
            }
            let mut out = summary(&started.run)?;
            out["left"] = serde_json::json!(started.left);
            Ok(out)
        }
        IngestCmd::Apply { run, only } => {
            let all = ingest::changes(v, &run)?;
            for p in &only {
                if !all.iter().any(|c| &c.path == p) {
                    anyhow::bail!("run {run} has no change to {p}");
                }
            }
            for c in all.iter().filter(|c| c.state == "undecided" && (only.is_empty() || only.contains(&c.path))) {
                ingest::decide(v, &run, &c.path, ingest::Decision::Approve)?;
            }
            Ok(serde_json::to_value(ingest::write(v, &run, &only)?)?)
        }
        IngestCmd::Discard { run } => {
            ingest::discard(v, &run)?;
            Ok(serde_json::json!({ "discarded": run }))
        }
        IngestCmd::Add { .. } => unreachable!("an add is ingest_command's"),
    }
}

/// `knowledge lint` (spec K16-11): an AI check started as the window's *Run AI checks* is, under the app's lock, waited
/// for (what it waits for said as it waits, X-S2); its run, state, what it kept and dropped (and how many past the most
/// Echo keeps it didn't read), and its cost.
fn lint_command(v: &knowledge::Vault) -> Result<serde_json::Value> {
    use knowledge::lint;
    knowledge::refuse_headless()?;
    // Ctrl-C (or a TERM) stops the check's Claude Code with this CLI, never leaves it running.
    let _ = ctrlc::set_handler(|| {
        lint::stop_all("Stopped from the command line");
    });
    let run = lint::start(v)?;
    let mut said: Option<String> = None;
    while !lint::wait(&run, std::time::Duration::from_secs(2)) {
        let waiting = lint::status(v, &run, u64::MAX)?["waiting"].as_str().map(str::to_owned);
        if let Some(w) = waiting.as_deref().filter(|w| said.as_deref() != Some(*w)) {
            eprintln!("{w}");
        }
        said = waiting;
    }
    let st = lint::status(v, &run, u64::MAX)?;
    // The findings it kept, as Health lists them: each by its key, kind, title and pages with their lines.
    let found = if st["state"] == "done" { lint::kept(v, &run)?.0 } else { vec![] };
    let found: Vec<serde_json::Value> = found.iter().map(|f| serde_json::json!({ "key": f.key, "kind": f.kind, "title": f.title, "pages": f.pages, "fixable": f.fixable })).collect();
    Ok(serde_json::json!({ "run": run, "state": st["state"], "reason": st["reason"], "pages": st["pages"], "kept": st["kept"], "dropped": st["dropped"], "left": st["left"], "cost": st["cost"], "findings": found }))
}

fn ingest_command(vault: &str, cmd: IngestCmd) -> Result<serde_json::Value> {
    use knowledge::sources;
    knowledge::refuse_headless()?;
    let (items, call, transcript, chat, anyway, mail, teams, since, until, url) = match cmd {
        IngestCmd::Add { items, call, transcript, chat, anyway, mail, teams, since, until, url } => (items, call, transcript, chat, anyway, mail, teams, since, until, url),
        other => return ingest_run_command(&knowledge::get(vault)?, other),
    };
    // A positional is a page when it is a web address, a call when it says so, else a file.
    let mut todo: Vec<IngestItem> = items
        .into_iter()
        .map(|a| {
            let lower = a.to_ascii_lowercase();
            if lower.starts_with("https://") || lower.starts_with("http://") {
                IngestItem::Web(a)
            } else if let Some(c) = a.strip_prefix("call:") {
                IngestItem::Call(c.to_string())
            } else {
                IngestItem::File(a.into())
            }
        })
        .collect();
    todo.extend(call.map(IngestItem::Call));
    todo.extend(mail.map(IngestItem::Mail));
    todo.extend(teams.map(IngestItem::Teams));
    todo.extend(url.map(IngestItem::Web));
    if todo.is_empty() {
        anyhow::bail!("add files, https pages or call:<stamp>, or one of --call, --mail, --teams, --url");
    }
    if (transcript || chat || anyway) && !todo.iter().any(|i| matches!(i, IngestItem::Call(_))) {
        anyhow::bail!("--transcript, --chat and --anyway go with a call (call:<stamp> or --call)");
    }
    if (since.is_some() || until.is_some()) && !todo.iter().any(|i| matches!(i, IngestItem::Teams(_))) {
        anyhow::bail!("--since and --until go with a Teams chat (--teams)");
    }
    let day = |d: &Option<String>, default: chrono::NaiveDate| -> Result<chrono::NaiveDate> {
        d.as_deref().map(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| anyhow::anyhow!("{d}: a day as YYYY-MM-DD"))).unwrap_or(Ok(default))
    };
    let until = day(&until, chrono::Local::now().date_naive())?;
    let since = day(&since, sources::last_week(until))?;
    let v = knowledge::get(vault)?;
    let (mut added, mut refused) = (vec![], vec![]);
    for item in todo {
        let (name, r) = match item {
            IngestItem::File(f) => sources::add_files(&v, &[f]).remove(0),
            IngestItem::Web(u) => (u.clone(), sources::add_web(&v, &u)),
            IngestItem::Call(c) => (format!("call:{c}"), ingest_call(&c).and_then(|d| sources::add_call(&v, &d, sources::Include { transcript, chat }, anyway))),
            IngestItem::Mail(m) => (m.clone(), sources::add_mail(&v, &m)),
            IngestItem::Teams(t) => (t.clone(), sources::add_chat(&v, &t, since, until)),
        };
        match r {
            Ok(a) => added.push(serde_json::to_value(a)?),
            Err(e) => refused.push((name, format!("{e:#}"))),
        }
    }
    if added.is_empty() {
        anyhow::bail!("{}", refused.iter().map(|(n, why)| format!("{n}: {why}")).collect::<Vec<_>>().join("; "));
    }
    Ok(serde_json::json!({ "added": added, "refused": refused.into_iter().map(|(name, why)| serde_json::json!({ "name": name, "why": why })).collect::<Vec<_>>() }))
}

pub fn knowledge_command(cmd: KnowledgeCmd) -> Result<()> {
    use knowledge as kn;
    let v = match cmd {
        KnowledgeCmd::List => serde_json::Value::Array(kn::list()?.iter().map(kn::vault_value).collect()),
        KnowledgeCmd::Detect { folder } => serde_json::to_value(kn::detect::detect(&folder)?)?,
        KnowledgeCmd::Add { folder, name, profile, minutes } => kn::vault_value(&kn::add(&folder, kn::AddOpts { name, profile, minutes, ..Default::default() })?),
        KnowledgeCmd::Set { vault, key, value } => kn::vault_value(&kn::set(&vault, &key, &value)?),
        KnowledgeCmd::Reread { vault } => {
            kn::refuse_headless()?;
            kn::vault_value(&kn::reread(&vault)?)
        }
        KnowledgeCmd::Remove { vault } => {
            kn::remove(&vault)?;
            serde_json::json!({ "removed": vault })
        }
        KnowledgeCmd::Pages { vault, q, limit } => {
            let v = kn::get(&vault)?;
            let ix = kn::scan::index(&v)?;
            match q {
                Some(q) => serde_json::Value::Array(kn::scan::search(&v, &ix, &q, limit)),
                None => serde_json::json!(ix.pages.values().take(limit).map(|p| serde_json::json!({ "path": kn::scan::vault_rel(&v, &p.path), "title": p.title, "tags": p.tags })).collect::<Vec<_>>()),
            }
        }
        KnowledgeCmd::Read { vault, path } => {
            let mut p = kn::page::read(&kn::get(&vault)?, &path)?;
            if let Some(o) = p.as_object_mut() {
                o.remove("html");
            }
            p
        }
        KnowledgeCmd::Check { vault } => kn::health::check(&kn::get(&vault)?)?,
        KnowledgeCmd::Lint { vault } => lint_command(&kn::get(&vault)?)?,
        KnowledgeCmd::IndexAdd { vault, paths } => kn::page::index_add(&kn::get(&vault)?, &paths)?,
        KnowledgeCmd::SaveMinutes { call, vault, new, replace, anyway } => {
            kn::refuse_headless()?;
            let dir = calls::resolve_call(&call)?;
            match vault {
                Some(id) => serde_json::to_value(kn::export::save(&dir, &id, kn::export::How::Manual(kn::export::Manual { new, replace, anyway, ..Default::default() }))?)?,
                None => serde_json::to_value(kn::export::save_by_rules(&dir))?,
            }
        }
        KnowledgeCmd::Ingest { vault, cmd } => ingest_command(&vault, cmd)?,
        KnowledgeCmd::New { name, folder, no_git, minutes } => kn::starter::created_value(&kn::starter::create(&name, folder, !no_git, &minutes)?),
        KnowledgeCmd::Twin { vault, global, remove, name } => {
            let v = kn::get(&vault)?;
            if remove {
                serde_json::json!({ "removed": kn::starter::global_twin_remove(&v)? })
            } else if global {
                serde_json::json!({ "installed": kn::starter::global_twin(&v, name.as_deref())? })
            } else {
                serde_json::json!({ "added": kn::starter::add_twin_skill(&v)? })
            }
        }
    };
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}


#[cfg(test)]
mod ingest_cli {
    use super::*;
    use crate::{Cli, Cmd};
    use clap::Parser;
    use crate::knowledge::fixture;

    /// `echoctl knowledge ingest <vault> add <args…>`, parsed as the shell would hand it over.
    fn add(vault: &str, args: &[&str]) -> Result<serde_json::Value> {
        let mut all = vec!["echoctl", "knowledge", "ingest", vault, "add"];
        all.extend_from_slice(args);
        match Cli::try_parse_from(all)?.cmd {
            Cmd::Knowledge { cmd: KnowledgeCmd::Ingest { vault, cmd } } => ingest_command(&vault, cmd),
            _ => unreachable!("an ingest command"),
        }
    }

    #[test]
    fn add_takes_files_https_pages_and_calls_as_written() {
        // K15-12: `add <file|https-url|call:<stamp>>`, the flags kept.
        let (_env, root) = crate::paths::testenv::folders("kn-cli-ingest");
        let t = fixture::twin(&root.join("twin"));
        let v = knowledge::add(&t, knowledge::AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap();
        fixture::call("2026-11-05_090012-weekly-sync", "Weekly delivery sync", "system", fixture::MINUTES);
        let brief = root.join("brief.txt");
        std::fs::write(&brief, "made up").unwrap();
        let brief = brief.to_str().unwrap();
        let r = add(&v.id, &[brief, "call:2026-11-05_090012"]).unwrap();
        let paths: Vec<&str> = r["added"].as_array().unwrap().iter().map(|a| a["path"].as_str().unwrap()).collect();
        assert_eq!(paths.len(), 2, "{r}");
        assert_eq!(paths[0], "raw_inbox/brief.txt");
        assert!(paths[1].starts_with("raw_inbox/echo/") && paths[1].ends_with("-weekly-delivery-sync.md"), "{r}");
        // An https address is a web page: here refused by its own check, before anything is fetched. http is refused.
        let e = add(&v.id, &["https://contoso.example/a b"]).unwrap_err().to_string();
        assert!(e.contains("isn't a web address"), "{e}");
        let e = add(&v.id, &["http://contoso.example/notes"]).unwrap_err().to_string();
        assert!(e.contains("only https"), "{e}");
        // The flags still work, each with its kind.
        let r = add(&v.id, &["--call", "2026-11-05_090012-weekly-sync", "--transcript"]).unwrap();
        assert_eq!(r["added"].as_array().unwrap().len(), 1, "{r}");
        // A flag without its kind is refused, saying why; nothing is added.
        for args in [vec![brief, "--transcript"], vec![brief, "--anyway"], vec![brief, "--chat"], vec![brief, "--since", "2026-11-01"], vec!["--until", "2026-11-07", "https://contoso.example/x"]] {
            let e = add(&v.id, &args).unwrap_err().to_string();
            assert!(e.contains("go with"), "{args:?}: {e}");
        }
        assert!(add(&v.id, &[]).is_err(), "nothing named");
        let q = knowledge::sources::queue(&v).unwrap();
        assert_eq!(q.iter().filter(|s| s.path.starts_with("raw_inbox/brief")).count(), 1, "the refused adds wrote nothing");
    }
}

#[cfg(all(test, unix))]
mod lint_cli {
    use super::*;
    use crate::{Cli, Cmd};
    use clap::Parser;
    use crate::ai::fake;
    use crate::knowledge::fixture;

    #[test]
    fn lint_waits_and_prints_what_it_kept() {
        // K16-11: `echoctl knowledge lint <vault>`, with the fake.
        let (_env, root) = crate::paths::testenv::folders("kn-cli-lint");
        let _fake = fake::install(&root);
        knowledge::lint::reset();
        let p = fake::profile("w", None, &fake::signed_in("pat@example.com"));
        let t = fixture::twin(&root.join("twin"));
        let v = knowledge::add(&t, knowledge::AddOpts { name: Some("Twin".into()), profile: Some("w".into()), ..Default::default() }).unwrap();
        let found = serde_json::json!({"findings": [
            {"kind": "missing_summary", "title": "Acme Corp has no summary", "pages": [{"path": "knowledge/clients/acme.md", "quote": "Acme Corp tags client"}]},
            {"kind": "stale", "title": "Invented", "pages": [{"path": "knowledge/clients/acme.md", "quote": "Acme Corp closed its doors"}]},
        ]});
        std::fs::write(fake::file(&p, "findings.1.json"), found.to_string()).unwrap();
        std::fs::write(fake::file(&p, "lint.jsonl"), "{\"_findings\":1}\n{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"total_cost_usd\":0.12,\"usage\":{\"input_tokens\":10,\"output_tokens\":5}}\n").unwrap();
        let cmd = match Cli::try_parse_from(["echoctl", "knowledge", "lint", v.id.as_str()]).unwrap().cmd {
            Cmd::Knowledge { cmd: KnowledgeCmd::Lint { vault } } => vault,
            _ => unreachable!("a lint command"),
        };
        let out = lint_command(&knowledge::get(&cmd).unwrap()).unwrap();
        assert_eq!((out["state"].as_str(), out["kept"].as_u64(), out["dropped"].as_u64(), out["cost"].as_f64(), out["pages"].as_u64()), (Some("done"), Some(1), Some(1), Some(0.12), Some(4)), "{out}");
        assert!(out["run"].as_str().is_some_and(|r| !r.is_empty()) && out["reason"].is_null());
        assert_eq!((out["findings"][0]["kind"].as_str(), out["findings"][0]["title"].as_str(), out["findings"][0]["pages"].clone()), (Some("missing_summary"), Some("Acme Corp has no summary"), serde_json::json!([["knowledge/clients/acme.md", 2]])));
        knowledge::lint::reset();
    }

    #[test]
    fn lint_refuses_inside_a_run_echo_started() {
        // untrusted-text R2: a run Echo started can't start another Claude Code from the CLI.
        let (_env, root) = crate::paths::testenv::folders("kn-cli-lint-headless");
        let t = fixture::twin(&root.join("twin"));
        let v = knowledge::add(&t, knowledge::AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap();
        std::env::set_var(crate::notes::HEADLESS, "1");
        let r = lint_command(&v);
        std::env::remove_var(crate::notes::HEADLESS);
        assert!(r.unwrap_err().to_string().contains("Echo's own runs"));
        assert!(!knowledge::cache_dir(&v.id).join("lint").exists(), "nothing started");
    }
}
