//! A thread's and a client's own dashboards, beside the Brief and the Overview (which stay the first, each page's own):
//! each one is on every thread (every client), its layout shared (`today_layout::Page::Board`), its widgets showing
//! the page they are on. Listed in their order in `<config>/dashboards/boards.json`. A plugin's preset (its manifest's
//! `dashboards`) is offered in the "+", added only by the user, and says when its plugin changed it or went.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::paths;
use crate::plugins::presets::{PresetDef, PresetItem};
use crate::today_layout::Item;

/// At most this many beside the Brief (the Overview).
const MAX: usize = 12;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub id: String,
    pub name: String,
    /// A thread's: the stage groups it shows on (none: every thread).
    #[serde(default, rename = "for", skip_serializing_if = "Vec::is_empty")]
    pub stages: Vec<String>,
    /// Added from a plugin's preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<FromPreset>,
}

/// The plugin preset a dashboard was added from, and what was taken from it last.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FromPreset {
    pub plugin: String,
    pub preset: String,
    /// The plugin's version it came from.
    pub version: String,
    /// The preset's widgets and stage groups as taken (a newer one differs: offered as an update) …
    pub taken: Vec<PresetItem>,
    #[serde(default, rename = "taken_for", skip_serializing_if = "Vec::is_empty")]
    pub taken_stages: Vec<String>,
    /// … and the layout they made (the user's own differs: their changes, said before an update replaces them).
    pub laid: Vec<Item>,
}

#[derive(Default, Serialize, Deserialize)]
struct File {
    #[serde(default)]
    thread: Vec<Board>,
    #[serde(default)]
    client: Vec<Board>,
}

impl File {
    fn of(&mut self, kind: Kind) -> &mut Vec<Board> {
        match kind {
            Kind::Thread => &mut self.thread,
            Kind::Client => &mut self.client,
        }
    }
}

/// Whose pages a dashboard is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Thread,
    Client,
}

