//! Links between whole threads (#95): one thread is part of another (a workstream of a bigger program or product), or
//! two are related. A link is kept in one thread's profile only — the part's for part of, the side it was made from
//! for related — and the other side is read from every profile (`of`). Items link across threads in the ledger
//! instead (`ledger::link_threads`).

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::projects::{self, Project};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    PartOf,
    Related,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Kind> {
        match s {
            "part-of" | "part_of" => Ok(Kind::PartOf),
            "related" => Ok(Kind::Related),
            _ => bail!("kind is part-of or related"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub to: String,
    pub kind: Kind,
}

/// Link `id` to `to`: part of it (a thread is part of one other at most: a new parent replaces the old) or related.
/// Checked under the projects lock, against the profiles as they are then.
pub fn link(id: &str, to: &str, kind: Kind) -> Result<Project> {
    if id == to {
        bail!("a thread can't be linked to itself");
    }
    projects::update(id, |p| {
        let all = projects::list();
        let Some(other) = all.iter().find(|o| o.id == to) else { bail!("no thread {to:?}") };
        if other.links.iter().any(|l| l.to == id) {
            bail!("{} already links to this thread: change it there (`project link {to} --to {id} --remove`)", other.name);
        }
        if kind == Kind::PartOf {
            // Up from the new parent: reaching this thread would make a loop.
            let mut at = to.to_string();
            for _ in 0..all.len() {
                match all.iter().find(|o| o.id == at).and_then(|o| o.links.iter().find(|l| l.kind == Kind::PartOf)) {
                    Some(l) if l.to == id => bail!("{} is part of this thread already (a loop)", other.name),
                    Some(l) => at = l.to.clone(),
                    None => break,
                }
            }
        }
        p.links.retain(|l| l.to != to && !(kind == Kind::PartOf && l.kind == Kind::PartOf));
        p.links.push(Link { to: to.to_string(), kind });
        Ok(())
    })
}

/// Take the link between `id` and `to` away, whichever side keeps it.
pub fn unlink(id: &str, to: &str) -> Result<Project> {
    projects::load(id)?;
    let mut there = false;
    if projects::load(to).is_ok() {
        projects::update(to, |o| {
            there = o.links.iter().any(|l| l.to == id);
            o.links.retain(|l| l.to != id);
            Ok(())
        })?;
    }
    if there {
        return projects::load(id);
    }
    projects::update(id, |p| {
        let before = p.links.len();
        p.links.retain(|l| l.to != to);
        if p.links.len() == before {
            bail!("{id} and {to} aren't linked");
        }
        Ok(())
    })
}

/// The AI profile a thread's jobs run on.
pub fn profile_of(p: &Project) -> String {
    p.ai_profile.clone().unwrap_or_else(crate::ai::default_id)
}

/// A thread's links seen from it, both sides (`all`: every thread): `part_of` its parent, `part` a thread that is part
/// of it, `related`. A link to a thread that is gone is left out. Parent first, then parts, then related, by name.
pub fn of(p: &Project, all: &[Project]) -> Vec<Value> {
    let row = |o: &Project, kind: &str| json!({"id": o.id, "name": o.name, "kind": kind, "stage": o.stage, "closed": o.stage.closed(), "color": o.color, "client": o.client});
    let mut out: Vec<(u8, String, Value)> = vec![];
    for l in &p.links {
        if let Some(o) = all.iter().find(|o| o.id == l.to) {
            let (rank, kind) = if l.kind == Kind::PartOf { (0, "part_of") } else { (2, "related") };
            out.push((rank, o.name.to_lowercase(), row(o, kind)));
        }
    }
    for o in all.iter().filter(|o| o.id != p.id) {
        if let Some(l) = o.links.iter().find(|l| l.to == p.id) {
            let (rank, kind) = if l.kind == Kind::PartOf { (1, "part") } else { (2, "related") };
            out.push((rank, o.name.to_lowercase(), row(o, kind)));
        }
    }
    out.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    out.into_iter().map(|(_, _, v)| v).collect()
}

/// A thread's view as a run Echo started reads it (`run`: the run's AI profile): only links to threads on that profile,
/// by name, without their work; the others not at all.
pub fn for_run(v: &mut Value, run: &str) {
    if let Some(ls) = v["links"].as_array_mut() {
        ls.retain(|l| projects::load(l["id"].as_str().unwrap_or("")).is_ok_and(|o| !run.is_empty() && profile_of(&o) == run));
        for l in ls.iter_mut() {
            l.as_object_mut().map(|o| o.remove("work"));
        }
    }
}

const OPEN: [&str; 4] = ["open", "in_progress", "waiting", "blocked"];

/// What a linked thread holds for the reader: its open items (count, the first `n_open`: late first, then by due) and
/// its latest `n_dec` decisions, each with its `echo://task` link.
pub fn digest(id: &str, n_open: usize, n_dec: usize) -> Value {
    let Ok(tree) = crate::ledger::tree(id) else { return json!({"open_count": 0, "open": [], "decisions": []}) };
    let today = chrono::Local::now().date_naive().to_string();
    let item = |n: &Value| {
        let owner = n["internal_owner"].as_str().or(n["client_owner"].as_str());
        json!({"kind": n["kind"], "title": n["title"], "owner": owner, "due": n["due"], "late": n["due_guessed"] != true && n["due"].as_str().is_some_and(|d| d < today.as_str()),
            "date": n["created"], "link": format!("echo://task/{id}/{}", n["id"].as_str().unwrap_or(""))})
    };
    let nodes: Vec<&Value> = tree["nodes"].as_object().map(|m| m.values().collect()).unwrap_or_default();
    let mut open: Vec<Value> = nodes.iter().filter(|n| ["task", "risk", "question"].contains(&n["kind"].as_str().unwrap_or("")) && OPEN.contains(&n["state"].as_str().unwrap_or(""))).map(|n| item(n)).collect();
    open.sort_by_key(|x| (!x["late"].as_bool().unwrap_or(false), x["due"].as_str().map(str::to_owned).unwrap_or_else(|| "~".into())));
    let mut dec: Vec<Value> = nodes.iter().filter(|n| n["kind"] == "decision" && !["merged", "deleted", "cancelled"].contains(&n["state"].as_str().unwrap_or(""))).map(|n| item(n)).collect();
    dec.sort_by(|a, b| b["date"].as_str().cmp(&a["date"].as_str()));
    let count = open.len();
    open.truncate(n_open);
    dec.truncate(n_dec);
    json!({"open_count": count, "open": open, "decisions": dec})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::testenv;

    fn kinds(id: &str) -> Vec<(String, String)> {
        let all = projects::list();
        let p = all.iter().find(|p| p.id == id).unwrap();
        of(p, &all).iter().map(|v| (v["id"].as_str().unwrap().to_owned(), v["kind"].as_str().unwrap().to_owned())).collect()
    }

    #[test]
    fn a_link_is_kept_on_one_side_and_seen_from_both() {
        let _env = testenv::folders("threadlinks-sides");
        let w = projects::create("Widget", None, None).unwrap();
        let r = projects::create("Widget rollout", None, None).unwrap();
        let o = projects::create("Ops", None, None).unwrap();
        link(&r.id, &w.id, Kind::PartOf).unwrap();
        link(&o.id, &w.id, Kind::Related).unwrap();
        assert_eq!(projects::load(&w.id).unwrap().links, vec![], "the parent keeps nothing");
        assert_eq!(kinds(&r.id), [(w.id.clone(), "part_of".to_owned())]);
        assert_eq!(kinds(&w.id), [(r.id.clone(), "part".to_owned()), (o.id.clone(), "related".to_owned())]);
        // Unlinked from the side that doesn't keep it.
        unlink(&w.id, &o.id).unwrap();
        assert_eq!(kinds(&o.id), []);
        // A deleted thread is left out.
        projects::delete(&w.id).unwrap();
        assert_eq!(kinds(&r.id), []);
        // A new thread that gets the trashed one's id starts with no links.
        let again = projects::create("Widget", None, None).unwrap();
        assert_eq!(again.id, w.id);
        assert_eq!(kinds(&again.id), []);
        assert_eq!(projects::load(&r.id).unwrap().links, vec![]);
    }

    #[test]
    fn a_run_sees_only_links_on_its_profile_by_name() {
        let _env = testenv::folders("threadlinks-run");
        let a = projects::create("Widget", None, None).unwrap().id;
        let b = projects::create("Widget security", None, None).unwrap().id;
        let c = projects::create("Widget rollout", None, None).unwrap().id;
        projects::update(&b, |p| {
            p.ai_profile = Some("work".into());
            Ok(())
        })
        .unwrap();
        link(&a, &b, Kind::Related).unwrap();
        link(&c, &a, Kind::PartOf).unwrap();
        let mut v = projects::view(&projects::load(&a).unwrap(), true).unwrap();
        assert_eq!(v["links"].as_array().unwrap().len(), 2);
        for_run(&mut v, &crate::ai::default_id());
        let ls = v["links"].as_array().unwrap();
        assert_eq!(ls.iter().map(|l| l["id"].as_str().unwrap()).collect::<Vec<_>>(), [c.as_str()]);
        assert!(ls[0].get("work").is_none());
        let mut v = projects::view(&projects::load(&a).unwrap(), true).unwrap();
        for_run(&mut v, "");
        assert_eq!(v["links"], json!([]), "a run with no profile sees none");
        // A list's view carries no links at all.
        assert!(projects::view(&projects::load(&c).unwrap(), false).unwrap().get("links").is_none());
    }

    #[test]
    fn one_parent_no_loop_no_self_no_pair_twice() {
        let _env = testenv::folders("threadlinks-rules");
        let a = projects::create("A", None, None).unwrap().id;
        let b = projects::create("B", None, None).unwrap().id;
        let c = projects::create("C", None, None).unwrap().id;
        assert!(link(&a, &a, Kind::Related).is_err());
        assert!(link(&a, "nope", Kind::Related).is_err());
        link(&a, &b, Kind::PartOf).unwrap();
        link(&a, &c, Kind::PartOf).unwrap();
        assert_eq!(kinds(&a), [(c.clone(), "part_of".to_owned())], "a new parent replaces the old");
        link(&c, &b, Kind::PartOf).unwrap();
        let e = link(&b, &a, Kind::PartOf).unwrap_err().to_string();
        assert!(e.contains("loop"), "{e}");
        let e = link(&b, &c, Kind::Related).unwrap_err().to_string();
        assert!(e.contains("already links"), "{e}");
        // Related replaces part of between the same two.
        link(&a, &c, Kind::Related).unwrap();
        assert_eq!(kinds(&a), [(c.clone(), "related".to_owned())]);
        assert!(unlink(&a, &b).is_err());
    }
}
