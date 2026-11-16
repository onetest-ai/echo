//! Threads (projects), their work ledger, clients and organisations: `echoctl project | client | org`.

use anyhow::Result;
use clap::Subcommand;

use super::print_json;
#[cfg(feature = "brain")]
use crate::brain;
use crate::{calls, config, guard, ledger, notes, people, projects, projectsync, tasks, threadlinks, watch};

#[derive(Subcommand)]
pub enum ProjectCmd {
    /// Every project with its client, stage and call count
    List,
    /// One project, with its calls and why each belongs
    Show { id: String },
    /// A new project (stage active unless --stage)
    Create {
        name: String,
        #[arg(long)]
        client: Option<String>,
        #[arg(long)]
        stage: Option<String>,
    },
    /// Change a project's fields
    Set {
        id: String,
        #[arg(long)]
        name: Option<String>,
        /// A client id; "" for none
        #[arg(long)]
        client: Option<String>,
        /// lead, proposal, negotiation, kickoff, active, on_hold, wrap_up, done, lost, cancelled
        #[arg(long)]
        stage: Option<String>,
        /// Why it moved (kept with the stage change)
        #[arg(long, requires = "stage")]
        reason: Option<String>,
        /// YYYY-MM-DD: older calls don't belong; "" for none
        #[arg(long)]
        since: Option<String>,
        /// YYYY-MM-DD: its documents, posts and mail dated before it are history in the Brain; "" for none
        #[arg(long)]
        history_before: Option<String>,
        #[arg(long)]
        color: Option<String>,
        /// What Claude should know about it
        #[arg(long)]
        notes: Option<String>,
        /// The AI profile of its jobs; "" for the default
        #[arg(long)]
        profile: Option<String>,
        /// The model of its jobs; "" for the profile's minutes model
        #[arg(long)]
        model: Option<String>,
        /// The brief, from a file ("-" for stdin)
        #[arg(long)]
        brief: Option<std::path::PathBuf>,
        /// A symbol from the window's set (rocket, database, shield…); "" for its identicon
        #[arg(long)]
        icon: Option<String>,
        /// A logo image (PNG, JPEG or WebP, ≤ 512 KB); "" removes it
        #[arg(long)]
        logo: Option<std::path::PathBuf>,
    },
    /// Add a rule (one of --series, --people, --chat, --subject) or remove one by number (--remove)
    Rule {
        id: String,
        /// Every occurrence of this recurring meeting (its subject)
        #[arg(long)]
        series: Option<String>,
        /// Calls where half the other people are from these domains (none given: the client's)
        #[arg(long, num_args = 0.., value_delimiter = ',')]
        people: Option<Vec<String>>,
        /// Calls whose Teams chat is this chat id
        #[arg(long)]
        chat: Option<String>,
        /// Calls whose subject contains these words
        #[arg(long)]
        subject: Option<String>,
        /// Remove the rule with this number (1 = the first, as `show` lists them)
        #[arg(long)]
        remove: Option<usize>,
    },
    /// A source the project reads: --mail <domain, address or * (anyone)> (--subject "w1, w2": only mail whose
    /// subject, or its conversation's, contains one; * needs it), --channel <team id>|<channel id> (--name
    /// to show), --folder <path> (--exclude <glob>…: what in it not to read) into the Brain; --chat <chat
    /// id> (19:…) a meeting chat its work reads between calls; or --remove <kind:id> as `show` lists them
    #[cfg(feature = "brain")]
    Source {
        id: String,
        #[arg(long)]
        mail: Option<String>,
        /// With --mail: the subjects it reads ("Acme Platform, APX"); given again, they replace the source's
        #[arg(long)]
        subject: Option<String>,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        folder: Option<std::path::PathBuf>,
        #[arg(long)]
        chat: Option<String>,
        #[arg(long)]
        exclude: Vec<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        remove: Option<String>,
    },
    /// Put a call in the project by hand
    Include { id: String, call: String },
    /// Take a call out of the project (the rules won't put it back)
    Exclude { id: String, call: String },
    /// Leave a call to the rules again
    Reset { id: String, call: String },
    /// Add or remove glossary terms (they hint transcription on its calls)
    Term {
        id: String,
        terms: Vec<String>,
        /// What they are: system, term, acronym, milestone, deliverable…
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        remove: bool,
    },
    /// Add or remove a person (email, or a name)
    Person {
        id: String,
        name: String,
        #[arg(long)]
        email: Option<String>,
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        remove: bool,
    },
    /// The projects a call belongs to, and the ones it may belong to
    Of { call: String },
    /// Its people and organisations: who the calls show (calls, talk share, tasks), what you set
    People {
        id: String,
        /// Ask Microsoft 365 (the People API) about them first: title, department, company, office, phone
        #[arg(long)]
        refresh: bool,
    },
    /// Open tasks across open projects by who has to move (I owe, owed to me, waiting, nobody named), and asks by person
    Tasks,
    /// Projects Echo would make from calls in none (last 90 days); --accept, --merge or --reject one
    Found {
        /// Make this one (its key)
        #[arg(long, conflicts_with_all = ["merge", "reject"])]
        accept: Option<String>,
        /// Its name (with --accept)
        #[arg(long, requires = "accept")]
        name: Option<String>,
        /// Its client: an id, "new" for the one it proposes, "" for none (with --accept)
        #[arg(long, requires = "accept")]
        client: Option<String>,
        /// Add this one's rules to a project (with --into)
        #[arg(long, requires = "into", conflicts_with = "reject")]
        merge: Option<String>,
        #[arg(long)]
        into: Option<String>,
        /// Never suggest this one again
        #[arg(long)]
        reject: Option<String>,
    },
    /// Link two threads: --kind part-of (this one is part of --to) or related; --remove unlinks them
    Link {
        id: String,
        #[arg(long)]
        to: String,
        #[arg(long, value_parser = ["part-of", "related"], required_unless_present = "remove")]
        kind: Option<String>,
        #[arg(long, conflicts_with = "kind")]
        remove: bool,
    },
    /// Move a project to the trash (its calls are untouched)
    Delete { id: String },
    /// A thread's work as `ledger <id> tree` gives it, or one edit of it as `ledger <id> edit <json>` makes it (the shape
    /// Echo's sessions may run without a prompt)
    Work {
        id: String,
        /// One edit (JSON); none: the tree
        json: Option<String>,
    },
    /// The project's work ledger: stories, tasks, decisions, risks, questions from its calls
    Ledger {
        id: String,
        #[command(subcommand)]
        cmd: LedgerCmd,
    },
}

