//! Today's layout: which widgets Today shows, in what order, how wide, where a row starts — kept on this computer
//! (`<config>/today-layout.json`); a thread's and a client's dashboard keep theirs beside it (`<config>/dashboards/`,
//! [`Page`]). One `place` makes every change, by the rules of the edit-mode prototype
//! (Workbench, `WidgetsGallery`); the CLI (`echoctl today …`) and, later, edit mode change it through it (#323).

use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::{paths, widgets};

/// A widget on Today: which, how wide (S, M, L: 1, 2, 3 columns), how high (1 or 2 rows), and whether it starts a
/// new row.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Item {
    pub widget: String,
    pub size: String,
    #[serde(default = "one", skip_serializing_if = "is_one", deserialize_with = "height_or_one")]
    pub height: u8,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub row: bool,
    /// Its settings the user set (the rest are the widget's defaults: `params_of`).
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub params: serde_json::Map<String, serde_json::Value>,
}

fn one() -> u8 {
    1
}
fn is_one(h: &u8) -> bool {
    *h == 1
}
/// A height as written (a hand edit may hold anything): 2, else 1 — never a reason to lose the layout.
fn height_or_one<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u8, D::Error> {
    Ok(if serde_json::Value::deserialize(d)? == 2 { 2 } else { 1 })
}

#[derive(Serialize, Deserialize, Default)]
struct File {
    #[serde(default)]
    items: Option<Vec<Item>>,
}

/// Where a widget goes, against one already on Today.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// In its row, before it.
    Before,
    /// In its row, after it.
    After,
    /// On a row of its own above it.
    Above,
    /// On a row of its own below it.
    Below,
}

/// The widgets Today draws now; the others' views come with the layout steps (#315).
pub const DRAWN: &[&str] = &["core:needs.now", "core:routines.cards", "core:calendar.agenda", "core:tasks.mine", "core:calendar.next", "core:calendar.calendar", "core:calendar.strip", "core:calendar.choices", "core:tasks.glance", "core:tasks.done", "core:calls.week"];

const MAX_BYTES: usize = 16 * 1024;

/// Where a layout is: Today, or the dashboard of a thread (its Brief) or of a client (its Overview).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Page {
    Today,
    Thread(String),
    Client(String),
}

impl Page {
    /// `today`, `thread:<id>` or `client:<id>` (an id of letters, digits and dashes).
    pub fn parse(s: &str) -> Result<Page> {
        let id_ok = |id: &str| !id.is_empty() && id.len() <= 60 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !id.starts_with('-');
        match s {
            "" | "today" => Ok(Page::Today),
            _ => match s.split_once(':') {
                Some(("thread", id)) if id_ok(id) => Ok(Page::Thread(id.into())),
                Some(("client", id)) if id_ok(id) => Ok(Page::Client(id.into())),
                _ => bail!("{s:?} isn't a page: today, thread:<id> or client:<id>"),
            },
        }
    }

    /// The page a thread or a client names (none: Today), one that exists.
    pub fn of(thread: Option<&str>, client: Option<&str>) -> Result<Page> {
        match (thread.filter(|t| !t.is_empty()), client.filter(|c| !c.is_empty())) {
            (Some(t), _) => {
                crate::projects::load(t).map_err(|_| anyhow::anyhow!("no thread {t:?}: `echoctl project list` names them"))?;
                Page::parse(&format!("thread:{t}"))
            }
            (None, Some(c)) => {
                if !crate::projects::clients()?.iter().any(|x| x.id == c) {
                    bail!("no client {c:?}: `echoctl client list` names them");
                }
                Page::parse(&format!("client:{c}"))
            }
            (None, None) => Ok(Page::Today),
        }
    }

    /// What the page is called: `today`, `thread:<id>`, `client:<id>`.
    pub fn key(&self) -> String {
        match self {
            Page::Today => "today".into(),
            Page::Thread(id) => format!("thread:{id}"),
            Page::Client(id) => format!("client:{id}"),
        }
    }

