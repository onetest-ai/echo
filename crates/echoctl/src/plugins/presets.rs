//! Dashboards a plugin offers ready-made (`dashboards` in its manifest): offered in a thread's or a client's "+", added
//! only by the user (`boards::add_preset`). Its widgets are the plugin's own or its marketplace's other plugins'.

use super::*;

/// One dashboard a plugin offers: on threads or clients, its name, and its widgets as a layout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresetDef {
    pub id: String,
    /// `thread` or `client`.
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub about: String,
    /// A thread's: the stage groups it shows on (`pursuit`, `delivery`, `closed`); none: every thread.
    #[serde(default, rename = "for", skip_serializing_if = "Vec::is_empty")]
    pub stages: Vec<String>,
    pub items: Vec<PresetItem>,
}

/// A widget on a preset: the plugin's own (`<widget>`) or a plugin's of its marketplace (`plugin:<id>:<widget>`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PresetItem {
    pub widget: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u8>,
    /// It starts a new row.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub row: bool,
    /// Its settings; never its `thread` / `client` one — the page sets those.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub params: serde_json::Map<String, Value>,
}

/// The dashboards one plugin may offer, and the widgets on one.
const MAX_PRESETS: usize = 6;
const MAX_ITEMS: usize = 24;
pub const GROUPS: [&str; 3] = ["pursuit", "delivery", "closed"];

/// A preset item's widget as the catalog names it (one named in full as it is).
pub fn widget_id(plugin: &str, widget: &str) -> String {
    if widget.contains(':') {
        widget.to_string()
    } else {
        format!("plugin:{plugin}:{widget}")
    }
}