#[derive(Subcommand)]
pub enum LedgerCmd {
    /// Bring the evidence up to date with the project's calls (--limit: only the newest N); --chats also
    /// reads its meeting chats between calls, --mail its ✉ Mail sources (Microsoft 365)
    Sync {
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        chats: bool,
        #[arg(long)]
        mail: bool,
    },
    /// Mails as evidence (Graph message ids): one item a mail, read by the next sync like a call;
    /// --conversation adds the rest of each one's conversation
    Mail {
        #[arg(required = true)]
        ids: Vec<String>,
        #[arg(long)]
        conversation: bool,
    },
    /// Evidence not processed yet, oldest first
    Pending,
    /// The live tree as the extractor reads it
    Context,
    /// Apply proposals (a JSON array file) for one evidence item
    Apply {
        #[arg(long)]
        evidence: String,
        #[arg(long)]
        proposals: std::path::PathBuf,
    },
    /// Mark an evidence item processed (also when it changed nothing)
    Done {
        #[arg(long)]
        evidence: String,
        #[arg(long, default_value = "")]
        note: String,
    },
    /// The claims a verifier must judge (--weak: those judged weak too)
    VerifyPrep {
        #[arg(long)]
        weak: bool,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
    /// Apply verdicts (a JSON array file of {mid, verdict, reason})
    VerifyApply {
        #[arg(long)]
        file: std::path::PathBuf,
    },
    /// The people pass: the thread's people and organisations, these calls' files, what was proposed
    PeopleContext {
        /// Evidence ids (comma-separated)
        #[arg(long, value_delimiter = ',')]
        items: Vec<String>,
    },
    /// Propose changes about people for one evidence item (a JSON array file of {who, field, value, quote, to?, note?})
    PeopleApply {
        #[arg(long)]
        evidence: String,
        #[arg(long)]
        file: std::path::PathBuf,
    },
    /// The whole tree (JSON)
    Tree,
    /// Sync now: the calls into evidence, then Claude reads them in (extract, then verify, then people)
    Run,
    /// The last sync's state
    Status,
    /// An edit of yours (JSON: {op: edit|ac_add|ac_set|link|unlink|merge|delete|accept|reject|undo|create, …})
    Edit { json: String },
}

#[derive(Subcommand)]
pub enum ClientCmd {
    List,
    /// Its people and organisations across its threads: who the calls show, what you set
    People { id: String },
    Add {
        name: String,
        /// Mail domains (comma-separated)
        #[arg(long, value_delimiter = ',')]
        domains: Vec<String>,
        /// The user's own company
        #[arg(long)]
        internal: bool,
    },
    Set {
        id: String,
        #[arg(long)]
        name: Option<String>,
        /// Replaces the domains (comma-separated)
        #[arg(long, value_delimiter = ',')]
        domains: Option<Vec<String>>,
        #[arg(long)]
        internal: Option<bool>,
        /// A logo image (PNG, JPEG or WebP, ≤ 512 KB); "" removes it
        #[arg(long)]
        logo: Option<std::path::PathBuf>,
    },
    /// Remove a client without projects
    Remove { id: String },
}

#[derive(Subcommand)]
pub enum OrgCmd {
    /// Every organisation (`account: false` = not a client)
    List,
    /// A new organisation (not an account unless --account); --thread makes it that thread's counterpart
    Add {
        name: String,
        /// Mail domains (comma-separated)
        #[arg(long, value_delimiter = ',')]
        domains: Vec<String>,
        #[arg(long)]
        thread: Option<String>,
        /// A client: threads may sit under it
        #[arg(long)]
        account: bool,
    },
    Set {
        id: String,
        #[arg(long)]
        name: Option<String>,
        /// Replaces the domains (comma-separated)
        #[arg(long, value_delimiter = ',')]
        domains: Option<Vec<String>>,
        /// Replaces the other names (comma-separated)
        #[arg(long, value_delimiter = ',')]
        aliases: Option<Vec<String>>,
        /// The organisation it is part of (its id; "" for none)
        #[arg(long)]
        parent: Option<String>,
        /// A client or not (one with threads under it stays one)
        #[arg(long)]
        account: Option<bool>,
        /// A logo image (PNG, JPEG or WebP, ≤ 512 KB); "" removes it
        #[arg(long)]
        logo: Option<std::path::PathBuf>,
        /// Make it this thread's counterpart
        #[arg(long)]
        thread: Option<String>,
        /// No longer this thread's counterpart
        #[arg(long)]
        unlink: Option<String>,
    },
    /// Remove an organisation (an account only without threads); threads stop listing it
    Remove { id: String },
}

pub fn project_command(cmd: ProjectCmd) -> Result<()> {
    use projects::{Member, Rule, Stage, Term};
    if let ProjectCmd::Ledger { id, cmd } = cmd {
        return ledger_command(&id, cmd);
    }
    if let ProjectCmd::Work { id, json } = cmd {
        return ledger_command(&id, json.map_or(LedgerCmd::Tree, |json| LedgerCmd::Edit { json }));
    }
    // A run Echo started works on its own call (or project) only; projects are the user's. A read-only
    // routine (the user's own prompt) may read them.
    let reads = matches!(cmd, ProjectCmd::List | ProjectCmd::Show { .. } | ProjectCmd::Of { .. } | ProjectCmd::Tasks);
    // Yolo reads its own thread: its brief and rules, its people (as they are: no Microsoft 365 refresh).
    let own_thread_read = match &cmd {
        ProjectCmd::Show { id } | ProjectCmd::People { id, refresh: false } => guard::run_thread().is_some_and(|t| &t == id),
        _ => false,
    };
    if notes::run_limited() && !(reads && routine_reads()) && !own_thread_read {
        anyhow::bail!("projects can't be changed or listed from a run Echo started");
    }
    let empty = |s: String| if s.trim().is_empty() { None } else { Some(s) };
    let show = |p: &projects::Project| -> Result<()> {
        let mut v = projects::view(p, true)?;
        if notes::headless() {
            threadlinks::for_run(&mut v, &std::env::var(guard::RUN_PROFILE).unwrap_or_default());
        }
        print_json(&v);
        Ok(())
    };
    match cmd {
        ProjectCmd::List => print_json(&projects::overview()?),
        ProjectCmd::Link { id, to, kind, remove } => match kind {
            Some(k) if !remove => show(&threadlinks::link(&id, &to, threadlinks::Kind::parse(&k)?)?)?,
            _ => show(&threadlinks::unlink(&id, &to)?)?,
        },
        ProjectCmd::Show { id } => show(&projects::load(&id)?)?,
        ProjectCmd::Create { name, client, stage } => {
            let stage = stage.as_deref().map(Stage::parse).transpose()?;
            show(&projects::create(&name, client.as_deref(), stage)?)?
        }
        ProjectCmd::Set { id, name, client, stage, reason, since, history_before, color, notes, profile, model, brief, icon, logo } => {
            if let Some(f) = logo {
                projects::set_logo(&id, (!f.as_os_str().is_empty()).then_some(f.as_path()))?;
            }
            if let Some(f) = brief {
                let text = if f.as_os_str() == "-" { std::io::read_to_string(std::io::stdin())? } else { std::fs::read_to_string(&f)? };
                projects::set_brief(&id, &text)?;
            }
            if let Some(st) = stage {
                projects::set_stage(&id, Stage::parse(&st)?, reason.as_deref())?;
            }
            let p = projects::update(&id, |p| {
                if let Some(n) = name {
                    p.name = n.trim().to_string();
                }
                if let Some(c) = client {
                    p.client = empty(c);
                }
                if let Some(s) = since {
                    p.since = empty(s);
                }
                if let Some(s) = history_before {
                    p.history_before = empty(s);
                }
                if let Some(c) = color {
                    p.color = c;
                }
                if let Some(n) = notes {
                    p.notes = n;
                }
                if let Some(a) = profile {
                    p.ai_profile = empty(a);
                }
                if let Some(m) = model {
                    p.model = empty(m);
                }
                if let Some(i) = icon {
                    p.icon = empty(i);
                }
                Ok(())
            })?;
            show(&p)?
        }
        #[cfg(feature = "brain")]
        ProjectCmd::Source { id, mail, subject, channel, folder, chat, exclude, name, remove } => {
            anyhow::ensure!(subject.is_none() || mail.is_some(), "--subject goes with --mail");
            let subject = subject.as_deref().map(projects::subject_words);
            let add = match (mail, channel, folder) {
                _ if chat.is_some() => {
                    let c = chat.clone().unwrap_or_default().trim().to_string();
                    anyhow::ensure!(c.starts_with("19:") && !c.contains(char::is_whitespace), "--chat takes a Teams chat id (19:…@thread.v2)");
                    Some(("teams_chat".to_string(), c))
                }
                (Some(m), _, _) => Some(("mail".to_string(), brain::sources::mail_source(&m)?)),
                (None, Some(c), _) if c.split_once('|').is_some_and(|(a, b)| !a.is_empty() && !b.is_empty()) => Some(("teams_channel".to_string(), c)),
                (None, Some(_), _) => anyhow::bail!("--channel takes <team id>|<channel id>"),
                (None, None, Some(f)) => {
                    let f = dunce::canonicalize(&f).map_err(|e| anyhow::anyhow!("{}: {e}", f.display()))?;
                    anyhow::ensure!(f.is_dir(), "{} isn't a folder", f.display());
                    Some(("folder".to_string(), f.to_string_lossy().to_string()))
                }
                _ => None,
            };
            let mut changed: Option<String> = None;
            let p = projects::update(&id, |p| {
                if let Some(r) = &remove {
                    let before = p.sources.len();
                    p.sources.retain(|s| format!("{}:{}", s.kind, s.id) != *r);
                    if p.sources.len() == before {
                        anyhow::bail!("no source {r}");
                    }
                }
                if let Some((kind, sid)) = &add {
                    match p.sources.iter_mut().find(|s| &s.kind == kind && &s.id == sid) {
                        Some(s) => {
                            if !exclude.is_empty() {
                                s.exclude = exclude.clone();
                            }
                            if let Some(w) = subject.as_ref().filter(|w| **w != s.subject) {
                                s.subject = w.clone();
                                changed = Some(s.id.clone());
                            }
                        }
                        None => p.sources.push(projects::Source { kind: kind.clone(), id: sid.clone(), name: name.clone(), exclude: exclude.clone(), subject: subject.clone().unwrap_or_default() }),
                    }
                }
                Ok(())
            })?;
            // New words: the source reads its window again by them.
            if let Some(rule) = changed {
                ledger::read_mail_again(&id, &rule)?;
            }
            print_json(&serde_json::json!(p.sources));
        }
        ProjectCmd::Rule { id, series, people, chat, subject, remove } => {
            let p = projects::update(&id, |p| {
                if let Some(n) = remove {
                    if n == 0 || n > p.rules.len() {
                        anyhow::bail!("no rule {n} (it has {})", p.rules.len());
                    }
                    p.rules.remove(n - 1);
                }
                let mut add = vec![];
                if let Some(s) = series {
                    add.push(Rule::Series { subject: s, series_id: None });
                }
                if let Some(d) = people {
                    add.push(Rule::People { domains: d });
                }
                if let Some(c) = chat {
                    add.push(Rule::Chat { id: c.trim().to_string(), name: None });
                }
                if let Some(s) = subject {
                    add.push(Rule::Subject { contains: s });
                }
                for r in add {
                    if !p.rules.contains(&r) {
                        p.rules.push(r);
                    }
                }
                Ok(())
            })?;
            show(&p)?
        }
        ProjectCmd::Include { id, call } => show(&projects::set_membership(&id, &calls::resolve_call(&call)?, "include")?)?,
        ProjectCmd::Exclude { id, call } => show(&projects::set_membership(&id, &calls::resolve_call(&call)?, "exclude")?)?,
        ProjectCmd::Reset { id, call } => show(&projects::set_membership(&id, &calls::resolve_call(&call)?, "rules")?)?,
        ProjectCmd::Term { id, terms, kind, remove } => {
            let p = projects::update(&id, |p| {
                for t in terms.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
                    p.terms.retain(|x| !x.term.eq_ignore_ascii_case(t));
                    if !remove {
                        if !config::valid_term(t) {
                            anyhow::bail!("{t:?}: a term is 2–50 characters, at most 6 words");
                        }
                        p.terms.push(Term { term: t.to_string(), kind: kind.clone(), note: None });
                    }
                }
                Ok(())
            })?;
            show(&p)?
        }
        ProjectCmd::Person { id, name, email, role, remove } => {
            let p = projects::update(&id, |p| {
                let key = email.clone().unwrap_or_else(|| name.clone()).to_lowercase();
                p.people.retain(|m| m.email.clone().unwrap_or_else(|| m.name.clone()).to_lowercase() != key);
                if !remove {
                    p.people.push(Member { name: name.trim().to_string(), email: email.clone(), role: role.clone(), ..Default::default() });
                }
                Ok(())
            })?;
            show(&p)?
        }
        ProjectCmd::People { id, refresh } => {
            let today = chrono::Local::now().date_naive();
            if refresh {
                let v = people::of_thread(&id, today)?;
                let who: Vec<(String, String)> = v["people"].as_array().into_iter().flatten().filter_map(|p| Some((p["name"].as_str()?.to_string(), p["email"].as_str()?.to_string()))).collect();
                eprintln!("asked Microsoft 365 about {} people", people::refresh_m365(&who)?);
            }
            print_json(&people::of_thread(&id, today)?)
        }
        ProjectCmd::Tasks => {
            let now = chrono::Utc::now();
            print_json(&tasks::across(&watch::agenda(), chrono::Local::now().date_naive(), &now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))?);
        }
        ProjectCmd::Of { call } => {
            let dir = calls::resolve_call(&call)?;
            let of: Vec<serde_json::Value> = projects::of_call(&dir)?.into_iter().map(|(p, w)| serde_json::json!({"id": p.id, "name": p.name, "why": w})).collect();
            print_json(&serde_json::json!({"projects": of, "suggested": projects::suggest(&dir)?}));
        }
        ProjectCmd::Found { accept, name, client, merge, into, reject } => {
            if let Some(k) = accept {
                show(&projects::found_accept(&k, name.as_deref(), client.as_deref())?)?
            } else if let (Some(k), Some(into)) = (merge, into) {
                show(&projects::found_merge(&k, &into)?)?
            } else if let Some(k) = reject {
                projects::found_reject(&k)?;
                println!("{k}: not suggested again");
            } else {
                print_json(&serde_json::json!(projects::found()?));
            }
        }
        ProjectCmd::Ledger { .. } | ProjectCmd::Work { .. } => unreachable!(),
        ProjectCmd::Delete { id } => {
            projects::delete(&id)?;
            println!("{id} moved to the trash");
        }
    }
    Ok(())
}

