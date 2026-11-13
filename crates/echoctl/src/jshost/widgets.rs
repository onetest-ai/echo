//! The plugins' Today widgets: each one's last data in the plugin's `widgets.json`, its provider asked in the
//! background (or now, for `echoctl widget show`).

use std::sync::Mutex;

use super::*;

fn widgets_file(id: &str) -> PathBuf {
    plugins::data_dir(id).join("widgets.json")
}

/// Where a widget's answer for a page is kept in `widgets.json`: Today's under its id, a dashboard's beside it
/// (`<widget>@thread:<id>`, `<widget>@client:<id>`).
fn widget_key(widget: &str, page: &crate::today_layout::Page) -> String {
    match page {
        crate::today_layout::Page::Today => widget.to_string(),
        p => format!("{widget}@{}", p.key()),
    }
}

/// A widget's last data (`None`: cleared), or why its provider couldn't answer (the data before kept) — one the
/// plugin declares, at most 64 KB; `size`: the size its provider was asked for. Given for Today (a `set` or a `clear`
/// from the plugin), it replaces every page's: the dashboards' own answers go, and they show it until asked again.
pub(super) fn widget_store(id: &str, widget: &str, page: &crate::today_layout::Page, data: Option<&Value>, error: Option<&str>, size: Option<&str>) -> Result<()> {
    let declared = plugins::enabled_manifests().into_iter().find(|(p, _)| p == id).is_some_and(|(_, m)| m.widgets.iter().any(|w| w.id == widget));
    if !declared {
        bail!("{widget:?} isn't a widget this plugin declares (`widgets` in its manifest)");
    }
    if data.is_some_and(|d| d.to_string().len() > 64 * 1024) {
        bail!("a widget's data is 64 KB at most");
    }
    let f = widgets_file(id);
    let _lock = WIDGETS.lock().unwrap_or_else(|e| e.into_inner());
    let mut all: serde_json::Map<String, Value> = std::fs::read(&f).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    // `at`: when its data was given (Today's "from HH:MM"); `asked`: when Echo last asked its provider (or found it
    // had none), for the next ask; `error`: why the last ask failed, its data kept.
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let key = widget_key(widget, page);
    let mut last = all.get(&key).cloned().unwrap_or_else(|| json!({"data": null}));
    match (data, error) {
        (_, Some("")) => {
            last["asked"] = json!(now);
        }
        (_, Some(e)) => {
            last["error"] = json!(e);
            last["asked"] = json!(now);
        }
        (d, None) => {
            // A `set` keeps the size its provider was last asked for (else Today would take it for a resize).
            let kept = last.get("size").cloned();
            last = json!({"data": d.cloned().unwrap_or(Value::Null), "at": now, "asked": now});
            if let Some(k) = kept {
                last["size"] = k;
            }
        }
    }
    if let Some(size) = size {
        last["size"] = json!(size);
    }
    // The plugin's own `set` / `clear` (no ask, no size): every copy — the dashboards' answers give way to it.
    if *page == crate::today_layout::Page::Today && error.is_none() && size.is_none() {
        all.retain(|k, _| !k.starts_with(&format!("{widget}@")));
    }
    all.insert(key, last);
    std::fs::create_dir_all(plugins::data_dir(id))?;
    paths::write_atomic(&f, serde_json::to_string_pretty(&all)?.as_bytes())?;
    Ok(())
}

static WIDGETS: Mutex<()> = Mutex::new(());
/// The widgets being asked now (`plugin:widget`), so a busy Today asks each once at a time.
static ASKING: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Widgets asked for again while being asked: one more ask once this one is in.
static AGAIN: Mutex<std::collections::BTreeSet<String>> = Mutex::new(std::collections::BTreeSet::new());

/// A widget's last data, when it was given, when Echo last asked, and its provider's last error:
/// `{data, at, asked, error?}`.
#[cfg(test)]
pub fn widget_last(plugin: &str, widget: &str) -> Option<Value> {
    widget_last_on(plugin, widget, &crate::today_layout::Page::Today)
}

