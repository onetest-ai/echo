//! Echo plugins and their marketplaces: `echoctl plugin …`.

use anyhow::Result;
use clap::Subcommand;

use super::print_json;
#[cfg(feature = "js")]
use crate::jshost;
use crate::plugins;

#[derive(Subcommand)]
pub enum PluginCmd {
    /// Installed plugins, what the marketplaces offer, and the marketplaces (JSON)
    List,
    /// Install a plugin (shows what it uses; --yes agrees)
    Install {
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
    /// Run a plugin's command (its host half's `ctx.command`) with JSON arguments; prints its JSON answer
    Call {
        id: String,
        command: String,
        /// JSON arguments (default {})
        args: Option<String>,
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
    /// The plugin authoring rules (Markdown), as this Echo has them: manifest, widgets and their data, host and window
    /// halves, testing, publishing
    Guide,
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
    Remove { name: String },
    /// Leave a marketplace out of Discover (the built-in one can't be removed, only hidden)
    Hide { name: String },
    Show { name: String },
    /// Fetch again (all, or one)
    Update { name: Option<String> },
}

/// Plugins are installed and updated by the user, never from inside a run Echo started.
fn refuse_in_run() -> Result<()> {
    if crate::notes::headless() {
        anyhow::bail!("plugins are installed and updated by you, not from inside a run Echo started");
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
        // From My plugins every install and update asks (new code, whatever it uses).
        if !yes && (!r.new_uses.is_empty() || r.market == MINE) {
            anyhow::bail!("run again with --yes to agree");
        }
        Ok(r.uses.clone())
    };
    match cmd {
        PluginCmd::List => print_json(&snapshot()),
        PluginCmd::Install { id, from, yes } => {
            refuse_in_run()?;
            let r = review(&id, from.as_deref())?;
            let agreed = agree(&r, yes)?;
            let p = install(&id, from.as_deref(), &agreed)?;
            println!("Installed {} {}", p.name, p.version);
        }
        PluginCmd::Update { id, yes } => {
            refuse_in_run()?;
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
        PluginCmd::Set { id, key, value: Some(v) } => set_setting(&id, &key, &serde_json::Value::String(v))?,
        #[cfg(feature = "js")]
        PluginCmd::Call { id, command, args } => {
            let args: serde_json::Value = serde_json::from_str(args.as_deref().unwrap_or("{}")).map_err(|e| anyhow::anyhow!("the arguments aren't JSON: {e}"))?;
            // A terminal's host runs commands, never the plugins' timers: those are the app's.
            jshost::no_timers();
            // `call_within` loads this plugin only.
            print_json(&jshost::call_within(&id, &command, &args, std::time::Duration::from_secs(3600))?);
        }
        #[cfg(not(feature = "js"))]
        PluginCmd::Call { .. } => anyhow::bail!("this build of Echo runs no plugin code"),
        PluginCmd::Script { id, skill, script, args } => {
            // In a read-only routine only a script marked readonly runs.
            let read_only = std::env::var(crate::guard::RUN_MODE).is_ok_and(|m| m == "routine");
            let v = crate::plugins::scripts::run(&id, &skill, &script, &args, None, read_only)?;
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
            let c = refresh(MINE)?;
            let plugins: Vec<serde_json::Value> = c.plugins.iter().map(|o| serde_json::json!({"id": o.id, "version": o.manifest.as_ref().map(|m| m.version.clone()), "error": o.error})).collect();
            print_json(&serde_json::json!({"folder": mine_dir(), "plugins": plugins, "error": c.error}));
        }
        PluginCmd::Guide => print!("{}", include_str!("../../../../.ai-run/guides/plugins/plugin-authoring.md")),
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
            MarketCmd::Remove { name } => market_remove(&name)?,
            MarketCmd::Hide { name } => market_hide(&name, true)?,
            MarketCmd::Show { name } => market_hide(&name, false)?,
            MarketCmd::Update { name } => match name {
                Some(n) => {
                    let c = refresh(&n)?;
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