/// Inside a read-only routine Echo started (`guard::RUN_MODE=routine`, set by the app, never by the
/// run), or a routine's run as the folder session (`notes::RUN_SESSION`): the user's own prompt may read projects,
/// clients and organisations.
fn routine_reads() -> bool {
    // Never a Yolo run's: its own task and thread are all it reads, whatever the user's choice.
    let yolo = std::env::var_os(guard::RUN_TASK).is_some() || std::env::var_os(guard::RUN_THREAD).is_some();
    !yolo && (std::env::var(guard::RUN_MODE).is_ok_and(|m| m == "routine") || notes::routine_session())
}

pub fn org_command(cmd: OrgCmd) -> Result<()> {
    if notes::run_limited() && !(matches!(cmd, OrgCmd::List) && routine_reads()) {
        anyhow::bail!("organisations can't be changed or listed from a run Echo started");
    }
    match cmd {
        OrgCmd::List => print_json(&serde_json::to_value(projects::orgs()?)?),
        OrgCmd::Add { name, domains, thread, account } => {
            let o = projects::add_org(&name, &domains, false, account)?;
            if let Some(t) = thread {
                people::link_org(&t, &o.id, true)?;
            }
            print_json(&serde_json::to_value(o)?)
        }
        OrgCmd::Set { id, name, domains, aliases, parent, account, logo, thread, unlink } => {
            if let Some(f) = logo {
                let bytes = if f.as_os_str().is_empty() { None } else { Some(std::fs::read(&f)?) };
                projects::set_client_logo(&id, bytes.as_deref())?;
            }
            let known = projects::orgs()?;
            if let Some(p) = parent.as_deref().filter(|p| !p.is_empty()) {
                if p == id || !known.iter().any(|c| c.id == p) {
                    anyhow::bail!("no other organisation {p:?}");
                }
            }
            let o = projects::update_client(&id, |c| {
                if let Some(n) = name {
                    c.name = n;
                }
                if let Some(d) = domains {
                    c.domains = d;
                }
                if let Some(a) = aliases {
                    c.aliases = a.into_iter().map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
                }
                if let Some(p) = parent {
                    c.parent = Some(p).filter(|p| !p.is_empty());
                }
                if let Some(a) = account {
                    c.account = a;
                }
                Ok(())
            })?;
            if let Some(t) = thread {
                people::link_org(&t, &id, true)?;
            }
            if let Some(t) = unlink {
                people::link_org(&t, &id, false)?;
            }
            print_json(&serde_json::to_value(o)?)
        }
        OrgCmd::Remove { id } => {
            projects::remove_client(&id)?;
            println!("{id} removed");
        }
    }
    Ok(())
}

