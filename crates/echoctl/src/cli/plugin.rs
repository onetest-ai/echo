//! Echo plugins and their marketplaces: `echoctl plugin …`.

use anyhow::Result;
use clap::Subcommand;

use super::print_json;
#[cfg(feature = "js")]
use crate::jshost;
use crate::plugins;

/// What `echoctl plugin guide` prints: the authoring rules, then the API reference they point to.
const GUIDE: &str = concat!(
    include_str!("../../../../.ai-run/guides/plugins/plugin-authoring.md"),
    "\n",
    include_str!("../../../../.ai-run/reference/plugins/plugin-api.md"),
);

#[derive(Subcommand)]
pub enum PluginCmd {
    /// Installed plugins, what the marketplaces offer, and the marketplaces (JSON)
    List,
    /// Install a plugin (shows what it uses; --yes agrees). A folder (a path: `./pulse`, `.`) is copied into My plugins
    /// first, then installed from there; the same folder again is its update
    Install {
        /// A plugin's id, or a path to a folder with a plugin
        id: String,
        /// The marketplace, when more than one offers it
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    /// Update a plugin (or all) to its marketplace's newer version; more permissions need --yes
    Update {
        id: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    /// Remove a plugin (its settings and data stay unless --delete-data)
    Remove {
        id: String,
        #[arg(long)]
        delete_data: bool,
    },
    Enable { id: String },
    Disable { id: String },
    /// Show (no value) or set a plugin's setting
    Set { id: String, key: String, value: Option<String> },
    /// The certificates you trust for a plugin's https / wss addresses, and those Echo met there and refused (JSON)
    Certs { id: String },
    /// Trust the certificate at an address you listed for a plugin (host:port): the one Echo met there, or the key you
    /// give (sha256//… of its public key; needed once Echo met more than one key there); Echo then reaches it with that
    /// key alone and refuses another
    Trust {
        id: String,
        address: String,
        #[arg(long)]
        key: Option<String>,
    },
    /// No longer trust the certificate you trusted at an address (host:port)
    Untrust { id: String, address: String },
    /// Run a plugin's command (its host half's `ctx.command`) with JSON arguments; prints its JSON answer
    Call {
        id: String,
        command: String,
        /// JSON arguments (default {})
        args: Option<String>,
        /// A file handed to the command, as a handle it reads or copies (repeatable). Refused inside a run Echo started —
        /// a minutes run, a thread's sync, Yolo — but not a routine
        #[arg(long = "file")]
        files: Vec<std::path::PathBuf>,
    },
    /// Run a plugin's declared script (`uses.scripts`) as Echo runs it: its output, its exit status (124 when its time ran out)
    Script {
        id: String,
        skill: String,
        script: String,
        /// Its arguments (after `--`)
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// My plugins: the folder your own plugins go in (made if missing), and what Echo reads there (JSON)
    Mine,
    /// Check plugin package folders as an install would (manifest, uses, widgets, skills, scripts, agent/), for plugin
    /// authors: each problem by file; exit 1 when there is one. Needs no data folder, nothing installed, no network
    Check {
        #[arg(required = true)]
        folders: Vec<std::path::PathBuf>,
        /// JSON: {ok, problems: [{id, file, what}], plugins: [{id, folder, ok}]}
        #[arg(long)]
        json: bool,
    },
    /// The plugin authoring rules, then the plugin API reference (Markdown), as this Echo has them: manifest, widgets
    /// and their data, host and window halves, testing, publishing
    Guide,
    /// Debug builds: queue an event (`meeting_soon`, …) with a JSON payload, as Echo would tell it, for the running app
    /// or a served window to deliver to the plugins that hear it — testing a plugin on test data (after `. <dir>/env`:
    /// without it, it goes to the Echo you use)
    #[command(hide = !cfg!(debug_assertions))]
    Emit {
        event: String,
        /// JSON object (default {})
        payload: Option<String>,
        /// Build the payload as Echo does instead: from a call's folder (`minutes_written`) or a Recall source
        /// (`sources_changed`: `mail/…`, `channel/…`)
        #[arg(long, conflicts_with = "payload")]
        from: Option<String>,
    },
    /// Marketplaces: list | add <owner/repo | git URL | folder> [--name] | remove <name> | update [<name>]
    Marketplace {
        #[command(subcommand)]
        cmd: MarketCmd,
    },
}

#[derive(Subcommand)]
pub enum MarketCmd {
    List,
    Add {
        source: String,
        #[arg(long)]
        name: Option<String>,
    },
    /// Remove a marketplace and the plugins installed from it (listed; --yes when there are any; their data kept
    /// unless --delete-data)
    Remove {
        name: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        delete_data: bool,
    },
    /// Leave a marketplace out of Discover (the built-in one can't be removed, only hidden)
    Hide { name: String },
    Show { name: String },
    /// Fetch again (all, or one)
    Update { name: Option<String> },
}

/// Why a run Echo started can't change plugins.
const REFUSED: &str = "plugins are installed, updated and changed by you, not from inside a run Echo started";
/// Why a plugin's script can't change plugins (Echo sets `ECHO_PLUGIN_SCRIPT` for every script it runs).
const SCRIPT_REFUSED: &str = "plugins are installed, updated and changed by you, not from a plugin's script";

/// Plugins are installed, updated, set up, switched on and off and removed by the user, and so are their marketplaces:
/// never from inside a run Echo started, a routine acting on its own too (a setting may list addresses a plugin reaches),
/// nor from a plugin's script (it would widen its own reach).
fn refuse_in_run(cmd: &PluginCmd) -> Result<()> {
    let changes = matches!(
        cmd,
        PluginCmd::Install { .. }
            | PluginCmd::Update { .. }
            | PluginCmd::Remove { .. }
            | PluginCmd::Enable { .. }
            | PluginCmd::Disable { .. }
            | PluginCmd::Set { value: Some(_), .. }
            | PluginCmd::Trust { .. }
            | PluginCmd::Untrust { .. }
            | PluginCmd::Marketplace { cmd: MarketCmd::Add { .. } | MarketCmd::Remove { .. } | MarketCmd::Hide { .. } | MarketCmd::Show { .. } }
    );
    if changes && crate::notes::headless() {
        anyhow::bail!(REFUSED);
    }
    if changes && std::env::var_os("ECHO_PLUGIN_SCRIPT").is_some() {
        anyhow::bail!(SCRIPT_REFUSED);
    }
    Ok(())
}

pub fn plugin_command(cmd: PluginCmd) -> Result<()> {
    use plugins::*;
    // Install / update ask in the terminal's own words; --yes is the answer.
    let agree = |r: &Review, yes: bool| -> Result<Vec<String>> {
        print_review(r);
        if let Some(b) = &r.blocked {
            anyhow::bail!("{} can't be installed: {b}", r.name);
        }
        if !yes && !(r.new_uses.is_empty()) {
            anyhow::bail!("run again with --yes to agree");
        }
        Ok(r.uses.clone())
    };
    refuse_in_run(&cmd)?;
    match cmd {
        PluginCmd::List => print_json(&snapshot()),
        PluginCmd::Install { id, from, yes } => {
            let mut from = from.map(|f| market_named(&f)).transpose()?;
            // A folder (a path: `./pulse`, `/…`, `.`): copied into My plugins, installed from there (#589). A bare name is
            // always an id.
            let folder = std::path::Path::new(&id);
            let id = if id.contains('/') || id.contains('\\') || id == "." || id == ".." {
                if from.as_deref().is_some_and(|f| f != MINE) {
                    anyhow::bail!("a folder installs from My plugins, not from {}", from.unwrap_or_default());
                }
                let (id, copy, copied) = plugins::mine::copy_into_mine(folder)?;
                if copied {
                    println!("Copied {} into My plugins ({})", dunce::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf()).display(), copy.display());
                }
                from = Some(MINE.to_string());
                id
            } else {
                id
            };
            let r = review(&id, from.as_deref())?;
            let agreed = agree(&r, yes)?;
            let p = install(&id, from.as_deref(), &agreed)?;
            println!("Installed {} {}", p.name, p.version);
        }
        PluginCmd::Update { id, yes } => {
            let ids: Vec<String> = match id {
                Some(i) => vec![i],
                None => Registry::load()?.installed.into_keys().collect(),
            };
            for id in ids {
                let p = Registry::load()?.installed.get(&id).cloned().ok_or_else(|| anyhow::anyhow!("“{id}” isn't installed"))?;
                refresh(&p.market)?;
                let Some(r) = update_for(&p) else {
                    println!("{} {} is up to date", p.name, p.version);
                    continue;
                };
                let agreed = agree(&r, yes)?;
                if let Some(n) = update(&id, &agreed)? {
                    println!("Updated {} to {}", n.name, n.version);
                }
            }
        }
        PluginCmd::Remove { id, delete_data } => remove(&id, !delete_data)?,
        PluginCmd::Enable { id } => set_enabled(&id, true)?,
        PluginCmd::Disable { id } => set_enabled(&id, false)?,
        PluginCmd::Set { id, key, value: None } => print_json(&serde_json::json!(settings_view(&id)?.into_iter().find(|s| s["key"] == key.as_str()))),
        PluginCmd::Set { id, key, value: Some(v) } => {
            set_setting(&id, &key, &serde_json::Value::String(v))?;
            // An address list: what the user should know about what they listed, as the window's row says it.
            for n in settings_view(&id)?.into_iter().find(|s| s["key"] == key.as_str()).and_then(|s| s["notes"].as_array().cloned()).unwrap_or_default() {
                eprintln!("Note: {}", n.as_str().unwrap_or(""));
            }
        }
        PluginCmd::Certs { id } => print_json(&serde_json::json!(pins::view(&id)?)),
        PluginCmd::Trust { id, address, key } => {
            let key = pins::trust(&id, &address, key.as_deref())?;
            println!("Trusted {} for {id}: {key}", pins::host_port(&address)?);
        }
        PluginCmd::Untrust { id, address } => {
            let hp = pins::host_port(&address)?;
            println!("{}", if pins::forget(&id, &address)? { format!("No longer trusted at {hp} for {id}") } else { format!("Nothing was trusted at {hp} for {id}") });
        }
        #[cfg(feature = "js")]
        PluginCmd::Call { id, command, args, files } => {
            let args: serde_json::Value = serde_json::from_str(args.as_deref().unwrap_or("{}")).map_err(|e| anyhow::anyhow!("the arguments aren't JSON: {e}"))?;
            let handles = file_handles(&id, &files)?;
            // A terminal's host runs commands, never the plugins' timers: those are the app's.
            jshost::no_timers();
            // `call_with` loads this plugin only.
            print_json(&jshost::call_with(&id, &command, &args, &handles, std::time::Duration::from_secs(3600))?);
        }
        #[cfg(not(feature = "js"))]
        PluginCmd::Call { .. } => anyhow::bail!("this build of Echo runs no plugin code"),
        PluginCmd::Script { id, skill, script, args } => {
            let argv: Vec<String> = std::env::args().skip(1).collect();
            let v = match crate::plugins::scripts::run(&id, &skill, &script, &args, None) {
                Ok(v) => v,
                Err(e) => {
                    crate::notes::note_action(&argv, Some(&format!("failed: {e:#}")));
                    return Err(e);
                }
            };
            // It exits below with the script's own status: noted now, inside a routine's or Yolo's run.
            crate::notes::note_action(&argv, Some(&match v["status"].as_i64() {
                Some(0) => "exit 0".to_string(),
                Some(s) => format!("exit {s}"),
                None => "stopped".to_string(),
            }));
            use std::io::Write;
            let _ = std::io::stdout().write_all(v["stdout"].as_str().unwrap_or("").as_bytes());
            let _ = std::io::stderr().write_all(v["stderr"].as_str().unwrap_or("").as_bytes());
            if v["truncated"] == true {
                eprintln!("(Echo kept the first 1 MB of its output)");
            }
            if v["timed_out"] == true {
                eprintln!("(stopped: {skill}/{script} took longer than it may)");
                std::process::exit(124);
            }
            let _ = std::io::stdout().flush();
            std::process::exit(v["status"].as_i64().map(|s| s as i32).unwrap_or(1));
        }
        PluginCmd::Mine => {
            if !crate::paths::home_chosen() {
                anyhow::bail!("choose Echo's data folder first (Settings → Data): your plugins go in it");
            }
            std::fs::create_dir_all(mine_dir())?;
            let c = refresh(MINE)?;
            let plugins: Vec<serde_json::Value> = c.plugins.iter().map(|o| serde_json::json!({"id": o.id, "version": o.manifest.as_ref().map(|m| m.version.clone()), "error": o.error})).collect();
            print_json(&serde_json::json!({"folder": mine_dir(), "plugins": plugins, "error": c.error}));
        }
        PluginCmd::Check { folders, json } => {
            let mut all = vec![];
            let mut each = vec![];
            for f in &folders {
                let (id, mut problems) = agent::check_folder(f);
                // Its dashboards' widgets of the plugins beside it (a marketplace's folder).
                problems.extend(plugins::presets::check_siblings(f));
                if !json {
                    if problems.is_empty() {
                        println!("{id} ({}): ok", f.display());
                    } else {
                        println!("{id} ({}): {} problem{}", f.display(), problems.len(), if problems.len() == 1 { "" } else { "s" });
                        for p in &problems {
                            println!("  - {}{}", if p.file.is_empty() { String::new() } else { format!("{}: ", p.file) }, p.what);
                        }
                    }
                }
                each.push(serde_json::json!({"id": id, "folder": f, "ok": problems.is_empty()}));
                all.extend(problems.into_iter().map(|p| serde_json::json!({"id": id, "file": p.file, "what": p.what})));
            }
            if json {
                print_json(&serde_json::json!({"ok": all.is_empty(), "problems": all, "plugins": each}));
            }
            if !all.is_empty() {
                std::process::exit(1);
            }
        }
        PluginCmd::Emit { event, payload, from } => {
            let payload: serde_json::Value = match from {
                Some(from) => emit_from(&event, &from)?,
                None => serde_json::from_str(payload.as_deref().unwrap_or("{}")).map_err(|e| anyhow::anyhow!("the payload isn't JSON: {e}"))?,
            };
            plugins::thread_values::emit(&event, &payload, cfg!(debug_assertions))?;
            println!("Queued {event}: the running app takes it within a second, or at its next start");
        }
        PluginCmd::Guide => print!("{GUIDE}"),
        PluginCmd::Marketplace { cmd } => match cmd {
            MarketCmd::List => print_json(&snapshot()["markets"]),
            MarketCmd::Add { source, name } => {
                let n = market_add(&source, name.as_deref())?;
                let c = refresh(&n)?;
                match c.error {
                    Some(e) => println!("Added {n}, but it can't be read yet: {e}"),
                    None => println!("Added {n}: {} plugins", c.plugins.len()),
                }
            }
            MarketCmd::Remove { name, yes, delete_data } => {
                let name = market_named(&name)?;
                // Refused first (the built-in one, My plugins): never asks for a --yes it would then refuse.
                removable(&name)?;
                let plugins = installed_from(&name)?;
                if !plugins.is_empty() {
                    let list: Vec<String> = plugins.iter().map(|(id, n)| format!("{n} ({id})")).collect();
                    println!("Removing {name} removes the plugins installed from it: {}", list.join(", "));
                    if !yes {
                        anyhow::bail!("run again with --yes to remove them with it (their data is kept; --delete-data deletes it)");
                    }
                }
                let gone = market_remove(&name, !delete_data)?;
                if gone.is_empty() {
                    println!("Removed {name}");
                } else {
                    println!("Removed {name} and the plugins installed from it: {}{}", gone.join(", "), if delete_data { " (their data deleted)" } else { "" });
                }
            }
            MarketCmd::Hide { name } => market_hide(&market_named(&name)?, true)?,
            MarketCmd::Show { name } => market_hide(&market_named(&name)?, false)?,
            MarketCmd::Update { name } => match name {
                Some(n) => {
                    let c = refresh(&market_named(&n)?)?;
                    if let Some(e) = c.error {
                        anyhow::bail!("{e}");
                    }
                }
                None => refresh_stale(true),
            },
        },
    }
    Ok(())
}

/// `plugin emit --from`: the payload as Echo builds it — `minutes_written` of a call's folder, `sources_changed` of
/// one Recall source.
#[cfg(feature = "js")]
fn emit_from(event: &str, from: &str) -> Result<serde_json::Value> {
    match event {
        "minutes_written" => Ok(crate::share::minutes_payload(&crate::calls::resolve_call(from)?)),
        "sources_changed" => crate::share::sources_payload(&[from.to_string()], &[]).ok_or_else(|| anyhow::anyhow!("not a mail or post: {from} (mail/… | channel/…)")),
        e => anyhow::bail!("--from builds minutes_written or sources_changed, not {e}"),
    }
}

#[cfg(not(feature = "js"))]
fn emit_from(_event: &str, _from: &str) -> Result<serde_json::Value> {
    anyhow::bail!("this build has no plugins")
}

/// `plugin call --file`: a handle of plugin `id` for each file. Handing a plugin a file is the user's own act: refused
/// where the CLI's limits for runs hold (`notes::run_limited`: every run Echo started but a routine acting on its own).
#[cfg(feature = "js")]
fn file_handles(id: &str, files: &[std::path::PathBuf]) -> anyhow::Result<Vec<serde_json::Value>> {
    if files.is_empty() {
        return Ok(vec![]);
    }
    anyhow::ensure!(!crate::notes::run_limited(), "--file is refused inside a run Echo started (all but a routine acting on its own): handing a plugin a file is yours to do");
    anyhow::ensure!(crate::plugins::enabled().iter().any(|p| p.id == id), "{id} isn't installed and on");
    files.iter().map(|f| jshost::files::make(id, f)).collect()
}

/// A plugin's review in the terminal: what it is, where from, and what it uses.
fn print_review(r: &plugins::Review) {
    println!("{} {} — from {} ({})", r.name, r.version, r.market, &r.commit[..r.commit.len().min(12)]);
    if r.uses.is_empty() {
        println!("  uses nothing of Echo's yet");
    }
    if let Some(b) = &r.blocked {
        println!("  ⚠ {b}");
    }
    for u in &r.uses {
        let new = if r.new_uses.contains(u) { " (new)" } else { "" };
        println!("  • {u}{new}");
    }
    if let Some(o) = &r.offers {
        println!("  {o}");
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "js")]
    #[test]
    fn plugin_call_file_is_refused_inside_a_run_echo_started() {
        let (_env, root) = crate::paths::testenv::folders("cli-plugin-call-file");
        let f = root.join("Brief.md");
        std::fs::write(&f, "# Brief").unwrap();
        std::env::set_var(crate::notes::HEADLESS, "1");
        let r = super::file_handles("acme", std::slice::from_ref(&f));
        std::env::remove_var(crate::notes::HEADLESS);
        assert!(r.unwrap_err().to_string().contains("refused inside a run Echo started"));
        assert!(super::file_handles("acme", &[]).unwrap().is_empty(), "no --file: nothing to refuse");
        // Outside a run: only for a plugin installed and on.
        assert!(super::file_handles("acme", &[f]).unwrap_err().to_string().contains("isn't installed and on"));
    }

    use super::{plugin_command, MarketCmd, PluginCmd};
    use serde_json::json;

    /// "acme-feed" installed from marketplace "t": an address setting (`feed`) and a plain one (`label`).
    fn installed(root: &std::path::Path) -> String {
        let mk = root.join("market");
        let d = mk.join("plugins/acme-feed");
        std::fs::create_dir_all(&d).unwrap();
        let manifest = json!({"id": "acme-feed", "name": "Acme feed", "version": "0.1.0",
            "uses": {"network": {"hosts_from": ["feed"]}},
            "settings": [{"key": "feed", "label": "Feed server", "type": "string"}, {"key": "label", "label": "Label", "type": "string"}]});
        std::fs::write(d.join("echo-plugin.json"), manifest.to_string()).unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "acme-feed", "source": "./plugins/acme-feed"}]}).to_string()).unwrap();
        let name = crate::plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        crate::plugins::refresh(&name).unwrap();
        let agreed = crate::plugins::review("acme-feed", Some(&name)).unwrap().uses;
        crate::plugins::install("acme-feed", Some(&name), &agreed).unwrap();
        name
    }

    /// `cmd` run inside a run Echo started: a Yolo-like run (limited), then a routine acting on its own (the CLI's
    /// limits lifted) — each error, or None where it ran.
    fn in_runs(cmd: impl Fn() -> PluginCmd) -> [Option<String>; 2] {
        std::env::set_var(crate::notes::HEADLESS, "1");
        let limited = plugin_command(cmd()).err().map(|e| e.to_string());
        std::env::set_var(crate::notes::RUN_SESSION, "acts");
        assert!(!crate::notes::run_limited(), "a routine acting on its own");
        let acts = plugin_command(cmd()).err().map(|e| e.to_string());
        std::env::remove_var(crate::notes::RUN_SESSION);
        std::env::remove_var(crate::notes::HEADLESS);
        [limited, acts]
    }

    fn refused(r: [Option<String>; 2]) -> bool {
        r.iter().all(|e| e.as_deref() == Some(super::REFUSED))
    }

    fn value(key: &str) -> serde_json::Value {
        crate::plugins::settings_view("acme-feed").unwrap().into_iter().find(|s| s["key"] == key).unwrap()["value"].clone()
    }

    fn on() -> bool {
        crate::plugins::enabled().iter().any(|p| p.id == "acme-feed")
    }

    #[test]
    fn plugin_set_is_refused_in_every_run_echo_started() {
        let (_env, root) = crate::paths::testenv::folders("cli-plugin-set-run");
        installed(&root);
        fn set(key: &'static str, v: &'static str) -> impl Fn() -> PluginCmd {
            move || PluginCmd::Set { id: "acme-feed".into(), key: key.into(), value: Some(v.into()) }
        }
        // The addresses it may reach, and a plain setting: neither changes from a run.
        assert!(refused(in_runs(set("feed", "https://feed.acme.example"))));
        assert!(refused(in_runs(set("label", "Acme"))));
        assert_eq!((value("feed"), value("label")), (json!(null), json!(null)));
        // Showing one still works there; outside a run the user sets them.
        assert_eq!(in_runs(|| PluginCmd::Set { id: "acme-feed".into(), key: "label".into(), value: None }), [None, None]);
        plugin_command(set("label", "Acme")()).unwrap();
        assert_eq!(value("label"), json!("Acme"));
    }

    #[test]
    fn plugin_trust_and_untrust_are_refused_in_every_run_echo_started() {
        let (_env, root) = crate::paths::testenv::folders("cli-plugin-trust-run");
        installed(&root);
        crate::plugins::set_setting("acme-feed", "feed", &json!("https://feed.acme.example")).unwrap();
        const KEY: &str = "sha256//Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc=";
        let trust = || PluginCmd::Trust { id: "acme-feed".into(), address: "feed.acme.example".into(), key: Some(KEY.into()) };
        assert!(refused(in_runs(trust)));
        assert!(crate::plugins::pins::view("acme-feed").unwrap().is_empty());
        // Listing them still works there; outside a run the user trusts and forgets.
        assert_eq!(in_runs(|| PluginCmd::Certs { id: "acme-feed".into() }), [None, None]);
        plugin_command(trust()).unwrap();
        assert_eq!(crate::plugins::pins::pinned("acme-feed", "feed.acme.example:443").as_deref(), Some(KEY));
        assert!(refused(in_runs(|| PluginCmd::Untrust { id: "acme-feed".into(), address: "feed.acme.example:443".into() })));
        plugin_command(PluginCmd::Untrust { id: "acme-feed".into(), address: "https://feed.acme.example".into() }).unwrap();
        assert!(crate::plugins::pins::view("acme-feed").unwrap().is_empty());
    }

    #[test]
    fn plugin_disable_is_refused_in_every_run_echo_started() {
        let (_env, root) = crate::paths::testenv::folders("cli-plugin-disable-run");
        installed(&root);
        assert!(refused(in_runs(|| PluginCmd::Disable { id: "acme-feed".into() })));
        assert!(on());
        plugin_command(PluginCmd::Disable { id: "acme-feed".into() }).unwrap();
        assert!(!on());
    }

    #[test]
    fn plugin_enable_is_refused_in_every_run_echo_started() {
        let (_env, root) = crate::paths::testenv::folders("cli-plugin-enable-run");
        installed(&root);
        crate::plugins::set_enabled("acme-feed", false).unwrap();
        assert!(refused(in_runs(|| PluginCmd::Enable { id: "acme-feed".into() })));
        assert!(!on());
        plugin_command(PluginCmd::Enable { id: "acme-feed".into() }).unwrap();
        assert!(on());
    }

    #[test]
    fn plugin_remove_and_marketplace_changes_are_refused_in_every_run_echo_started() {
        let (_env, root) = crate::paths::testenv::folders("cli-plugin-remove-run");
        let name = installed(&root);
        let other = root.join("other");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("echo-marketplace.json"), json!({"name": "o", "plugins": []}).to_string()).unwrap();
        assert!(refused(in_runs(|| PluginCmd::Remove { id: "acme-feed".into(), delete_data: true })));
        assert!(refused(in_runs(|| PluginCmd::Marketplace { cmd: MarketCmd::Add { source: other.display().to_string(), name: Some("o".into()) } })));
        assert!(refused(in_runs(|| PluginCmd::Marketplace { cmd: MarketCmd::Remove { name: name.clone(), yes: true, delete_data: true } })));
        assert!(refused(in_runs(|| PluginCmd::Marketplace { cmd: MarketCmd::Hide { name: name.clone() } })));
        assert!(refused(in_runs(|| PluginCmd::Marketplace { cmd: MarketCmd::Show { name: name.clone() } })));
        assert!(on(), "still installed and on");
        assert!(crate::plugins::snapshot()["markets"].as_array().unwrap().iter().all(|m| m["name"] != "o"), "no marketplace added");
    }

    #[test]
    fn a_plugins_script_cant_set_its_addresses_or_install_enable_or_trust() {
        let (_env, root) = crate::paths::testenv::folders("cli-plugin-script");
        installed(&root);
        crate::plugins::set_enabled("acme-feed", false).unwrap();
        let cmds = [
            PluginCmd::Set { id: "acme-feed".into(), key: "feed".into(), value: Some("https://evil.acme.example".into()) },
            PluginCmd::Enable { id: "acme-feed".into() },
            PluginCmd::Install { id: "acme-other".into(), from: None, yes: true },
            PluginCmd::Trust { id: "acme-feed".into(), address: "feed.acme.example".into(), key: None },
        ];
        std::env::set_var("ECHO_PLUGIN_SCRIPT", "1");
        let errs: Vec<String> = cmds.into_iter().map(|c| plugin_command(c).unwrap_err().to_string()).collect();
        // Showing a setting still works there.
        let shown = plugin_command(PluginCmd::Set { id: "acme-feed".into(), key: "feed".into(), value: None });
        std::env::remove_var("ECHO_PLUGIN_SCRIPT");
        assert!(errs.iter().all(|e| e == super::SCRIPT_REFUSED), "{errs:?}");
        shown.unwrap();
        assert_eq!(value("feed"), json!(null));
        assert!(!on());
        // The user, outside a script, still sets it.
        plugin_command(PluginCmd::Set { id: "acme-feed".into(), key: "feed".into(), value: Some("https://feed.acme.example".into()) }).unwrap();
        assert_eq!(value("feed"), json!("https://feed.acme.example"));
    }

    #[test]
    fn the_guide_gives_the_authoring_rules_then_the_api_reference() {
        let rules = super::GUIDE.find("# Plugin Authoring").expect("the rules");
        let api = super::GUIDE.find("# Echo Plugin API").expect("the reference");
        assert!(rules < api);
    }
}