    fn file(&self) -> PathBuf {
        match self {
            Page::Today => paths::config_dir().join("today-layout.json"),
            Page::Thread(id) => paths::config_dir().join("dashboards").join(format!("thread-{id}.json")),
            Page::Client(id) => paths::config_dir().join("dashboards").join(format!("client-{id}.json")),
        }
    }

    /// Where its widgets are, said in an error.
    fn on(&self) -> &'static str {
        match self {
            Page::Today => "on Today",
            _ => "on this dashboard",
        }
    }

    /// Whether this page offers `w`: Today every widget; a thread's dashboard one with a `thread` setting, a client's
    /// one with a `client` setting.
    pub fn offers(&self, w: &widgets::Widget) -> bool {
        match self {
            Page::Today => true,
            Page::Thread(_) => w.params.iter().any(|p| p.kind == "thread"),
            Page::Client(_) => w.params.iter().any(|p| p.kind == "client"),
        }
    }

    /// The settings this page sets on a widget (not the user's to change here): a thread's page its `thread` setting,
    /// and a `client` one the thread's client (a thread with none: the setting's default, null when it has none); a
    /// client's page its `client` setting.
    fn fixed(&self, w: &widgets::Widget) -> serde_json::Map<String, serde_json::Value> {
        use serde_json::Value;
        let mut out = serde_json::Map::new();
        let client_of = |t: &str| crate::projects::load(t).ok().and_then(|p| p.client).filter(|c| !c.is_empty());
        for p in &w.params {
            let v = match (self, p.kind.as_str()) {
                (Page::Thread(t), "thread") => Value::String(t.clone()),
                (Page::Thread(t), "client") => client_of(t).map(Value::String).or_else(|| p.default.clone()).unwrap_or(Value::Null),
                (Page::Client(c), "client") => Value::String(c.clone()),
                _ => continue,
            };
            out.insert(p.key.clone(), v);
        }
        out
    }
}

fn item(widget: &str, size: &str, row: bool) -> Item {
    Item { widget: widget.into(), size: size.into(), height: 1, row, params: Default::default() }
}

/// Today as it has been: Needs you now and the routines' cards across, then Your day (2 columns) beside My work (1).
pub fn default_items() -> Vec<Item> {
    // TodayResponsive A / TodayDense: Your day holding the first column; beside it Next meeting and Needs you now, the
    // day's choices, routine cards, My work filling the rest (the temporal ones only while they hold something).
    vec![
        item("core:calendar.agenda", "S", true),
        item("core:calendar.next", "S", true),
        item("core:needs.now", "M", false),
        item("core:calendar.choices", "M", true),
        item("core:routines.cards", "M", true),
        item("core:tasks.mine", "M", true),
    ]
}

/// Today's widgets: the saved layout, else the default. A widget the catalog doesn't have now (a plugin off) is kept
/// — the page just doesn't draw it — so a change never loses its place; a size it doesn't take becomes its first.
pub fn load() -> Vec<Item> {
    load_on(&Page::Today)
}

/// A page's widgets: its saved layout, else its default (Today's; a dashboard starts empty).
pub fn load_on(page: &Page) -> Vec<Item> {
    normalize(page, std::fs::read(page.file()).ok().and_then(|b| serde_json::from_slice::<File>(&b).ok()).and_then(|f| f.items))
}

/// What a page shows of a saved list (None: its default).
fn normalize(page: &Page, saved: Option<Vec<Item>>) -> Vec<Item> {
    let mut items = saved.unwrap_or_else(|| if *page == Page::Today { default_items() } else { vec![] });
    for i in &mut items {
        if let Ok(w) = widgets::find(&i.widget) {
            if !w.sizes.contains(&i.size.as_str()) {
                i.size = w.sizes[0].into();
            }
            if !w.heights.contains(&i.height) {
                i.height = 1;
            }
        }
    }
    if let Some(first) = items.first_mut() {
        first.row = true;
    }
    items
}

