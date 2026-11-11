//! Today's layout: which widgets Today shows, in what order, how wide, where a row starts — kept on this computer
//! (`<config>/today-layout.json`). One `place` makes every change, by the rules of the edit-mode prototype
//! (Workbench, `WidgetsGallery`); the CLI (`echoctl today …`) and, later, edit mode change it through it (#323).

use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::{paths, widgets};

/// A widget on Today: which, how wide (S, M, L: 1, 2, 3 columns), and whether it starts a new row.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Item {
    pub widget: String,
    pub size: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub row: bool,
    /// Its settings the user set (the rest are the widget's defaults: `params_of`).
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub params: serde_json::Map<String, serde_json::Value>,
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

fn file() -> PathBuf {
    paths::config_dir().join("today-layout.json")
}

fn item(widget: &str, size: &str, row: bool) -> Item {
    Item { widget: widget.into(), size: size.into(), row, params: Default::default() }
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
    normalize(std::fs::read(file()).ok().and_then(|b| serde_json::from_slice::<File>(&b).ok()).and_then(|f| f.items))
}

/// What Today shows of a saved list (None: the default).
fn normalize(saved: Option<Vec<Item>>) -> Vec<Item> {
    let mut items = saved.unwrap_or_else(default_items);
    for i in &mut items {
        if let Ok(w) = widgets::find(&i.widget) {
            if !w.sizes.contains(&i.size.as_str()) {
                i.size = w.sizes[0].into();
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
    if let Some(i) = out.iter().position(|x| x.widget == widget) {
        let gone = out.remove(i);
        if gone.row {
            if let Some(next) = out.get_mut(i) {
                next.row = true;
            }
        }
    }
    let mut new = item(widget, size, false);
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

/// A widget's size, checked: one it takes (the default: M if it takes it, else its first).
fn size_of(widget: &str, size: Option<&str>) -> Result<String> {
    let w = widgets::find(widget)?;
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

/// Changes the saved layout (starting from what Today shows) under its lock, and returns Today's widgets after it.
fn change(f: impl FnOnce(Vec<Item>) -> Result<Vec<Item>>) -> Result<Vec<Item>> {
    std::fs::create_dir_all(paths::config_dir())?;
    // A file that can't be read (a hand edit gone wrong) is kept beside it, and the change starts from the default —
    // as the page already shows.
    if std::fs::read(file()).is_ok_and(|b| serde_json::from_slice::<File>(&b).is_err()) {
        std::fs::rename(file(), file().with_extension("unreadable.json"))?;
    }
    crate::calls::update_state(&file(), |v: &mut File| {
        // What it holds now, read under the lock: two changes at once both count.
        let next = f(normalize(v.items.take()))?;
        let bytes = serde_json::to_vec(&next)?.len();
        if bytes > MAX_BYTES {
            bail!("Today's layout would be {bytes} bytes, over {MAX_BYTES}");
        }
        v.items = Some(next);
        Ok(())
    })?;
    Ok(load())
}

pub fn add(widget: &str, size: Option<&str>, target: Option<(&str, Side)>) -> Result<Vec<Item>> {
    let size = size_of(widget, size)?;
    change(|items| {
        if items.iter().any(|i| i.widget == widget) {
            bail!("{widget} is on Today already: `echoctl today move` or `today size` change it");
        }
        on_today(&items, target)?;
        Ok(place(&items, widget, &size, target))
    })
}

pub fn move_to(widget: &str, target: Option<(&str, Side)>) -> Result<Vec<Item>> {
    change(|items| {
        let cur = items.iter().find(|i| i.widget == widget).ok_or_else(|| anyhow::anyhow!("{widget} isn't on Today"))?.clone();
        on_today(&items, target)?;
        if target.is_some_and(|(t, _)| t == widget) {
            return Ok(items);
        }
        Ok(place(&items, widget, &cur.size, target))
    })
}

pub fn resize(widget: &str, size: &str) -> Result<Vec<Item>> {
    let size = size_of(widget, Some(size))?;
    change(|mut items| {
        let i = items.iter_mut().find(|i| i.widget == widget).ok_or_else(|| anyhow::anyhow!("{widget} isn't on Today"))?;
        i.size = size;
        Ok(items)
    })
}

/// Set one of a widget's settings on Today (`value` as text: a choice's option, true / false, a thread's or a client's
/// id), refusing a key it doesn't have or a value it doesn't take.
pub fn set_param(widget: &str, key: &str, value: &str) -> Result<Vec<Item>> {
    let w = widgets::find(widget)?;
    let def = w.params.iter().find(|p| p.key == key).ok_or_else(|| {
        let keys: Vec<&str> = w.params.iter().map(|p| p.key.as_str()).collect();
        anyhow::anyhow!("{widget} has no setting {key:?}{}", if keys.is_empty() { String::new() } else { format!(" (it has {})", keys.join(", ")) })
    })?;
    let v = if def.kind == "bool" { serde_json::Value::Bool(matches!(value, "true" | "yes" | "on")) } else { serde_json::Value::String(value.to_string()) };
    if !def.takes(&v) {
        bail!("{widget}'s {key} doesn't take {value:?}{}", if def.kind == "choice" { format!(" ({})", def.options.join(", ")) } else { String::new() });
    }
    change(|mut items| {
        let i = items.iter_mut().find(|i| i.widget == widget).ok_or_else(|| anyhow::anyhow!("{widget} isn't on Today"))?;
        i.params.insert(key.into(), v.clone());
        Ok(items)
    })
}

/// A widget's settings as Today has them: its defaults, then what the user set (a value it no longer takes, dropped).
pub fn params_of(widget: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut out = serde_json::Map::new();
    let Ok(w) = widgets::find(widget) else { return out };
    let set = load().into_iter().find(|i| i.widget == widget).map(|i| i.params).unwrap_or_default();
    for p in &w.params {
        if let Some(v) = set.get(&p.key).filter(|v| p.takes(v)).or(p.default.as_ref()) {
            out.insert(p.key.clone(), v.clone());
        }
    }
    out
}

pub fn remove(widget: &str) -> Result<Vec<Item>> {
    change(|items| {
        let i = items.iter().position(|i| i.widget == widget).ok_or_else(|| anyhow::anyhow!("{widget} isn't on Today"))?;
        let mut out = items;
        let gone = out.remove(i);
        if gone.row {
            if let Some(next) = out.get_mut(i) {
                next.row = true;
            }
        }
        Ok(out)
    })
}

pub fn reset() -> Result<Vec<Item>> {
    change(|_| Ok(default_items()))
}

fn on_today(items: &[Item], target: Option<(&str, Side)>) -> Result<()> {
    match target {
        Some((t, _)) if !items.iter().any(|i| i.widget == t) => bail!("{t} isn't on Today: `echoctl today` lists what is"),
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
        assert!(err(set_param("core:tasks.done", "colour", "red")).contains("has no setting \"colour\" (it has period)"));
        assert!(err(set_param("core:tasks.mine", "period", "month")).contains("has no setting"));
        assert!(err(set_param("core:tasks.done", "period", "month")).contains("isn't on Today"), "only a widget on Today");
        add("core:tasks.done", Some("S"), None).unwrap();
        set_param("core:tasks.done", "period", "month").unwrap();
        assert_eq!(params_of("core:tasks.done")["period"], "month");
        assert_eq!(load().iter().find(|i| i.widget == "core:tasks.done").unwrap().params["period"], "month", "kept on disk");
    }

    #[test]
    fn the_default_is_todays_page_and_a_missing_or_unreadable_file_gives_it() {
        let (_env, _) = paths::testenv::folders("today-layout-default");
        assert_eq!(show(&load()), "| core:calendar.agenda | core:calendar.next core:needs.now | core:calendar.choices | core:routines.cards | core:tasks.mine");
        std::fs::create_dir_all(paths::config_dir()).unwrap();
        std::fs::write(file(), "{ not json").unwrap();
        assert_eq!(load(), default_items());
        // A change starts from the default and keeps the unreadable file beside it.
        assert_eq!(show(&remove("core:routines.cards").unwrap()), "| core:calendar.agenda | core:calendar.next core:needs.now | core:calendar.choices | core:tasks.mine");
        assert_eq!(std::fs::read_to_string(file().with_extension("unreadable.json")).unwrap(), "{ not json");
        // A widget the catalog doesn't have now (a plugin off) keeps its place through a change.
        std::fs::write(file(), r#"{"items":[{"widget":"plugin:x:y","size":"M","row":true},{"widget":"core:tasks.mine","size":"M"}]}"#).unwrap();
        assert_eq!(show(&resize("core:tasks.mine", "S").unwrap()), "| plugin:x:y core:tasks.mine");
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