pub fn client_command(cmd: ClientCmd) -> Result<()> {
    if notes::run_limited() && !(matches!(cmd, ClientCmd::List) && routine_reads()) {
        anyhow::bail!("clients can't be changed or listed from a run Echo started");
    }
    match cmd {
        ClientCmd::List => print_json(&serde_json::to_value(projects::clients()?)?),
        ClientCmd::People { id } => print_json(&people::of_client(&id, chrono::Local::now().date_naive())?),
        ClientCmd::Add { name, domains, internal } => print_json(&serde_json::to_value(projects::add_client(&name, &domains, internal)?)?),
        ClientCmd::Set { id, name, domains, internal, logo } => {
            if let Some(f) = logo {
                let bytes = if f.as_os_str().is_empty() { None } else { Some(std::fs::read(&f)?) };
                projects::set_client_logo(&id, bytes.as_deref())?;
            }
            let c = projects::update_client(&id, |c| {
                if let Some(n) = name {
                    c.name = n;
                }
                if let Some(d) = domains {
                    c.domains = d;
                }
                if let Some(i) = internal {
                    c.internal = i;
                }
                Ok(())
            })?;
            print_json(&serde_json::to_value(c)?)
        }
        ClientCmd::Remove { id } => {
            projects::remove_client(&id)?;
            println!("{id} removed");
        }
    }
    Ok(())
}