/// `widget` at `size`, placed against `target` (None: last, on a row of its own). If it is on Today already it moves;
/// the widget it leaves hands its row start on. The order is the list's; nothing else moves.
pub fn place(items: &[Item], widget: &str, size: &str, target: Option<(&str, Side)>) -> Vec<Item> {
    let mut out = items.to_vec();
    // A widget moved keeps its settings and its height.
    let (mut kept, mut height) = (serde_json::Map::new(), 1);
    if let Some(i) = out.iter().position(|x| x.widget == widget) {
        let gone = out.remove(i);
        kept = gone.params;
        height = gone.height;
        if gone.row {
            if let Some(next) = out.get_mut(i) {
                next.row = true;
            }
        }
    }
    let mut new = item(widget, size, false);
    new.params = kept;
    new.height = height;
    let at = match target.and_then(|(t, side)| out.iter().position(|x| x.widget == t).map(|i| (i, side))) {
        None => {
            new.row = true;
            out.len()
        }
        Some((i, Side::Above)) => {
            new.row = true;
            out[i].row = true;
            i
        }
        Some((i, Side::Below)) => {
            new.row = true;
            if let Some(next) = out.get_mut(i + 1) {
                next.row = true;
            }
            i + 1
        }
        Some((i, Side::After)) => i + 1,
        Some((i, Side::Before)) => {
            new.row = std::mem::take(&mut out[i].row);
            i
        }
    };
    out.insert(at, new);
    if let Some(first) = out.first_mut() {
        first.row = true;
    }
    out
}

/// A widget's size, checked: one it takes (the default: M if it takes it, else its first) — and, on a dashboard, one
/// the page offers.
fn size_of(page: &Page, widget: &str, size: Option<&str>) -> Result<String> {
    let w = widgets::find(widget)?;
    if !page.offers(&w) {
        bail!("{widget} can't go on a {} dashboard: it has no {} setting", if matches!(page, Page::Client(_)) { "client's" } else { "thread's" }, if matches!(page, Page::Client(_)) { "client" } else { "thread" });
    }
    // A plugin's widget is drawn by its kind (12-today.js, pluginWidgetView).
    if !DRAWN.contains(&widget) && !widget.starts_with("plugin:") && !widget.starts_with(widgets::CARD) {
        bail!("{widget} isn't on Today's page yet: its view comes with the next widget steps (#315)");
    }
    let s = size.map(str::to_uppercase).unwrap_or_else(|| if w.sizes.contains(&"M") { "M".into() } else { w.sizes[0].into() });
    if !w.sizes.contains(&s.as_str()) {
        bail!("{widget} takes {}, not {s}", w.sizes.join(", "));
    }
    Ok(s)
}

/// Changes a page's saved layout (starting from what it shows) under its lock, and returns its widgets after it.
fn change(page: &Page, f: impl FnOnce(Vec<Item>) -> Result<Vec<Item>>) -> Result<Vec<Item>> {
    let file = page.file();
    std::fs::create_dir_all(file.parent().expect("a folder"))?;
    // A file that can't be read (a hand edit gone wrong) is kept beside it, and the change starts from the default —
    // as the page already shows.
    if std::fs::read(&file).is_ok_and(|b| serde_json::from_slice::<File>(&b).is_err()) {
        std::fs::rename(&file, file.with_extension("unreadable.json"))?;
    }
    crate::calls::update_state(&file, |v: &mut File| {
        // What it holds now, read under the lock: two changes at once both count.
        let next = f(normalize(page, v.items.take()))?;
        let bytes = serde_json::to_vec(&next)?.len();
        if bytes > MAX_BYTES {
            bail!("the layout would be {bytes} bytes, over {MAX_BYTES}");
        }
        v.items = Some(next);
        Ok(())
    })?;
    Ok(load_on(page))
}

#[cfg(test)]
pub fn add(widget: &str, size: Option<&str>, target: Option<(&str, Side)>) -> Result<Vec<Item>> {
    add_on(&Page::Today, widget, size, target)
}

