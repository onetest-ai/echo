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
    match cmd {
        PluginCmd::List => print_json(&snapshot()),
        PluginCmd::Install { id, from, yes } => {
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
        PluginCmd::Set { id, key, value: Some(v) } => set_setting(&id, &key, &serde_json::Value::String(v))?,
        #[cfg(feature = "js")]
        PluginCmd::Call { id, command, args } => {
            let args: serde_json::Value = serde_json::from_str(args.as_deref().unwrap_or("{}")).map_err(|e| anyhow::anyhow!("the arguments aren't JSON: {e}"))?;
            // A terminal's host runs commands, never the plugins' timers: those are the app's.
            jshost::no_timers();
            jshost::sync();
            print_json(&jshost::call_within(&id, &command, &args, std::time::Duration::from_secs(3600))?);
        }
        #[cfg(not(feature = "js"))]
        PluginCmd::Call { .. } => anyhow::bail!("this build of Echo runs no plugin code"),
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