impl Kind {
    /// The window's name for it (`board_op`).
    #[cfg_attr(not(feature = "tray"), allow(dead_code))]
    pub fn parse(s: &str) -> Result<Kind> {
        match s {
            "thread" => Ok(Kind::Thread),
            "client" => Ok(Kind::Client),
            _ => bail!("{s:?} isn't thread or client"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Thread => "thread",
            Kind::Client => "client",
        }
    }
}

fn file() -> PathBuf {
    paths::config_dir().join("dashboards").join("boards.json")
}

/// The dashboards on every thread (client), in their order.
pub fn list(kind: Kind) -> Vec<Board> {
    let f: File = std::fs::read(file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    match kind {
        Kind::Thread => f.thread,
        Kind::Client => f.client,
    }
}

pub fn find(kind: Kind, id: &str) -> Option<Board> {
    list(kind).into_iter().find(|b| b.id == id)
}

fn change<T>(f: impl FnOnce(&mut File) -> Result<T>) -> Result<T> {
    let path = file();
    std::fs::create_dir_all(path.parent().expect("a folder"))?;
    crate::calls::update_state(&path, f)
}

fn name_ok(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 40 {
        bail!("a dashboard's name is 1 to 40 characters");
    }
    Ok(n.to_string())
}

/// A new dashboard on every thread (client), last; its id from its name.
pub fn add(kind: Kind, name: &str) -> Result<Board> {
    add_as(kind, name, vec![], None)
}

fn add_as(kind: Kind, name: &str, stages: Vec<String>, from: Option<FromPreset>) -> Result<Board> {
    let name = name_ok(name)?;
    change(|f| {
        let all = f.of(kind);
        if all.len() >= MAX {
            bail!("{MAX} dashboards at most");
        }
        if all.iter().any(|b| b.name.to_lowercase() == name.to_lowercase()) {
            bail!("there is a dashboard {name:?} already");
        }
        let base: String = crate::recorder::slugify(&name).chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        let base = base.trim_matches('-').chars().take(40).collect::<String>();
        let base = if base.is_empty() { "dashboard".to_string() } else { base };
        let mut id = base.clone();
        let mut n = 2;
        while all.iter().any(|b| b.id == id) || id == "overview" || id == "brief" {
            id = format!("{base}-{n}");
            n += 1;
        }
        let b = Board { id, name, stages, from };
        all.push(b.clone());
        // Nothing left from an earlier one of that id (a window that wrote after it was removed).
        crate::today_layout::forget_board(kind, &b.id);
        Ok(b)
    })
}

pub fn rename(kind: Kind, id: &str, name: &str) -> Result<Board> {
    let name = name_ok(name)?;
    change(|f| {
        let all = f.of(kind);
        if all.iter().any(|b| b.id != id && b.name.to_lowercase() == name.to_lowercase()) {
            bail!("there is a dashboard {name:?} already");
        }
        let b = all.iter_mut().find(|b| b.id == id).ok_or_else(|| anyhow!("no dashboard {id:?}"))?;
        b.name = name;
        Ok(b.clone())
    })
}

/// Put it at `to` (0 is first after the Brief / the Overview; past the end: last).
pub fn move_to(kind: Kind, id: &str, to: usize) -> Result<Vec<Board>> {
    change(|f| {
        let all = f.of(kind);
        let at = all.iter().position(|b| b.id == id).ok_or_else(|| anyhow!("no dashboard {id:?}"))?;
        let b = all.remove(at);
        all.insert(to.min(all.len()), b);
        Ok(all.clone())
    })
}

/// Remove it from every thread (client), its layout with it.
pub fn remove(kind: Kind, id: &str) -> Result<()> {
    change(|f| {
        let all = f.of(kind);
        let n = all.len();
        all.retain(|b| b.id != id);
        if all.len() == n {
            bail!("no dashboard {id:?}");
        }
        Ok(())
    })?;
    crate::today_layout::forget_board(kind, id);
    Ok(())
}

// ── plugins' presets ──

/// A preset's widgets as a layout on a dashboard of `plugin`'s.
fn layout_of(plugin: &str, items: &[PresetItem]) -> Vec<Item> {
    items
        .iter()
        .map(|i| {
            let widget = crate::plugins::presets::widget_id(plugin, &i.widget);
            let size = i.size.clone().unwrap_or_else(|| crate::widgets::find(&widget).ok().and_then(|w| w.sizes.first().map(|s| s.to_string())).unwrap_or_else(|| "M".into()));
            Item { widget, size, height: i.height.unwrap_or(1), row: i.row, params: i.params.clone() }
        })
        .collect()
}

/// An enabled plugin's preset of `kind`, with the plugin's name and version.
fn preset(plugin: &str, id: &str) -> Result<(PresetDef, String, String)> {
    let (_, m) = crate::plugins::enabled_manifests().into_iter().find(|(p, _)| p == plugin).ok_or_else(|| anyhow!("no plugin {plugin:?} installed and on"))?;
    let d = m.dashboards.iter().find(|d| d.id == id).cloned().ok_or_else(|| anyhow!("{} offers no dashboard {id:?}", m.name))?;
    Ok((d, m.name, m.version))
}

/// The dashboards and the plugins' presets as the window reads them: each dashboard with its `status`, and what each
/// kind's "+" offers.
pub fn view() -> Value {
    let ps = Plugins::now();
    let of = |kind: Kind| -> Vec<Value> {
        list(kind)
            .into_iter()
            .map(|b| {
                let mut v = serde_json::to_value(&b).unwrap_or_default();
                if let Some(s) = status_with(&ps, kind, &b) {
                    v["status"] = s;
                }
                v
            })
            .collect()
    };
    json!({"thread": of(Kind::Thread), "client": of(Kind::Client), "presets": {"thread": presets(Kind::Thread), "client": presets(Kind::Client)}})
}

/// What the plugins on offer for `kind`'s "+": `{plugin, plugin_name, preset, name, about, for, added}`.
pub fn presets(kind: Kind) -> Vec<Value> {
    let mine = list(kind);
    let mut out = vec![];
    for (p, m) in crate::plugins::enabled_manifests() {
        for d in m.dashboards.iter().filter(|d| d.kind == kind.name()) {
            let added = mine.iter().any(|b| b.from.as_ref().is_some_and(|f| f.plugin == p && f.preset == d.id));
            out.push(json!({"plugin": p, "plugin_name": m.name, "preset": d.id, "name": d.name.trim(), "about": d.about, "for": d.stages, "added": added}));
        }
    }
    out
}

/// Add a plugin's preset as a dashboard (once): its name, its layout, where it shows.
pub fn add_preset(plugin: &str, id: &str) -> Result<Board> {
    let (d, name, version) = preset(plugin, id)?;
    let kind = Kind::parse(&d.kind)?;
    if list(kind).iter().any(|b| b.from.as_ref().is_some_and(|f| f.plugin == plugin && f.preset == id)) {
        bail!("{name}'s “{}” is added already", d.name.trim());
    }
    let from = FromPreset { plugin: plugin.into(), preset: id.into(), version, taken: d.items.clone(), taken_stages: d.stages.clone(), laid: vec![] };
    // A name the user has a dashboard of already: the next free "<name> 2", "<name> 3"…
    let taken: Vec<String> = list(kind).into_iter().map(|b| b.name.to_lowercase()).collect();
    let base: String = d.name.trim().chars().take(37).collect();
    let name = std::iter::once(d.name.trim().to_string()).chain((2..=MAX + 1).map(|n| format!("{} {n}", base.trim_end()))).find(|n| !taken.contains(&n.to_lowercase())).unwrap_or_else(|| d.name.clone());
    let b = add_as(kind, &name, d.stages.clone(), Some(from))?;
    lay(kind, &b.id, layout_of(plugin, &d.items)).inspect_err(|_| {
        let _ = remove(kind, &b.id);
    })
}

/// A dashboard's layout set whole, kept as what its preset laid (as written: compared with what is saved, never with
/// what the catalog makes of it now).
fn lay(kind: Kind, id: &str, items: Vec<Item>) -> Result<Board> {
    crate::today_layout::set_board(kind, id, items)?;
    let laid = crate::today_layout::board_saved(kind, id);
    change(|f| {
        let b = f.of(kind).iter_mut().find(|b| b.id == id).ok_or_else(|| anyhow!("no dashboard {id:?}"))?;
        if let Some(from) = &mut b.from {
            from.laid = laid;
        }
        Ok(b.clone())
    })
}

/// What a status reads once for every dashboard: the installed plugins and the enabled ones' manifests.
struct Plugins {
    installed: std::collections::BTreeMap<String, crate::plugins::Installed>,
    manifests: Vec<(String, crate::plugins::Manifest)>,
}

impl Plugins {
    fn now() -> Plugins {
        Plugins { installed: crate::plugins::Registry::load().map(|r| r.installed).unwrap_or_default(), manifests: crate::plugins::enabled_manifests() }
    }
}

/// Where a preset's dashboard stands: `{state, plugin_name, edited}` — `ok`, `update` (its plugin changed the preset's
/// widgets or `for`; `edited`: the user changed the layout, which an update replaces), `dropped` (the plugin offers it no
/// more), `off` (its plugin is off), `gone` (removed). None for the user's own.
#[cfg(test)]
pub fn status(kind: Kind, b: &Board) -> Option<Value> {
    status_with(&Plugins::now(), kind, b)
}

fn status_with(ps: &Plugins, kind: Kind, b: &Board) -> Option<Value> {
    let f = b.from.as_ref()?;
    let edited = crate::today_layout::board_saved(kind, &b.id) != f.laid;
    let (state, name) = match ps.installed.get(&f.plugin) {
        None => ("gone", f.plugin.clone()),
        Some(p) if !p.enabled => ("off", p.name.clone()),
        Some(p) => match ps.manifests.iter().find(|(id, _)| *id == f.plugin).and_then(|(_, m)| m.dashboards.iter().find(|d| d.id == f.preset)) {
            None => ("dropped", p.name.clone()),
            Some(d) if d.items != f.taken || d.stages != f.taken_stages => ("update", p.name.clone()),
            Some(_) => ("ok", p.name.clone()),
        },
    };
    Some(json!({"state": state, "plugin_name": name, "edited": edited}))
}

/// Its plugin's newer preset: taken (its layout replaces the dashboard's, the user's changes with it), or kept off
/// (`keep`: the dashboard as it is, this version not offered again).
pub fn take_update(kind: Kind, id: &str, keep: bool) -> Result<Board> {
    let b = find(kind, id).ok_or_else(|| anyhow!("no dashboard {id:?}"))?;
    let f = b.from.clone().ok_or_else(|| anyhow!("“{}” isn't from a plugin", b.name))?;
    let (d, _, version) = preset(&f.plugin, &f.preset)?;
    change(|file| {
        let x = file.of(kind).iter_mut().find(|x| x.id == id).ok_or_else(|| anyhow!("no dashboard {id:?}"))?;
        let from = x.from.as_mut().expect("from a preset");
        from.taken = d.items.clone();
        from.taken_stages = d.stages.clone();
        from.version = version;
        if !keep {
            x.stages = d.stages.clone();
        }
        Ok(())
    })?;
    if keep {
        return find(kind, id).ok_or_else(|| anyhow!("no dashboard {id:?}"));
    }
    lay(kind, id, layout_of(&f.plugin, &d.items))
}

/// A dashboard as a preset block (`echoctl dashboard export`): its name, where it shows and its widgets — their
/// sizes, heights and settings but the page's thread / client.
pub fn export(kind: Kind, id: &str) -> Result<Value> {
    let b = find(kind, id).ok_or_else(|| anyhow!("no {} dashboard {id:?}", kind.name()))?;
    let items: Vec<PresetItem> = crate::today_layout::board_items(kind, id)
        .into_iter()
        .map(|i| {
            let page_set: Vec<String> = crate::widgets::find(&i.widget).map(|w| w.params.iter().filter(|p| p.kind == "thread" || p.kind == "client").map(|p| p.key.clone()).collect()).unwrap_or_else(|_| vec!["thread".into(), "client".into()]);
            let params = i.params.into_iter().filter(|(k, _)| !page_set.contains(k)).collect();
            PresetItem { widget: i.widget, size: Some(i.size), height: (i.height != 1).then_some(i.height), row: i.row, params }
        })
        .collect();
    Ok(serde_json::to_value(PresetDef { id: b.id, kind: kind.name().into(), name: b.name, about: String::new(), stages: b.stages, items })?)
}

/// A dashboard from a preset block (`echoctl dashboard import`): the user's own, named as it says.
pub fn import(kind: Kind, block: &Value) -> Result<Board> {
    let d: PresetDef = serde_json::from_value(block.clone()).map_err(|e| anyhow!("not a dashboard block (`echoctl dashboard export` writes one): {e}"))?;
    if d.kind != kind.name() {
        bail!("it is a {}'s dashboard, not a {}'s", d.kind, kind.name());
    }
    if d.items.is_empty() || d.items.len() > 24 || d.items.iter().any(|i| !crate::plugins::presets::full_ok(&i.widget)) {
        bail!("its widgets: 1–24, each named in full (plugin:<id>:<widget>, or Echo's core:…)");
    }
    if d.stages.iter().any(|s| !crate::plugins::presets::GROUPS.contains(&s.as_str())) || (!d.stages.is_empty() && kind != Kind::Thread) {
        bail!("“for” is a thread's, of {:?}", crate::plugins::presets::GROUPS);
    }
    let b = add_as(kind, &d.name, d.stages, None)?;
    crate::today_layout::set_board(kind, &b.id, layout_of("", &d.items)).inspect_err(|_| {
        let _ = remove(kind, &b.id);
    })?;
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboards_are_added_named_moved_and_removed_per_kind() {
        let (_env, _root) = paths::testenv::folders("boards");
        let a = add(Kind::Client, "Staffing").unwrap();
        assert_eq!(a, Board { id: "staffing".into(), name: "Staffing".into(), ..Default::default() });
        let b = add(Kind::Client, "Delivery health").unwrap();
        assert_eq!(b.id, "delivery-health");
        assert!(add(Kind::Client, "staffing").unwrap_err().to_string().contains("already"));
        assert!(add(Kind::Client, "  ").is_err());
        assert_eq!(add(Kind::Client, "Overview").unwrap().id, "overview-2", "never the default tab's id");
        assert!(list(Kind::Thread).is_empty(), "a client's are not a thread's");
        let ids = |v: Vec<Board>| v.into_iter().map(|b| b.id).collect::<Vec<_>>();
        assert_eq!(ids(move_to(Kind::Client, "delivery-health", 0).unwrap()), ["delivery-health", "staffing", "overview-2"]);
        assert_eq!(ids(move_to(Kind::Client, "delivery-health", 99).unwrap()), ["staffing", "overview-2", "delivery-health"]);
        assert_eq!(rename(Kind::Client, "staffing", "People").unwrap().name, "People");
        assert_eq!(find(Kind::Client, "staffing").unwrap().id, "staffing", "renamed, its id (and links) stay");
        remove(Kind::Client, "overview-2").unwrap();
        assert_eq!(ids(list(Kind::Client)), ["staffing", "delivery-health"]);
        assert!(remove(Kind::Client, "gone").is_err());
    }

    /// A made-up plugin offering a client's and a thread's dashboard, installed from a marketplace folder; `version` and
    /// the client preset's widgets as given (an update writes it again).
    fn plugin(root: &std::path::Path, version: &str, staffing: Value) {
        let d = root.join("market/plugins/acme-crm");
        std::fs::create_dir_all(&d).unwrap();
        let widgets = json!([
            {"id": "glance", "label": "Glance", "kind": "stats", "sizes": ["M", "L"], "heights": [1, 2], "params": [{"key": "client", "label": "Client", "type": "client"}, {"key": "span", "label": "Span", "type": "choice", "options": ["week", "month"]}]},
            {"id": "roster", "label": "Roster", "kind": "table", "sizes": ["M"], "params": [{"key": "client", "label": "Client", "type": "client"}]},
            {"id": "pipe", "label": "Pipeline", "kind": "list", "params": [{"key": "thread", "label": "Thread", "type": "thread"}]}
        ]);
        let dashboards = json!([{"id": "staffing", "kind": "client", "name": "Staffing", "about": "Who works where", "items": staffing},
            {"id": "pursuit", "kind": "thread", "name": "Pursuit", "for": ["pursuit"], "items": [{"widget": "pipe"}]}]);
        std::fs::write(d.join("echo-plugin.json"), json!({"id": "acme-crm", "name": "Acme CRM", "version": version, "main": "main.js", "widgets": widgets, "dashboards": dashboards}).to_string()).unwrap();
        std::fs::write(d.join("main.js"), "export function apply(ctx) {}").unwrap();
        std::fs::write(root.join("market/echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "acme-crm", "source": "./plugins/acme-crm"}]}).to_string()).unwrap();
    }

    #[test]
    fn a_plugins_dashboard_is_offered_added_once_by_the_user_and_says_when_its_plugin_changed_or_went() {
        use crate::plugins;
        let (_env, root) = paths::testenv::folders("boards-presets");
        plugin(&root, "0.1.0", json!([{"widget": "glance", "size": "L", "height": 2, "params": {"span": "month"}}, {"widget": "roster", "row": true}]));
        let m = plugins::market_add(root.join("market").to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&m).unwrap();
        let r = plugins::review("acme-crm", Some(&m)).unwrap();
        assert_eq!(r.offers.as_deref(), Some("Offers dashboards: Staffing (clients), Pursuit (threads)"));
        plugins::install("acme-crm", Some(&m), &r.uses).unwrap();
        // Offered, never added by itself.
        assert!(list(Kind::Client).is_empty());
        let offered = presets(Kind::Client);
        assert_eq!((offered.len(), offered[0]["name"].as_str(), offered[0]["added"].as_bool()), (1, Some("Staffing"), Some(false)));
        let b = add_preset("acme-crm", "staffing").unwrap();
        assert_eq!((b.id.as_str(), b.from.as_ref().unwrap().version.as_str()), ("staffing", "0.1.0"));
        let laid = crate::today_layout::board_items(Kind::Client, "staffing");
        assert_eq!(laid.iter().map(|i| (i.widget.as_str(), i.size.as_str(), i.height, i.row)).collect::<Vec<_>>(), [("plugin:acme-crm:glance", "L", 2, true), ("plugin:acme-crm:roster", "M", 1, true)]);
        assert_eq!(laid[0].params["span"], "month");
        assert_eq!(presets(Kind::Client)[0]["added"], true);
        assert!(add_preset("acme-crm", "staffing").unwrap_err().to_string().contains("added already"));
        let t = add_preset("acme-crm", "pursuit").unwrap();
        assert_eq!(t.stages, ["pursuit"], "a thread's: only on pursuit threads");
        let st = |id: &str| status(Kind::Client, &find(Kind::Client, id).unwrap()).unwrap();
        assert_eq!((st("staffing")["state"].as_str(), st("staffing")["edited"].as_bool()), (Some("ok"), Some(false)));
        assert!(status(Kind::Client, &add(Kind::Client, "Mine").unwrap()).is_none(), "the user's own: nothing to say");
        // The user changes it; the plugin changes its preset: offered, the user's changes said.
        crate::today_layout::set_board(Kind::Client, "staffing", vec![laid[1].clone()]).unwrap();
        plugin(&root, "0.2.0", json!([{"widget": "roster", "size": "M"}, {"widget": "glance", "size": "M", "row": true}]));
        plugins::refresh(&m).unwrap();
        plugins::update("acme-crm", &[]).unwrap();
        assert_eq!((st("staffing")["state"].as_str(), st("staffing")["edited"].as_bool(), st("staffing")["plugin_name"].as_str()), (Some("update"), Some(true), Some("Acme CRM")));
        // Keep mine: the layout stays, this version isn't offered again.
        take_update(Kind::Client, "staffing", true).unwrap();
        assert_eq!(st("staffing")["state"], "ok");
        assert_eq!(crate::today_layout::board_items(Kind::Client, "staffing").len(), 1);
        // A later one, taken: the plugin's layout replaces the user's.
        plugin(&root, "0.3.0", json!([{"widget": "glance", "size": "L"}]));
        plugins::refresh(&m).unwrap();
        plugins::update("acme-crm", &[]).unwrap();
        assert_eq!(st("staffing")["state"], "update");
        let b = take_update(Kind::Client, "staffing", false).unwrap();
        assert_eq!(b.from.unwrap().version, "0.3.0");
        assert_eq!(crate::today_layout::board_items(Kind::Client, "staffing").iter().map(|i| i.widget.as_str()).collect::<Vec<_>>(), ["plugin:acme-crm:glance"]);
        assert_eq!((st("staffing")["state"].as_str(), st("staffing")["edited"].as_bool()), (Some("ok"), Some(false)));
        // Its plugin off, then removed: the dashboard stays the user's, saying so.
        plugins::set_enabled("acme-crm", false).unwrap();
        assert_eq!(st("staffing")["state"], "off");
        assert!(presets(Kind::Client).is_empty(), "an off plugin offers nothing");
        plugins::remove("acme-crm", false).unwrap();
        assert_eq!(st("staffing")["state"], "gone");
        assert!(find(Kind::Client, "staffing").is_some());
    }

    #[test]
    fn a_dashboard_exported_as_a_preset_block_comes_back_by_import_without_its_pages_thread_or_client() {
        let (_env, _root) = paths::testenv::folders("boards-export");
        add(Kind::Client, "Staffing").unwrap();
        let items = vec![
            Item { widget: "plugin:acme-crm:glance".into(), size: "L".into(), height: 2, row: true, params: json!({"span": "month", "client": "acme"}).as_object().unwrap().clone() },
            Item { widget: "plugin:acme-crm:roster".into(), size: "M".into(), height: 1, row: false, params: Default::default() },
        ];
        crate::today_layout::set_board(Kind::Client, "staffing", items).unwrap();
        let block = export(Kind::Client, "staffing").unwrap();
        assert_eq!(block["items"][0], json!({"widget": "plugin:acme-crm:glance", "size": "L", "height": 2, "row": true, "params": {"span": "month"}}), "{block}");
        assert_eq!(block["items"][1], json!({"widget": "plugin:acme-crm:roster", "size": "M"}));
        assert!(import(Kind::Client, &block).unwrap_err().to_string().contains("already"), "its name is taken");
        let mut again = block.clone();
        again["name"] = json!("Staffing copy");
        let b = import(Kind::Client, &again).unwrap();
        assert!(b.from.is_none(), "an import is the user's own");
        assert_eq!(crate::today_layout::board_items(Kind::Client, &b.id).iter().map(|i| (i.widget.as_str(), i.size.as_str(), i.height)).collect::<Vec<_>>(), [("plugin:acme-crm:glance", "L", 2), ("plugin:acme-crm:roster", "M", 1)]);
        assert!(import(Kind::Thread, &again).unwrap_err().to_string().contains("a client's dashboard, not a thread's"));
        let mut bare = again.clone();
        bare["name"] = json!("Bare");
        bare["items"] = json!([{"widget": "glance"}]);
        assert!(import(Kind::Client, &bare).unwrap_err().to_string().contains("named in full"));
    }

    #[test]
    fn a_preset_named_like_a_users_dashboard_is_added_numbered_and_its_plugins_changes_are_told_apart_from_the_users() {
        use crate::plugins;
        let (_env, root) = paths::testenv::folders("boards-presets-edges");
        plugin(&root, "0.1.0", json!([{"widget": "glance", "size": "L", "height": 2}]));
        let m = plugins::market_add(root.join("market").to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&m).unwrap();
        let r = plugins::review("acme-crm", Some(&m)).unwrap();
        plugins::install("acme-crm", Some(&m), &r.uses).unwrap();
        add(Kind::Client, "Staffing").unwrap();
        let b = add_preset("acme-crm", "staffing").unwrap();
        assert_eq!((b.id.as_str(), b.name.as_str()), ("staffing-2", "Staffing 2"), "the user's own name kept, the preset's numbered");
        assert_eq!(presets(Kind::Client)[0]["added"], true);
        let st = || status(Kind::Client, &find(Kind::Client, "staffing-2").unwrap()).unwrap();
        // The plugin's widget no longer takes L or 2 rows: the page draws it smaller, but the user changed nothing.
        let d = root.join("market/plugins/acme-crm/echo-plugin.json");
        let mut man: Value = serde_json::from_slice(&std::fs::read(&d).unwrap()).unwrap();
        man["version"] = json!("0.2.0");
        man["widgets"][0]["sizes"] = json!(["M"]);
        man["widgets"][0]["heights"] = json!([1]);
        man["dashboards"][0]["items"] = json!([{"widget": "glance"}]);
        // Its thread preset now shows on delivery threads too: an update as well.
        man["dashboards"][1]["for"] = json!(["pursuit", "delivery"]);
        std::fs::write(&d, man.to_string()).unwrap();
        plugins::refresh(&m).unwrap();
        plugins::update("acme-crm", &[]).unwrap();
        assert_eq!((st()["state"].as_str(), st()["edited"].as_bool()), (Some("update"), Some(false)), "not the user's change");
        let t = add_preset("acme-crm", "pursuit").unwrap();
        assert_eq!(t.stages, ["pursuit", "delivery"]);
        man["dashboards"][1]["for"] = json!(["pursuit"]);
        man["version"] = json!("0.3.0");
        std::fs::write(&d, man.to_string()).unwrap();
        plugins::refresh(&m).unwrap();
        plugins::update("acme-crm", &[]).unwrap();
        assert_eq!(status(Kind::Thread, &find(Kind::Thread, &t.id).unwrap()).unwrap()["state"], "update", "only its `for` changed");
        assert_eq!(take_update(Kind::Thread, &t.id, false).unwrap().stages, ["pursuit"]);
    }

    #[test]
    fn an_import_keeps_echos_own_widgets_and_refuses_one_not_named_in_full() {
        let (_env, _root) = paths::testenv::folders("boards-import-core");
        let block = json!({"id": "x", "kind": "client", "name": "Mine", "items": [{"widget": "core:needs.now", "size": "M"}, {"widget": "plugin:acme-crm:glance"}]});
        let b = import(Kind::Client, &block).unwrap();
        assert_eq!(crate::today_layout::board_saved(Kind::Client, &b.id).iter().map(|i| i.widget.as_str()).collect::<Vec<_>>(), ["core:needs.now", "plugin:acme-crm:glance"]);
        for w in ["plugin::x", "plugin:Acme:x", "core:<b>", "glance"] {
            let mut bad = block.clone();
            bad["name"] = json!("Other");
            bad["items"] = json!([{"widget": w}]);
            assert!(import(Kind::Client, &bad).unwrap_err().to_string().contains("named in full"), "{w}");
        }
    }
}