pub fn add_on(page: &Page, widget: &str, size: Option<&str>, target: Option<(&str, Side)>) -> Result<Vec<Item>> {
    let size = size_of(page, widget, size)?;
    change(page, |items| {
        if items.iter().any(|i| i.widget == widget) {
            bail!("{widget} is {} already: `echoctl today move` or `today size` change it", page.on());
        }
        on_today(page, &items, target)?;
        Ok(place(&items, widget, &size, target))
    })
}

#[cfg(test)]
pub fn move_to(widget: &str, target: Option<(&str, Side)>) -> Result<Vec<Item>> {
    move_on(&Page::Today, widget, target)
}

pub fn move_on(page: &Page, widget: &str, target: Option<(&str, Side)>) -> Result<Vec<Item>> {
    change(page, |items| {
        let cur = items.iter().find(|i| i.widget == widget).ok_or_else(|| anyhow::anyhow!("{widget} isn't {}", page.on()))?.clone();
        on_today(page, &items, target)?;
        if target.is_some_and(|(t, _)| t == widget) {
            return Ok(items);
        }
        Ok(place(&items, widget, &cur.size, target))
    })
}

#[cfg(test)]
pub fn resize(widget: &str, size: &str) -> Result<Vec<Item>> {
    resize_on(&Page::Today, widget, size)
}

pub fn resize_on(page: &Page, widget: &str, size: &str) -> Result<Vec<Item>> {
    let size = size_of(page, widget, Some(size))?;
    change(page, |mut items| {
        let i = items.iter_mut().find(|i| i.widget == widget).ok_or_else(|| anyhow::anyhow!("{widget} isn't {}", page.on()))?;
        i.size = size;
        Ok(items)
    })
}

/// A widget's height on Today: 1 or 2 rows, of those it takes.
#[cfg(test)]
pub fn set_height(widget: &str, height: u8) -> Result<Vec<Item>> {
    set_height_on(&Page::Today, widget, height)
}

pub fn set_height_on(page: &Page, widget: &str, height: u8) -> Result<Vec<Item>> {
    let w = widgets::find(widget)?;
    if !w.heights.contains(&height) {
        let takes: Vec<String> = w.heights.iter().map(|h| h.to_string()).collect();
        bail!("{widget} is {} row{} high, not {height}", takes.join(" or "), if takes == ["1"] { "" } else { "s" });
    }
    change(page, |mut items| {
        let i = items.iter_mut().find(|i| i.widget == widget).ok_or_else(|| anyhow::anyhow!("{widget} isn't {}", page.on()))?;
        i.height = height;
        Ok(items)
    })
}

/// Set one of a widget's settings on Today (`value` as text: a choice's option, true / false, a thread's or a client's
/// id), refusing a key it doesn't have or a value it doesn't take.
#[cfg(test)]
pub fn set_param(widget: &str, key: &str, value: &str) -> Result<Vec<Item>> {
    set_param_on(&Page::Today, widget, key, value)
}

pub fn set_param_on(page: &Page, widget: &str, key: &str, value: &str) -> Result<Vec<Item>> {
    let w = widgets::find(widget)?;
    if page.fixed(&w).contains_key(key) {
        bail!("{widget}'s {key} is this page's: it isn't set here");
    }
    let def = w.params.iter().find(|p| p.key == key).ok_or_else(|| {
        let keys: Vec<&str> = w.params.iter().map(|p| p.key.as_str()).collect();
        anyhow::anyhow!("{widget} has no setting {key:?}{}", if keys.is_empty() { String::new() } else { format!(" (it has {})", keys.join(", ")) })
    })?;
    let v = if def.kind == "bool" { serde_json::Value::Bool(matches!(value, "true" | "yes" | "on")) } else { serde_json::Value::String(value.to_string()) };
    if !def.takes(&v) {
        bail!("{widget}'s {key} doesn't take {value:?}{}", if def.kind == "choice" { format!(" ({})", def.options.join(", ")) } else { String::new() });
    }
    change(page, |mut items| {
        let i = items.iter_mut().find(|i| i.widget == widget).ok_or_else(|| anyhow::anyhow!("{widget} isn't {}", page.on()))?;
        i.params.insert(key.into(), v.clone());
        Ok(items)
    })
}