/// A widget's last answer for a page: a dashboard's own once its provider answered for it, else Today's (a widget
/// that only `set`s shows the same everywhere).
pub fn widget_last_on(plugin: &str, widget: &str, page: &crate::today_layout::Page) -> Option<Value> {
    let all: serde_json::Map<String, Value> = std::fs::read(widgets_file(plugin)).ok().and_then(|b| serde_json::from_slice(&b).ok())?;
    let own = all.get(&widget_key(widget, page)).cloned();
    match own {
        Some(o) if o.get("at").is_some() || *page == crate::today_layout::Page::Today => Some(o),
        o => match all.get(widget).cloned() {
            // Its own asking time and error stand (when it was last asked, for its next ask).
            Some(mut base) => {
                for k in ["asked", "error", "size"] {
                    if let Some(v) = o.as_ref().and_then(|o| o.get(k)) {
                        base[k] = v.clone();
                    }
                }
                Some(base)
            }
            None => o,
        },
    }
}

/// Ask a widget's provider for its data in the background (5 s, as a command), for a page — each place a widget is
/// on asked on its own, with that page's settings — keeping the answer; or, when it has no provider (it only `set`s)
/// or fails, its last data with why.
pub fn widget_ask_on(plugin: &str, widget: &str, page: &crate::today_layout::Page) {
    let key = format!("{plugin}:{}", widget_key(widget, page));
    {
        let mut asking = ASKING.lock().unwrap_or_else(|e| e.into_inner());
        if asking.contains(&key) {
            // Asked while an ask is on its way (a setting just changed): asked once more after it.
            AGAIN.lock().unwrap_or_else(|e| e.into_inner()).insert(key);
            return;
        }
        asking.push(key.clone());
    }
    let (plugin, widget, page) = (plugin.to_string(), widget.to_string(), page.clone());
    std::thread::spawn(move || {
        loop {
            widget_ask_now_on(&plugin, &widget, &page);
            if !AGAIN.lock().unwrap_or_else(|e| e.into_inner()).remove(&key) {
                break;
            }
        }
        ASKING.lock().unwrap_or_else(|e| e.into_inner()).retain(|k| k != &key);
    });
}

/// Ask a widget's provider and wait (5 s at most) — `echoctl widget show`, where a plugin's author checks its answer.
pub fn widget_ask_now(plugin: &str, widget: &str) {
    widget_ask_now_on(plugin, widget, &crate::today_layout::Page::Today)
}

pub(super) fn widget_ask_now_on(plugin: &str, widget: &str, page: &crate::today_layout::Page) {
    // Its settings as the page has them (defaults, the user's, then the page's own: a dashboard's thread or client),
    // given to the provider, and the size it has there (S, M, L), so it can give as much as fits.
    let id = format!("plugin:{plugin}:{widget}");
    let (params, size) = (crate::today_layout::params_on(page, &id), crate::today_layout::size_on(page, &id));
    let size = Some(size.as_str());
    match call_within(plugin, &format!("__widget:{widget}"), &json!({ "params": params, "size": size }), Duration::from_secs(5)) {
        // Kept — or, refused (over 64 KB), why, its data before kept.
        Ok(v) => {
            if let Err(e) = widget_store(plugin, widget, page, Some(&v), None, size) {
                let _ = widget_store(plugin, widget, page, None, Some(&format!("{e:#}")), size);
            }
        }
        // Only `set`: nothing to ask; its data stays as the plugin last gave it, marked asked (not asked again before
        // its refresh) — under the lock, so a `set` meanwhile isn't overwritten.
        Err(e) if e.to_string().contains(&format!("no command “__widget:{widget}”")) => drop(widget_store(plugin, widget, page, None, Some(""), size)),
        Err(e) => drop(widget_store(plugin, widget, page, None, Some(&format!("{e:#}")), size)),
    }
}

// ── the plugins' Inbox items ──