/// Is this ledger edit (`json`, on project `id`) Yolo closing its own task (`run_task`, `<project>/<node>`)? Its
/// state only (with a note) — never who does it or what Claude hands back, which would stand as the user's choice.
fn own_task_edit(id: &str, json: &str, run_task: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json).ok().is_some_and(|b| {
        b["op"] == "edit"
            && b["node"].as_str().is_some_and(|n| !run_task.is_empty() && run_task == format!("{id}/{n}"))
            && b["set"].as_object().is_some_and(|s| !s.is_empty() && s.keys().all(|k| k == "state"))
    })
}

/// Why a run Echo started can't take this ledger step (`edit`: the step was an edit). Yolo's (`run_task`,
/// `<project>/<node>`) is told the one edit it may make, so its model doesn't take the refusal for a passing one and
/// try again (#137); asked for anything else of the ledger, also where it reads its task.
fn run_refusal(run_task: &str, edit: bool) -> String {
    match run_task.split_once('/') {
        Some((project, node)) => {
            let may = format!(
                "`echoctl project ledger {project} edit '{{\"op\":\"edit\",\"node\":\"{node}\",\"set\":{{\"state\":\"done\"}}}}'` \
                 (or in_progress, blocked, cancelled)"
            );
            if edit {
                format!(
                    "Yolo may change only its own task's state: {may} — not its description, owners or any other item; \
                     trying again won't change that, say the rest in your answer"
                )
            } else {
                format!(
                    "Yolo reads its task with `echoctl read echo://task/{project}/{node}` and its thread's work with \
                     `echoctl project ledger {project} tree` — no other thread's; of the ledger it may only edit that task's state: {may}"
                )
            }
        }
        // Yolo on an item of Needs you now: its thread's work to read, none of it to change.
        None => match guard::run_thread() {
            Some(t) => format!("Yolo reads its thread's work (`echoctl project ledger {t} tree`) and changes none of it; no other thread's"),
            None => "a run Echo started works only on its own project's ledger, with the agent's steps".into(),
        },
    }
}

