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
    // Its kind as declared now: an answer is kept with it, and one kept for another kind (the plugin changed it in an
    // update) is never drawn as this one (`widgets::plugin_data` asks again).
    let kind = plugins::enabled_manifests().into_iter().find(|(p, _)| p == id).and_then(|(_, m)| m.widgets.iter().find(|w| w.id == widget).map(|w| w.kind.clone()));
    let Some(kind) = kind else {
        bail!("{widget:?} isn't a widget this plugin declares (`widgets` in its manifest)");
    };
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
            // It has no provider (it only `set`s): a dashboard shows what it set (`widget_last_on`).
            last["unasked"] = json!(true);
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
    last["kind"] = json!(kind);
    // The plugin's own `set` / `clear` (no ask, no size): every copy — each dashboard's shows it, until its own next ask.
    if *page == crate::today_layout::Page::Today && error.is_none() && size.is_none() {
        let mine = format!("{widget}@");
        for (_, v) in all.iter_mut().filter(|(k, _)| k.starts_with(&mine)) {
            v["data"] = last["data"].clone();
            v["at"] = last["at"].clone();
            v["asked"] = last["asked"].clone();
            v["kind"] = last["kind"].clone();
            // As on Today: what the plugin set replaces a failed ask's error.
            if let Some(o) = v.as_object_mut() {
                o.remove("error");
            }
        }
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

/// A widget's last answer for a page — its own only: a dashboard never shows Today's answer (another thread's, or
/// none), it waits for its own ask. One whose plugin has no provider (it only `set`s) shows what the plugin set.
pub fn widget_last_on(plugin: &str, widget: &str, page: &crate::today_layout::Page) -> Option<Value> {
    let all: serde_json::Map<String, Value> = std::fs::read(widgets_file(plugin)).ok().and_then(|b| serde_json::from_slice(&b).ok())?;
    let mut own = all.get(&widget_key(widget, page)).cloned()?;
    if *page != crate::today_layout::Page::Today && own["unasked"] == true && own["data"].is_null() {
        if let Some(base) = all.get(widget) {
            own["data"] = base["data"].clone();
            own["at"] = base["at"].clone();
        }
    }
    Some(own)
}

/// A page's dashboard went (its thread or client deleted), or a widget came off it (`widget`): their answers go.
pub fn widget_forget(page: &crate::today_layout::Page, widget: Option<&str>) {
    use crate::today_layout::Page;
    let key = page.key();
    // A dashboard of its own is every thread's (client's): its answers on each page go. A thread's (client's) page
    // takes its dashboards' with it.
    let board = match page {
        Page::Board(base, b) => Some((format!("{}:", base.key().split(':').next().unwrap_or_default()), format!("/{b}"))),
        _ => None,
    };
    let here = |p: &str| match &board {
        Some((kind, b)) => p.starts_with(kind.as_str()) && p.ends_with(b.as_str()),
        None => p == key || p.strip_prefix(key.as_str()).is_some_and(|r| r.starts_with('/')),
    };
    let _lock = WIDGETS.lock().unwrap_or_else(|e| e.into_inner());
    for e in std::fs::read_dir(paths::state_dir().join("plugins")).into_iter().flatten().flatten() {
        let id = e.file_name().to_string_lossy().into_owned();
        let f = widgets_file(&id);
        let Some(mut all) = std::fs::read(&f).ok().and_then(|b| serde_json::from_slice::<serde_json::Map<String, Value>>(&b).ok()) else { continue };
        let n = all.len();
        all.retain(|k, _| !k.split_once('@').is_some_and(|(w, p)| here(p) && widget.is_none_or(|x| x == w)));
        if all.len() != n {
            let _ = serde_json::to_string_pretty(&all).map(|t| paths::write_atomic(&f, t.as_bytes()));
        }
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

#[cfg(test)]
mod forget_tests {
    use super::*;
    use crate::today_layout::Page;

    #[test]
    fn answers_go_with_their_page_their_dashboards_and_a_dashboard_on_every_page() {
        let (_env, _root) = paths::testenv::folders("widget-forget");
        let f = widgets_file("demo");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        let put = |keys: &[&str]| std::fs::write(&f, serde_json::to_vec(&keys.iter().map(|k| (k.to_string(), json!(1))).collect::<serde_json::Map<_, _>>()).unwrap()).unwrap();
        let left = || { let m: serde_json::Map<String, Value> = serde_json::from_slice(&std::fs::read(&f).unwrap()).unwrap(); m.keys().cloned().collect::<Vec<_>>() };
        let all = ["a", "a@thread:x", "a@thread:x/hiring", "a@thread:xy", "b@thread:y/hiring", "a@client:c/hiring"];
        put(&all);
        widget_forget(&Page::Thread("x".into()), None);
        assert_eq!(left(), ["a", "a@client:c/hiring", "a@thread:xy", "b@thread:y/hiring"], "the thread's own and its dashboards', not thread xy's");
        put(&all);
        widget_forget(&Page::Board(Box::new(Page::Thread("x".into())), "hiring".into()), Some("b"));
        assert_eq!(left(), ["a", "a@client:c/hiring", "a@thread:x", "a@thread:x/hiring", "a@thread:xy"], "b off Hiring: on every thread");
        put(&all);
        widget_forget(&Page::Board(Box::new(Page::Thread("-".into())), "hiring".into()), None);
        assert_eq!(left(), ["a", "a@client:c/hiring", "a@thread:x", "a@thread:xy"], "Hiring removed: every thread's, never a client's");
    }
}
