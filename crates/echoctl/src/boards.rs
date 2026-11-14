//! A thread's and a client's own dashboards, beside the Brief and the Overview (which stay the first, each page's own):
//! each one is on every thread (every client), its layout shared (`today_layout::Page::Board`), its widgets showing
//! the page they are on. Listed in their order in `<config>/dashboards/boards.json`.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::paths;

/// At most this many beside the Brief (the Overview).
const MAX: usize = 12;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub id: String,
    pub name: String,
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
        let b = Board { id, name };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboards_are_added_named_moved_and_removed_per_kind() {
        let (_env, _root) = paths::testenv::folders("boards");
        let a = add(Kind::Client, "Staffing").unwrap();
        assert_eq!(a, Board { id: "staffing".into(), name: "Staffing".into() });
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
}