fn ledger_command(id: &str, cmd: LedgerCmd) -> Result<()> {
    // Inside a run Echo started: the agent's steps, on that run's project only (a read-only routine:
    // the tree).
    // Yolo's run: an edit of its own task (`guard::RUN_TASK`), nothing else of the ledger's.
    let own_task = match &cmd {
        LedgerCmd::Edit { json } => own_task_edit(id, json, &std::env::var(guard::RUN_TASK).unwrap_or_default()),
        _ => false,
    };
    // Yolo reads its own thread's work (the tree, the last sync's state).
    let own_thread_read = matches!(cmd, LedgerCmd::Tree | LedgerCmd::Status) && guard::run_thread().is_some_and(|t| t == id);
    // A routine's run (as the folder session) edits a thread's work: Echo's own data, with or without prompts.
    let routine_edit = matches!(cmd, LedgerCmd::Edit { .. }) && notes::routine_session();
    let allowed = own_task || own_thread_read || routine_edit || (matches!(cmd, LedgerCmd::Tree) && routine_reads());
    if notes::run_limited() && !allowed {
        let own = std::env::var(guard::RUN_PROJECT).unwrap_or_default();
        let agent_step = matches!(cmd, LedgerCmd::Pending | LedgerCmd::Context | LedgerCmd::Apply { .. } | LedgerCmd::Done { .. } | LedgerCmd::VerifyPrep { .. } | LedgerCmd::VerifyApply { .. } | LedgerCmd::PeopleContext { .. } | LedgerCmd::PeopleApply { .. });
        if own.is_empty() || own != id || !agent_step {
            anyhow::bail!("{}", run_refusal(&std::env::var(guard::RUN_TASK).unwrap_or_default(), matches!(cmd, LedgerCmd::Edit { .. })));
        }
    }
    let read = |p: &std::path::Path| -> Result<Vec<serde_json::Value>> {
        // A run reads its inputs only from its own folder (not another project's files).
        if notes::headless() {
            let run = std::env::var_os(guard::RUN_DIR).and_then(|d| dunce::canonicalize(d).ok());
            let parent = dunce::canonicalize(p).ok().and_then(|f| f.parent().map(std::path::Path::to_path_buf));
            if run.is_none() || parent != run {
                anyhow::bail!("a run reads its files only from its own folder");
            }
        }
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(p)?).map_err(|e| anyhow::anyhow!("{}: not JSON ({e})", p.display()))?;
        v.as_array().cloned().ok_or_else(|| anyhow::anyhow!("{}: expected a JSON array", p.display()))
    };
    match cmd {
        LedgerCmd::Sync { limit, chats, mail } => {
            let mut written = ledger::sync(id, limit)?;
            if chats {
                written.extend(ledger::chat_sync(id)?);
            }
            if mail {
                written.extend(ledger::mail_sync(id)?);
            }
            print_json(&serde_json::json!({ "written": written, "pending": ledger::pending(id)?, "chats": ledger::chat_state(id), "mail": ledger::mail_state(id) }))
        }
        LedgerCmd::Mail { ids, conversation } => print_json(&serde_json::json!({ "mail": ledger::add_mail(id, &ids, conversation)?, "pending": ledger::pending(id)? })),
        LedgerCmd::Pending => print_json(&serde_json::json!(ledger::pending(id)?)),
        LedgerCmd::Context => print_json(&ledger::context(id)?),
        LedgerCmd::Apply { evidence, proposals } => print_json(&ledger::apply(id, &evidence, &read(&proposals)?)?),
        LedgerCmd::Done { evidence, note } => {
            ledger::mark_processed(id, &evidence, &note)?;
            print_json(&serde_json::json!({ "done": evidence }));
        }
        LedgerCmd::VerifyPrep { weak, out } => {
            // A run writes only into its own folder.
            if let (true, Some(f)) = (notes::headless(), &out) {
                let run = std::env::var_os(guard::RUN_DIR).and_then(|d| dunce::canonicalize(d).ok());
                let parent = f.parent().and_then(|d| dunce::canonicalize(d).ok());
                if run.is_none() || parent != run {
                    anyhow::bail!("a run writes its packets only into its own folder");
                }
            }
            let packets = serde_json::json!(ledger::verify_prep(id, weak)?);
            match out {
                Some(f) => {
                    std::fs::write(&f, serde_json::to_vec_pretty(&packets)?)?;
                    // How the user works (About me), for judging what counts as done or handed over.
                    let me = calls::me_profile().and_then(|t| calls::me_parts(&t, &["identity", "patterns"], 2500));
                    // What this thread is about, for packets from an item it shares with other threads (`also_in`).
                    let shared = packets.as_array().is_some_and(|a| a.iter().any(|p| p.get("also_in").is_some()));
                    let scope = shared.then(|| ledger::thread_scope(id)).flatten();
                    print_json(&serde_json::json!({ "packets": packets.as_array().map(|a| a.len()), "file": f, "me_patterns": me, "thread_scope": scope }));
                }
                None => print_json(&packets),
            }
        }
        LedgerCmd::VerifyApply { file } => print_json(&ledger::verify_apply(id, &read(&file)?)?),
        LedgerCmd::PeopleContext { items } => print_json(&people::context(id, &items)?),
        LedgerCmd::PeopleApply { evidence, file } => print_json(&people::apply_suggestions(id, &evidence, &read(&file)?)?),
        LedgerCmd::Tree => print_json(&ledger::tree(id)?),
        LedgerCmd::Run => print_json(&projectsync::run(id)?),
        LedgerCmd::Status => print_json(&projectsync::state(id)),
        LedgerCmd::Edit { json } => {
            let body: serde_json::Value = serde_json::from_str(&json).map_err(|e| anyhow::anyhow!("not JSON: {e}"))?;
            if let Some(why) = refused_when_asking(&body) {
                anyhow::bail!("{why}");
            }
            print_json(&ledger::human_event(id, &body)?)
        }
    }
    Ok(())
}