/// A widget's size on a page (S, M or L); off it, the first size it takes.
pub fn size_on(page: &Page, widget: &str) -> String {
    load_on(page).into_iter().find(|i| i.widget == widget).map(|i| i.size).or_else(|| widgets::find(widget).ok().and_then(|w| w.sizes.first().map(|s| s.to_string()))).unwrap_or_else(|| "M".into())
}

/// A widget's settings as Today has them: its defaults, then what the user set (a value it no longer takes, dropped).
pub fn params_of(widget: &str) -> serde_json::Map<String, serde_json::Value> {
    params_on(&Page::Today, widget)
}

/// A widget's settings on a page: its defaults, then what the user set there (a value it no longer takes, dropped),
/// then what the page sets ([`Page::fixed`]: a dashboard's thread or client).
pub fn params_on(page: &Page, widget: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut out = serde_json::Map::new();
    let Ok(w) = widgets::find(widget) else { return out };
    let set = load_on(page).into_iter().find(|i| i.widget == widget).map(|i| i.params).unwrap_or_default();
    for p in &w.params {
        if let Some(v) = set.get(&p.key).filter(|v| p.takes(v)).or(p.default.as_ref()) {
            out.insert(p.key.clone(), v.clone());
        }
    }
    for (k, v) in page.fixed(&w) {
        if v.is_null() {
            out.remove(&k);
        } else {
            out.insert(k, v);
        }
    }
    out
}

/// A thread or a client deleted: its dashboard goes (a new one may get its id), and the plugins' answers for it.
pub fn forget(page: &Page) {
    if *page == Page::Today {
        return;
    }
    let f = page.file();
    let _ = std::fs::remove_file(&f);
    let _ = std::fs::remove_file(f.with_extension("lock"));
    #[cfg(feature = "js")]
    crate::jshost::widget_forget(page, None);
}

#[cfg(test)]
pub fn remove(widget: &str) -> Result<Vec<Item>> {
    remove_on(&Page::Today, widget)
}

pub fn remove_on(page: &Page, widget: &str) -> Result<Vec<Item>> {
    let items = change(page, |items| {
        let i = items.iter().position(|i| i.widget == widget).ok_or_else(|| anyhow::anyhow!("{widget} isn't {}", page.on()))?;
        let mut out = items;
        let gone = out.remove(i);
        if gone.row {
            if let Some(next) = out.get_mut(i) {
                next.row = true;
            }
        }
        Ok(out)
    })?;
    // Off a dashboard: its answer there goes too (Today's stays, the plugin's own).
    #[cfg(feature = "js")]
    if let (false, Some((_, w))) = (*page == Page::Today, widget.strip_prefix("plugin:").and_then(|r| r.split_once(':'))) {
        crate::jshost::widget_forget(page, Some(w));
    }
    Ok(items)
}

#[cfg(test)]
pub fn reset() -> Result<Vec<Item>> {
    reset_on(&Page::Today)
}

/// A page as it comes: Today's default, a dashboard empty.
pub fn reset_on(page: &Page) -> Result<Vec<Item>> {
    change(page, |_| Ok(if *page == Page::Today { default_items() } else { vec![] }))
}