/// A widget named in full as a dashboard may hold it: Echo's own (`core:…`) or `plugin:<id>:<widget>`.
pub fn full_ok(id: &str) -> bool {
    parts(id).is_some() || (id.starts_with("core:") && id.len() <= 80 && id[5..].chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_'))
}

/// The plugin and widget of `plugin:<id>:<widget>`, each a plain id.
fn parts(id: &str) -> Option<(&str, &str)> {
    let (p, w) = id.strip_prefix("plugin:")?.split_once(':')?;
    (valid_id(p) && valid_id(w)).then_some((p, w))
}

/// A widget a preset puts on a page of `kind`, checked against its definition: it shows that page's thread or client,
/// takes the size and height, and its settings are ones it has (never the page's thread / client).
fn check_item(preset: &PresetDef, i: &PresetItem, w: &WidgetDef) -> Result<()> {
    let p = &preset.id;
    if !w.params.iter().any(|x| x.kind == preset.kind) {
        bail!("dashboard “{p}”: widget “{}” has no {} setting, so it can't show a {}'s own", i.widget, preset.kind, preset.kind);
    }
    if let Some(s) = &i.size {
        if !w.sizes.contains(s) {
            bail!("dashboard “{p}”: widget “{}” takes {}, not {s}", i.widget, w.sizes.join(", "));
        }
    }
    if let Some(h) = i.height {
        if !w.heights.contains(&h) {
            bail!("dashboard “{p}”: widget “{}” is {:?} rows high, not {h}", i.widget, w.heights);
        }
    }
    for (k, v) in &i.params {
        match w.params.iter().find(|x| &x.key == k) {
            None => bail!("dashboard “{p}”: widget “{}” has no setting “{k}”", i.widget),
            Some(x) if x.kind == "thread" || x.kind == "client" => bail!("dashboard “{p}”: widget “{}”'s “{k}” is set by the page, not the dashboard", i.widget),
            Some(x) if !x.takes(v) => bail!("dashboard “{p}”: widget “{}”'s “{k}” doesn't take {v}", i.widget),
            Some(_) => {}
        }
    }
    Ok(())
}

/// The manifest's dashboards, checked: their fields, and each widget that is the plugin's own against its definition
/// (another plugin's only by its name: `check_siblings` checks it where the marketplace's folder is at hand).
pub(super) fn check(m: &Manifest) -> Result<()> {
    if m.dashboards.len() > MAX_PRESETS {
        bail!("it offers more than {MAX_PRESETS} dashboards");
    }
    let mut ids = std::collections::HashSet::new();
    for d in &m.dashboards {
        if !valid_id(&d.id) || !ids.insert(d.id.clone()) {
            bail!("dashboard “{}” needs a plain, unique id (a–z, 0–9, -)", d.id);
        }
        if d.kind != "thread" && d.kind != "client" {
            bail!("dashboard “{}”: kind is thread or client", d.id);
        }
        if d.name.trim().is_empty() || d.name.trim().chars().count() > 40 || d.about.chars().count() > 160 {
            bail!("dashboard “{}” needs a name of 1–40 characters (and an about of 160 at most)", d.id);
        }
        if !d.stages.is_empty() && (d.kind != "thread" || d.stages.iter().any(|s| !GROUPS.contains(&s.as_str()))) {
            bail!("dashboard “{}”: “for” is a thread's, of {GROUPS:?}", d.id);
        }
        if d.items.is_empty() || d.items.len() > MAX_ITEMS {
            bail!("dashboard “{}” has 1–{MAX_ITEMS} widgets", d.id);
        }
        let mut seen = std::collections::HashSet::new();
        for i in &d.items {
            let full = widget_id(&m.id, &i.widget);
            let Some((p, w)) = parts(&full) else { bail!("dashboard “{}”: “{}” is a widget of its own or plugin:<id>:<widget>", d.id, i.widget) };
            if !seen.insert(full.clone()) {
                bail!("dashboard “{}” has widget “{full}” twice", d.id);
            }
            if p == m.id {
                let def = m.widgets.iter().find(|x| x.id == w).ok_or_else(|| anyhow!("dashboard “{}”: it declares no widget “{w}”", d.id))?;
                check_item(d, i, def)?;
            }
        }
    }
    Ok(())
}

/// The package's dashboards' widgets of other plugins, checked against those plugins' folders beside it (a
/// marketplace's `plugins/<id>`): what `plugin check` adds, when they are there.
pub fn check_siblings(dir: &Path) -> Vec<Problem> {
    let Ok(m) = read_manifest(dir) else { return vec![] };
    let mut out = vec![];
    for d in &m.dashboards {
        for i in &d.items {
            let full = widget_id(&m.id, &i.widget);
            let Some((p, w)) = parts(&full).filter(|(p, _)| *p != m.id) else { continue };
            let Some(other) = dir.parent().map(|f| f.join(p)).filter(|f| f.join(MANIFEST).is_file()) else { continue };
            let what = match read_manifest(&other) {
                Err(e) => format!("dashboard “{}”: its plugin “{p}” beside it can't be read: {e}", d.id),
                Ok(o) => match o.widgets.iter().find(|x| x.id == w) {
                    None => format!("dashboard “{}”: plugin “{p}” declares no widget “{w}”", d.id),
                    Some(def) => match check_item(d, i, def) {
                        Ok(()) => continue,
                        Err(e) => e.to_string(),
                    },
                },
            };
            out.push(Problem { file: MANIFEST.into(), what });
        }
    }
    out
}

/// What the install sheet says of them (not a permission: an update that adds one doesn't ask again).
pub fn offers(m: &Manifest) -> Option<String> {
    let each: Vec<String> = m.dashboards.iter().map(|d| format!("{} ({}s)", d.name.trim(), d.kind)).collect();
    (!each.is_empty()).then(|| format!("Offers dashboards: {}", each.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(dashboards: Value) -> Result<Manifest> {
        let dir = std::env::temp_dir().join(format!("echo-presets-{}-{}", std::process::id(), rand_id()));
        fs::create_dir_all(&dir).unwrap();
        let widgets = json!([
            {"id": "glance", "label": "Glance", "kind": "stats", "sizes": ["M", "L"], "heights": [1, 2], "params": [{"key": "client", "label": "Client", "type": "client"}, {"key": "span", "label": "Span", "type": "choice", "options": ["week", "month"]}]},
            {"id": "pipe", "label": "Pipeline", "kind": "list", "params": [{"key": "thread", "label": "Thread", "type": "thread"}]},
            {"id": "plain", "label": "Plain", "kind": "stat"}
        ]);
        fs::write(dir.join(MANIFEST), json!({"id": "acme-crm", "name": "Acme CRM", "version": "0.1.0", "main": "main.js", "widgets": widgets, "dashboards": dashboards}).to_string()).unwrap();
        fs::write(dir.join("main.js"), "export function apply(ctx) {}").unwrap();
        let r = read_manifest(&dir);
        let _ = fs::remove_dir_all(&dir);
        r
    }

    fn rand_id() -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    }

    #[test]
    fn a_plugins_dashboards_are_checked_against_its_own_widgets_and_said_on_the_sheet_without_asking() {
        let ok = json!([
            {"id": "staffing", "kind": "client", "name": "Staffing", "about": "Who works where", "items": [{"widget": "glance", "size": "L", "height": 2, "params": {"span": "month"}}, {"widget": "plugin:acme-staff:roster", "row": true}]},
            {"id": "pursuit", "kind": "thread", "name": "Pursuit", "for": ["pursuit"], "items": [{"widget": "plugin:acme-crm:pipe"}]}
        ]);
        let m = manifest(ok).unwrap();
        assert_eq!(m.dashboards[1].stages, ["pursuit"]);
        assert_eq!(offers(&m).as_deref(), Some("Offers dashboards: Staffing (clients), Pursuit (threads)"));
        assert!(!sheet(&m).unwrap().iter().any(|l| l.contains("dashboard")), "not a permission: not on the sheet's lines");
        let bad = |d: Value| manifest(json!([d])).unwrap_err().to_string();
        let one = |items: Value| json!({"id": "x", "kind": "client", "name": "X", "items": items});
        assert!(bad(one(json!([{"widget": "pipe"}]))).contains("has no client setting"));
        assert!(bad(one(json!([{"widget": "plain"}]))).contains("has no client setting"));
        assert!(bad(one(json!([{"widget": "nope"}]))).contains("declares no widget"));
        assert!(bad(one(json!([{"widget": "glance", "size": "S"}]))).contains("takes M, L"));
        assert!(bad(one(json!([{"widget": "glance", "height": 3}]))).contains("rows high"));
        assert!(bad(one(json!([{"widget": "glance", "params": {"client": "acme"}}]))).contains("set by the page"));
        assert!(bad(one(json!([{"widget": "glance", "params": {"span": "year"}}]))).contains("doesn't take"));
        assert!(bad(one(json!([{"widget": "glance", "params": {"color": "red"}}]))).contains("no setting"));
        assert!(bad(one(json!([{"widget": "glance"}, {"widget": "plugin:acme-crm:glance"}]))).contains("twice"));
        assert!(bad(one(json!([{"widget": "plugin:Bad Id:x"}]))).contains("plugin:<id>:<widget>"));
        assert!(bad(one(json!([]))).contains("1–24 widgets"));
        assert!(manifest(json!([{"id": "x", "kind": "client", "name": "X", "items": [{"widget": "glance"}]}, {"id": "x", "kind": "thread", "name": "Y", "items": [{"widget": "pipe"}]}])).unwrap_err().to_string().contains("unique id"), "one id for a thread's and a client's: refused");
        assert!(bad(one(json!([{"widget": "core:needs.now"}]))).contains("plugin:<id>:<widget>"), "Echo's own aren't a plugin's to offer");
        assert!(bad(json!({"id": "x", "kind": "page", "name": "X", "items": [{"widget": "glance"}]})).contains("thread or client"));
        assert!(bad(json!({"id": "x", "kind": "client", "name": "X", "for": ["pursuit"], "items": [{"widget": "glance"}]})).contains("“for” is a thread's"));
        assert!(bad(json!({"id": "x", "kind": "thread", "name": "X", "for": ["won"], "items": [{"widget": "pipe"}]})).contains("“for” is a thread's"));
        assert!(bad(json!({"id": "x", "kind": "client", "name": "", "items": [{"widget": "glance"}]})).contains("1–40"));
    }

    #[test]
    fn plugin_check_checks_a_dashboards_widgets_of_the_plugins_beside_it() {
        let (_env, root) = crate::paths::testenv::folders("presets-siblings");
        let mk = |id: &str, extra: Value| {
            let d = root.join("plugins").join(id);
            fs::create_dir_all(&d).unwrap();
            let mut m = json!({"id": id, "name": id, "version": "0.1.0", "main": "main.js"});
            for (k, v) in extra.as_object().unwrap() {
                m[k] = v.clone();
            }
            fs::write(d.join(MANIFEST), m.to_string()).unwrap();
            fs::write(d.join("main.js"), "export function apply(ctx) {}").unwrap();
            d
        };
        mk("acme-staff", json!({"widgets": [{"id": "roster", "label": "Roster", "kind": "table", "sizes": ["M"], "params": [{"key": "client", "label": "Client", "type": "client"}]}, {"id": "solo", "label": "Solo", "kind": "stat"}]}));
        let meta = mk("acme-meta", json!({"dashboards": [{"id": "staffing", "kind": "client", "name": "Staffing", "items": [
            {"widget": "plugin:acme-staff:roster", "size": "M"}, {"widget": "plugin:acme-staff:solo"}, {"widget": "plugin:acme-staff:gone"}, {"widget": "plugin:acme-elsewhere:x"}]}]}));
        let p: Vec<String> = check_siblings(&meta).into_iter().map(|p| p.what).collect();
        assert_eq!(p.len(), 2, "{p:?}: one without a client setting, one it doesn't declare; one not beside it isn't checked");
        assert!(p[0].contains("“plugin:acme-staff:solo” has no client setting") && p[1].contains("declares no widget “gone”"), "{p:?}");
    }
}