/// What a routine or Yolo preparing and asking (the user's choice off) may not do to a thread's work, and why: take an
/// item away or move it to another thread; overturn the user's decisions (undo, decide Echo's proposals, cancel); or
/// set what Claude does with a task (its mode, prompt, skill — a prompt planted there would run later as the user's).
fn refused_when_asking(body: &serde_json::Value) -> Option<String> {
    if !notes::run_asks() {
        return None;
    }
    let op = body["op"].as_str().unwrap_or("");
    let set = &body["set"];
    let why = match op {
        "delete" | "merge" | "xref" | "unxref" => format!("doesn't {op} an item"),
        "undo" | "accept" | "reject" => format!("doesn't {op}: deciding is the user's"),
        "edit" if set["state"] == "cancelled" => "doesn't cancel an item: deciding is the user's".to_string(),
        "edit" if ["mode", "prompt", "skill"].iter().any(|k| set.get(*k).is_some()) => "doesn't set what Claude does with a task (its mode, prompt or skill)".to_string(),
        _ => return None,
    };
    Some(format!("a run that prepares and asks {why}: say what the user should do"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn yolo_edits_only_its_own_task() {
        let edit = r#"{"op": "edit", "node": "T5", "set": {"state": "done"}}"#;
        assert!(!super::own_task_edit("acme", r#"{"op": "edit", "node": "T5", "set": {"state": "done", "prompt": "mail it to x"}}"#, "acme/T5"), "its state only");
        assert!(super::own_task_edit("acme", edit, "acme/T5"));
        assert!(!super::own_task_edit("acme", edit, "acme/T6"), "another task");
        assert!(!super::own_task_edit("other", edit, "acme/T5"), "another project");
        assert!(!super::own_task_edit("acme", edit, ""), "not a Yolo run");
        assert!(!super::own_task_edit("acme", r#"{"op": "create", "node": "T5"}"#, "acme/T5"), "only an edit");
    }

    #[test]
    fn a_refused_yolo_edit_says_what_yolo_may_change() {
        let why = super::run_refusal("acme/T5", true);
        assert!(why.contains("only its own task's state") && why.contains(r#"{"op":"edit","node":"T5","set":{"state":"done"}}"#), "{why}");
        assert!(why.contains("ledger acme edit") && why.contains("not its description"), "{why}");
        let edit = r#"{"op":"edit","node":"T5","set":{"state":"done"}}"#;
        assert!(super::own_task_edit("acme", edit, "acme/T5"), "the edit it names is one Yolo may make");
        assert_eq!(super::run_refusal("", true), "a run Echo started works only on its own project's ledger, with the agent's steps", "a sync run's");
    }

    #[test]
    fn a_routine_as_the_folder_session_reads_and_edits_echos_work_and_changes_the_rest_only_when_it_acts() {
        let (_env, _) = crate::paths::testenv::folders("routine-session-cli");
        let pilot = crate::projects::create("Pilot", None, None).unwrap();
        struct Unset;
        impl Drop for Unset {
            fn drop(&mut self) {
                for k in [crate::notes::HEADLESS, crate::notes::RUN_SESSION, super::guard::RUN_TASK] {
                    std::env::remove_var(k);
                }
            }
        }
        let _unset = Unset;
        std::env::set_var(crate::notes::HEADLESS, "1");
        let edit = || super::ledger_command(&pilot.id, super::LedgerCmd::Edit { json: r#"{"op":"create","kind":"epic","title":"Pilot work"}"#.into() });
        let show = || super::project_command(super::ProjectCmd::Show { id: pilot.id.clone() });
        let create = || super::project_command(super::ProjectCmd::Create { name: "Another".into(), client: None, stage: None });
        // Prepares and asks: reads and a thread's work, not the rest — and of the work never a removal or a move away.
        std::env::set_var(crate::notes::RUN_SESSION, "asks");
        assert!(show().is_ok(), "it reads");
        edit().unwrap();
        let tree = crate::ledger::tree(&pilot.id).unwrap();
        let epic = tree["nodes"].as_object().unwrap().keys().next_back().unwrap().clone();
        let delete = || super::ledger_command(&pilot.id, super::LedgerCmd::Edit { json: serde_json::json!({"op": "delete", "node": epic}).to_string() });
        assert!(delete().unwrap_err().to_string().contains("doesn't delete an item"));
        assert!(create().is_err(), "a new thread isn't a thread's work");
        // Its edits are Echo's run's, not the user's.
        let evs = std::fs::read_to_string(crate::paths::home().join("projects").join(&pilot.id).join("ledger/events.jsonl")).unwrap();
        assert!(evs.lines().filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok()).any(|e| e["by"] == "run"), "{evs}");
        assert!(!evs.contains("\"by\":\"human\""), "{evs}");
        // Acts on its own: as the folder session.
        std::env::set_var(crate::notes::RUN_SESSION, "acts");
        delete().unwrap();
        create().unwrap();
        // Yolo, acting on its own, keeps its own task's limit.
        std::env::set_var(super::guard::RUN_TASK, format!("{}/T001", pilot.id));
        assert!(edit().unwrap_err().to_string().contains("only its own task"));
        // Any other run Echo started (minutes, a sync): none of it.
        std::env::remove_var(super::guard::RUN_TASK);
        std::env::remove_var(crate::notes::RUN_SESSION);
        assert!(show().is_err() && edit().is_err());
    }

    #[test]
    fn a_run_preparing_and_asking_keeps_yolos_limits_and_leaves_the_users_decisions_and_claudes_prompts_alone() {
        let (_env, _) = crate::paths::testenv::folders("routine-asks-limits");
        let pilot = crate::projects::create("Pilot", None, None).unwrap();
        let other = crate::projects::create("Rollout", None, None).unwrap();
        struct Unset;
        impl Drop for Unset {
            fn drop(&mut self) {
                for k in [crate::notes::HEADLESS, crate::notes::RUN_SESSION, super::guard::RUN_MODE, super::guard::RUN_TASK, super::guard::RUN_THREAD] {
                    std::env::remove_var(k);
                }
            }
        }
        let _unset = Unset;
        let ed = |id: &str, j: serde_json::Value| super::ledger_command(id, super::LedgerCmd::Edit { json: j.to_string() });
        // The user's own work: an epic, a story, a task.
        ed(&pilot.id, serde_json::json!({"op": "create", "kind": "epic", "title": "Pilot work"})).unwrap();
        let ids = |kind: &str| -> Vec<String> {
            let t = crate::ledger::tree(&pilot.id).unwrap();
            t["nodes"].as_object().unwrap().iter().filter(|(_, n)| n["kind"] == kind).map(|(k, _)| k.clone()).collect()
        };
        let epic = ids("epic").pop().unwrap();
        ed(&pilot.id, serde_json::json!({"op": "create", "kind": "story", "parent": epic, "title": "SSO"})).unwrap();
        let story = ids("story").pop().unwrap();
        ed(&pilot.id, serde_json::json!({"op": "create", "kind": "task", "parent": story, "title": "Mail the runbook"})).unwrap();
        let task = ids("task").pop().unwrap();
        let last_mid = || {
            let evs = std::fs::read_to_string(crate::paths::home().join("projects").join(&pilot.id).join("ledger/events.jsonl")).unwrap();
            let e: serde_json::Value = serde_json::from_str(evs.lines().last().unwrap()).unwrap();
            (e["mid"].as_str().unwrap().to_string(), e["by"].clone())
        };
        let (mid, _) = last_mid();
        std::env::set_var(crate::notes::HEADLESS, "1");

        // 1. Yolo with the user's choice off (`RUN_MODE=routine`): still its own task's thread only, in every read.
        std::env::set_var(crate::notes::RUN_SESSION, "asks");
        std::env::set_var(super::guard::RUN_MODE, "routine");
        std::env::set_var(super::guard::RUN_TASK, format!("{}/{task}", pilot.id));
        let work = |id: &str| super::project_command(super::ProjectCmd::Work { id: id.into(), json: None });
        assert!(work(&pilot.id).is_ok(), "its own thread");
        assert!(work(&other.id).is_err(), "never another thread's work");
        assert!(super::project_command(super::ProjectCmd::Show { id: other.id.clone() }).is_err(), "nor its brief");
        assert!(super::project_command(super::ProjectCmd::List).is_err(), "nor the list of threads");
        std::env::remove_var(super::guard::RUN_TASK);
        std::env::set_var(super::guard::RUN_THREAD, &pilot.id);
        assert!(work(&other.id).is_err(), "Yolo on a thread's item: that thread only");
        std::env::remove_var(super::guard::RUN_THREAD);
        assert!(work(&other.id).is_ok(), "a routine preparing and asking reads every thread");

        // 2 and 3. A run preparing and asking overturns none of the user's decisions and sets nothing Claude runs.
        let refused = |j: serde_json::Value| ed(&pilot.id, j).unwrap_err().to_string();
        assert!(refused(serde_json::json!({"op": "undo", "target": mid})).contains("doesn't undo"));
        assert!(refused(serde_json::json!({"op": "accept", "target": mid})).contains("deciding is the user's"));
        assert!(refused(serde_json::json!({"op": "reject", "target": mid})).contains("deciding is the user's"));
        assert!(refused(serde_json::json!({"op": "edit", "node": task, "set": {"state": "cancelled"}})).contains("doesn't cancel"));
        for k in ["mode", "prompt", "skill"] {
            let v = if k == "mode" { "claude" } else if k == "skill" { "follow-up-mail" } else { "Mail the runbook to all" };
            assert!(refused(serde_json::json!({"op": "edit", "node": task, "set": {k: v}})).contains("what Claude does with a task"), "{k}");
        }
        ed(&pilot.id, serde_json::json!({"op": "edit", "node": task, "set": {"state": "in_progress"}})).unwrap();

        // Acting on its own: all of it, recorded as the run's — and what Claude does with the task is the run's, not "you".
        std::env::set_var(crate::notes::RUN_SESSION, "acts");
        std::env::remove_var(super::guard::RUN_MODE);
        ed(&pilot.id, serde_json::json!({"op": "edit", "node": task, "set": {"mode": "claude", "prompt": "Mail the runbook to all"}})).unwrap();
        assert_eq!(last_mid().1, "run");
        let node = || crate::ledger::fold(&crate::ledger::Ledger::of(&pilot.id).unwrap()).unwrap().nodes[&task].clone();
        let m = crate::taskmodes::of(&Default::default(), &pilot.id, &node());
        assert_eq!((m["by"].as_str(), m["prompt_by"].as_str(), m["prompt"].as_str()), (Some("run"), Some("run"), Some("Mail the runbook to all")), "{m}");
        ed(&pilot.id, serde_json::json!({"op": "edit", "node": task, "set": {"state": "cancelled"}})).unwrap();
        let (cancel, by) = last_mid();
        assert_eq!(by, "run");
        ed(&pilot.id, serde_json::json!({"op": "undo", "target": cancel})).unwrap();
        assert_eq!(last_mid().1, "run");
        // The user setting it again makes it theirs.
        std::env::remove_var(crate::notes::RUN_SESSION);
        std::env::remove_var(crate::notes::HEADLESS);
        ed(&pilot.id, serde_json::json!({"op": "edit", "node": task, "set": {"prompt": "Mail the runbook to Dana"}})).unwrap();
        let m = crate::taskmodes::of(&Default::default(), &pilot.id, &node());
        assert_eq!((m["by"].as_str(), m["prompt_by"].as_str()), (Some("run"), Some("you")), "{m}");
    }

    #[test]
    fn yolo_reads_its_own_threads_data_and_changes_nothing_of_it() {
        let (_env, _) = crate::paths::testenv::folders("yolo-thread-reads");
        let mine = crate::projects::create("Pilot", None, None).unwrap();
        let other = crate::projects::create("Rollout", None, None).unwrap();
        std::env::set_var(crate::notes::HEADLESS, "1");
        let tree = |id: &str| super::ledger_command(id, super::LedgerCmd::Tree);
        let show = |id: &str| super::project_command(super::ProjectCmd::Show { id: id.into() });
        let people = |id: &str, refresh| super::project_command(super::ProjectCmd::People { id: id.into(), refresh });
        // Yolo on an item of Needs you now: its thread by RUN_THREAD.
        std::env::set_var(super::guard::RUN_THREAD, &mine.id);
        let (t, s, pp) = (tree(&mine.id).is_ok(), show(&mine.id).is_ok(), people(&mine.id, false).is_ok());
        let (t2, s2) = (tree(&other.id).is_err(), show(&other.id).is_err());
        let refresh = people(&mine.id, true).is_err();
        let other_by_item = tree(&other.id).unwrap_err().to_string();
        let edit = super::ledger_command(&mine.id, super::LedgerCmd::Edit { json: r#"{"op":"create","kind":"task","title":"x"}"#.into() }).is_err();
        std::env::remove_var(super::guard::RUN_THREAD);
        // A task's Yolo: its task's thread.
        std::env::set_var(super::guard::RUN_TASK, format!("{}/T005", mine.id));
        let by_task = tree(&mine.id).is_ok();
        let other_by_task = tree(&other.id).unwrap_err().to_string();
        let title = r#"{"op":"edit","node":"T005","set":{"title":"x"}}"#.to_string();
        let title = super::ledger_command(&mine.id, super::LedgerCmd::Edit { json: title }).unwrap_err().to_string();
        std::env::remove_var(super::guard::RUN_TASK);
        // Any other run Echo started: none of it.
        let none = tree(&mine.id).is_err() && show(&mine.id).is_err();
        std::env::remove_var(crate::notes::HEADLESS);
        assert!(t && s && pp, "its own thread: the work, the brief, the people");
        assert!(t2 && s2, "never another thread");
        assert!(other_by_item.contains(&format!("project ledger {} tree", mine.id)) && other_by_item.contains("changes none of it"), "{other_by_item}");
        assert!(refresh, "no Microsoft 365 refresh of its people");
        assert!(edit, "nothing of the work changes");
        assert!(by_task, "a task's Yolo reads its task's thread");
        assert!(other_by_task.contains("echoctl read echo://task/") && !other_by_task.contains("not its description"), "another thread is refused, pointing to its task: {other_by_task}");
        assert!(title.contains("only its own task's state") && title.contains("not its description"), "{title}");
        assert!(none, "a run with no thread reads none");
    }
}