fn on_today(page: &Page, items: &[Item], target: Option<(&str, Side)>) -> Result<()> {
    match target {
        Some((t, _)) if !items.iter().any(|i| i.widget == t) => bail!("{t} isn't {}: `echoctl today` lists what is", page.on()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lay(xs: &[(&str, bool)]) -> Vec<Item> {
        xs.iter().map(|(w, r)| item(w, "M", *r)).collect()
    }
    fn show(items: &[Item]) -> String {
        items.iter().map(|i| if i.row { format!("| {}", i.widget) } else { i.widget.clone() }).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn place_puts_a_widget_before_after_above_below_or_last_and_hands_on_a_row_start() {
        // Rows: | a b | c
        let l = lay(&[("a", true), ("b", false), ("c", true)]);
        let t = |w: &str, target: Option<(&str, Side)>| show(&place(&l, w, "M", target));
        assert_eq!(t("x", Some(("b", Side::Before))), "| a x b | c");
        assert_eq!(t("x", Some(("a", Side::Before))), "| x a b | c", "before a row's first: x starts the row");
        assert_eq!(t("x", Some(("b", Side::After))), "| a b x | c");
        assert_eq!(t("x", Some(("b", Side::Above))), "| a | x | b | c", "a row of its own above b");
        assert_eq!(t("x", Some(("a", Side::Below))), "| a | x | b | c", "a row of its own below a");
        assert_eq!(t("x", None), "| a b | c | x", "last, on a new row");
        // Moving one that is there: a leaves its row start to b.
        assert_eq!(t("a", Some(("c", Side::After))), "| b | c a");
        assert_eq!(t("c", Some(("a", Side::After))), "| a c b", "c leaves nothing behind but its row");
        // Onto itself: still one of it.
        assert_eq!(t("b", Some(("b", Side::After))).matches('b').count(), 1);
    }

    #[test]
    fn a_widgets_settings_are_kept_on_today_checked_and_filled_from_its_defaults() {
        let (_env, _) = paths::testenv::folders("today-layout-params");
        assert_eq!(params_of("core:tasks.done")["period"], "week", "its default");
        let err = |r: Result<Vec<Item>>| r.unwrap_err().to_string();
        assert!(err(set_param("core:tasks.done", "period", "year")).contains("doesn't take \"year\" (week, month)"));
        assert!(err(set_param("core:tasks.done", "colour", "red")).contains("has no setting \"colour\" (it has period, thread, client)"));
        assert!(err(set_param("core:tasks.mine", "period", "month")).contains("has no setting"));
        assert!(err(set_param("core:tasks.done", "period", "month")).contains("isn't on Today"), "only a widget on Today");
        add("core:tasks.done", Some("S"), None).unwrap();
        set_param("core:tasks.done", "period", "month").unwrap();
        assert_eq!(params_of("core:tasks.done")["period"], "month");
        assert_eq!(load().iter().find(|i| i.widget == "core:tasks.done").unwrap().params["period"], "month", "kept on disk");
        // Moved (the CLI or a drag), it keeps them.
        move_to("core:tasks.done", Some(("core:calendar.agenda", Side::Above))).unwrap();
        assert_eq!(params_of("core:tasks.done")["period"], "month", "kept through a move");
    }

    #[test]
    fn the_default_is_todays_page_and_a_missing_or_unreadable_file_gives_it() {
        let (_env, _) = paths::testenv::folders("today-layout-default");
        assert_eq!(show(&load()), "| core:calendar.agenda | core:calendar.next core:needs.now | core:calendar.choices | core:routines.cards | core:tasks.mine");
        std::fs::create_dir_all(paths::config_dir()).unwrap();
        std::fs::write(Page::Today.file(), "{ not json").unwrap();
        assert_eq!(load(), default_items());
        // A change starts from the default and keeps the unreadable file beside it.
        assert_eq!(show(&remove("core:routines.cards").unwrap()), "| core:calendar.agenda | core:calendar.next core:needs.now | core:calendar.choices | core:tasks.mine");
        assert_eq!(std::fs::read_to_string(Page::Today.file().with_extension("unreadable.json")).unwrap(), "{ not json");
        // A widget the catalog doesn't have now (a plugin off) keeps its place through a change.
        std::fs::write(Page::Today.file(), r#"{"items":[{"widget":"plugin:x:y","size":"M","row":true},{"widget":"core:tasks.mine","size":"M"}]}"#).unwrap();
        assert_eq!(show(&resize("core:tasks.mine", "S").unwrap()), "| plugin:x:y core:tasks.mine");
    }

    #[test]
    fn a_height_is_one_the_widget_takes_kept_through_a_move_and_one_when_it_isnt_taken() {
        let (_env, _) = paths::testenv::folders("today-layout-height");
        let err = |r: Result<Vec<Item>>| r.unwrap_err().to_string();
        assert!(err(set_height("core:tasks.mine", 3)).contains("is 1 or 2 rows high, not 3"));
        assert!(err(set_height("core:calls.week", 2)).contains("is 1 row high, not 2"));
        assert!(err(set_height("core:tasks.glance", 2)).contains("isn't on Today"));
        let after = set_height("core:tasks.mine", 2).unwrap();
        assert_eq!(after.iter().find(|i| i.widget == "core:tasks.mine").unwrap().height, 2);
        let disk: serde_json::Value = serde_json::from_slice(&std::fs::read(Page::Today.file()).unwrap()).unwrap();
        let on_disk = |w: &str| disk["items"].as_array().unwrap().iter().find(|i| i["widget"] == w).unwrap()["height"].clone();
        assert_eq!(on_disk("core:tasks.mine"), 2, "kept on disk");
        assert!(on_disk("core:needs.now").is_null(), "1 is not written");
        move_to("core:tasks.mine", Some(("core:needs.now", Side::Before))).unwrap();
        assert_eq!(load().iter().find(|i| i.widget == "core:tasks.mine").unwrap().height, 2, "kept through a move");
        // A height the widget doesn't take (a hand edit) is 1.
        std::fs::write(Page::Today.file(), r#"{"items":[{"widget":"core:calls.week","size":"S","height":2,"row":true}]}"#).unwrap();
        assert_eq!(load()[0].height, 1);
        // One that isn't a height at all: 1, and the rest of the layout kept.
        std::fs::write(Page::Today.file(), r#"{"items":[{"widget":"core:tasks.mine","size":"S","height":"tall","row":true},{"widget":"core:needs.now","size":"M","height":2.5}]}"#).unwrap();
        assert_eq!(load().iter().map(|i| (i.widget.as_str(), i.height)).collect::<Vec<_>>(), [("core:tasks.mine", 1), ("core:needs.now", 1)]);
    }

    #[test]
    fn the_cli_changes_refuse_what_today_cant_take_and_keep_what_they_change() {
        let (_env, _) = paths::testenv::folders("today-layout-changes");
        let err = |r: Result<Vec<Item>>| r.unwrap_err().to_string();
        assert!(err(add("core:nope", None, None)).contains("echoctl widget list"));
        assert!(err(add("core:tasks.mine", None, None)).contains("on Today already"));
        assert!(err(resize("core:tasks.mine", "L")).contains("takes S, M, not L"));
        assert!(err(move_to("core:tasks.mine", Some(("core:calendar.strip", Side::After)))).contains("isn't on Today"));
        let after = remove("core:routines.cards").unwrap();
        assert_eq!(show(&after), "| core:calendar.agenda | core:calendar.next core:needs.now | core:calendar.choices | core:tasks.mine");
        let after = add("core:routines.cards", Some("l"), Some(("core:needs.now", Side::After))).unwrap();
        assert_eq!(show(&after), "| core:calendar.agenda | core:calendar.next core:needs.now core:routines.cards | core:calendar.choices | core:tasks.mine");
        assert_eq!(after[3].size, "L");
        assert_eq!(show(&move_to("core:tasks.mine", Some(("core:needs.now", Side::Above))).unwrap()), "| core:calendar.agenda | core:calendar.next | core:tasks.mine | core:needs.now core:routines.cards | core:calendar.choices", "a row of its own just above Needs");
        assert_eq!(load()[2].widget, "core:tasks.mine", "kept on disk");
        assert_eq!(reset().unwrap(), default_items());
    }
}
