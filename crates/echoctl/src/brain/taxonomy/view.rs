//! The taxonomy: stats, the view and the user's edits (split out of `taxonomy.rs`, moved as it was).

use super::*;

pub(super) fn stats_of(c: &Connection, scope: Option<(&str, &str)>) -> Result<Stats> {
    let mut out = Stats::default();
    let (sc, sp): (&str, Vec<&str>) = match scope {
        Some((k, v)) => ("WHERE EXISTS (SELECT 1 FROM scopes x WHERE x.chunk_id = k.chunk_id AND x.kind = ?1 AND x.id = ?2)", vec![k, v]),
        None => ("", vec![]),
    };
    let mut st = c.prepare(&format!("SELECT k.category_id, count(DISTINCT k.chunk_id) FROM chunk_topics k {sc} GROUP BY 1"))?;
    for row in st.query_map(rusqlite::params_from_iter(sp.iter()), |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
        let (id, n) = row?;
        out.sections.insert(id, n);
    }
    let mut st = c.prepare(&format!("SELECT k.category_id, s.kind, s.id, count(DISTINCT k.chunk_id) FROM chunk_topics k JOIN scopes s ON s.chunk_id = k.chunk_id {sc} GROUP BY 1, 2, 3"))?;
    for row in st.query_map(rusqlite::params_from_iter(sp.iter()), |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?)))? {
        let (id, kind, sid, n) = row?;
        if kind == "thread" {
            out.threads.entry(id).or_default().insert(sid);
        } else {
            out.clients.entry(id).or_default().insert(sid, n);
        }
    }
    let mut st = c.prepare(&format!(
        "SELECT k.category_id, b.category_id, count(DISTINCT k.chunk_id) FROM chunk_topics k JOIN chunk_topics b ON b.chunk_id = k.chunk_id AND b.kind = 'intent_l1'
         {} k.kind = 'term' GROUP BY 1, 2",
        if sc.is_empty() { "WHERE" } else { "WHERE EXISTS (SELECT 1 FROM scopes x WHERE x.chunk_id = k.chunk_id AND x.kind = ?1 AND x.id = ?2) AND" }
    ))?;
    for row in st.query_map(rusqlite::params_from_iter(sp.iter()), |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))? {
        out.together.push(row?);
    }
    Ok(out)
}

/// A store's stats in a scope (`store::kept`).
pub fn stats(scope: store::Scope) -> Result<std::sync::Arc<Stats>> {
    let path = super::super::store_path();
    let owned: Option<(String, String)> = scope.map(|(k, v)| (k.to_string(), v.to_string()));
    store::kept(format!("stats|{}|{scope:?}", path.display()), store::changed_stamp(&path), move || {
        stats_of(&store::open(&path)?, owned.as_ref().map(|(k, v)| (k.as_str(), v.as_str())))
    })
}

/// Every layer's drift (`store::kept`: it reads every verdict since each build).
pub(super) fn drift_kept(t: &Taxonomy, snoozed: &Value) -> Result<std::sync::Arc<Vec<Value>>> {
    let path = super::super::store_path();
    let fp = format!("{}|{}|{}", store::changed_stamp(&path), t.version, snoozed);
    let (t, snoozed) = (t.clone(), snoozed.clone());
    store::kept(format!("drift|{}", path.display()), fp, move || drift_in(&store::open(&path)?, &t, &snoozed))
}

pub(super) fn coverage_kept() -> Result<std::sync::Arc<Value>> {
    let path = super::super::store_path();
    store::kept(format!("coverage|{}", path.display()), store::changed_stamp(&path), move || coverage(&store::open(&path)?))
}

/// The taxonomy as the window and MCP show it.
pub fn view() -> Result<Value> {
    view_in(None)
}

/// The layers a scope sees: all (none), shared + the client's (a client), shared + its thread's client's.
pub(super) fn scope_layers(t: &Taxonomy, scope: store::Scope) -> Option<BTreeSet<String>> {
    let (k, v) = scope?;
    let client = if k == "client" { Some(v.to_string()) } else { crate::projects::load(v).ok().and_then(|p| p.client) };
    let mut out: BTreeSet<String> = [SHARED.to_string()].into();
    if let Some(c) = client.filter(|c| t.layers.contains_key(c)) {
        out.insert(c);
    }
    Some(out)
}

/// Every topic and term with how many sections it has (within `scope` when one is given) and the
/// clients and threads where it comes up (`-`: calls in none); a scope sees the shared layer and its client's.
pub fn view_in(scope: store::Scope) -> Result<Value> {
    let Some(t) = current() else { return Ok(json!({"exists": false})) };
    let st = stats(scope)?;
    let all = if scope.is_some() { stats(None)? } else { st.clone() };
    let empty = BTreeSet::new();
    let count = |id: &str| -> (i64, BTreeSet<String>, BTreeSet<String>) {
        (st.sections.get(id).copied().unwrap_or(0), st.threads.get(id).unwrap_or(&empty).clone(), st.clients.get(id).map(|m| m.keys().cloned().collect()).unwrap_or_default())
    };
    let layers = scope_layers(&t, scope);
    let visible = |l: &String| layers.as_ref().is_none_or(|s| s.contains(l));
    // Only in closed threads' sections (#55): the archive — listed apart, out of transcription hints; still resolved.
    let closed = crate::projects::closed_threads();
    let archived = |th: &BTreeSet<String>| -> Value {
        if th.is_empty() || !th.iter().all(|t| closed.contains_key(t)) {
            return Value::Null;
        }
        Value::Array(th.iter().filter_map(|t| closed.get(t).cloned()).collect())
    };
    let together = &st.together;
    let label_of: BTreeMap<String, String> = t.topics.iter().map(|x| (topic_id(&x.layer, &x.label), x.label.clone())).collect();
    let name_of: BTreeMap<String, String> = t.terms.iter().map(|x| (term_id(&x.layer, &x.name), x.name.clone())).collect();
    // Indexed both ways (tens of thousands of terms: never a scan of every pair for each).
    let mut by_topic: BTreeMap<&str, Vec<(String, i64)>> = BTreeMap::new();
    let mut by_term: BTreeMap<&str, Vec<(String, i64)>> = BTreeMap::new();
    for (tm, tp, n) in together {
        by_topic.entry(tp.as_str()).or_default().push((tm.clone(), *n));
        by_term.entry(tm.as_str()).or_default().push((tp.clone(), *n));
    }
    let top = |rows: Vec<(String, i64)>, names: &BTreeMap<String, String>, k: usize| -> Vec<Value> {
        let mut rows: Vec<(String, i64)> = rows.into_iter().filter(|(id, _)| names.contains_key(id)).collect();
        rows.sort_by_key(|r| std::cmp::Reverse(r.1));
        rows.into_iter().take(k).map(|(id, n)| json!({"id": id, "label": names[&id], "sections": n})).collect()
    };
    let topics: Vec<Value> = t
        .topics
        .iter()
        .filter(|x| visible(&x.layer))
        .map(|x| {
            let id = topic_id(&x.layer, &x.label);
            let (n, th, cl) = count(&id);
            let parent = parent_of(&t, x);
            let terms = top(by_topic.get(id.as_str()).cloned().unwrap_or_default(), &name_of, 6);
            json!({"id": id, "layer": x.layer, "label": x.label, "level": x.level, "parent": x.parent, "parent_id": parent.map(|p| topic_id(&p.layer, &p.label)), "parent_layer": parent.map(|p| &p.layer),
                   "description": x.description, "synonyms": x.synonyms, "sections": n, "archived": archived(&th), "threads": th, "clients": cl, "terms": terms})
        })
        .collect();
    let terms: Vec<Value> = t
        .terms
        .iter()
        .filter(|x| visible(&x.layer))
        .map(|x| {
            let id = term_id(&x.layer, &x.name);
            let (n, mut th, cl) = count(&id);
            th.extend(x.threads.iter().cloned());
            let topics = top(by_term.get(id.as_str()).cloned().unwrap_or_default(), &label_of, 3);
            json!({"id": id, "layer": x.layer, "name": x.name, "kind": x.kind, "group": x.group(), "group_set": !x.group.is_empty(), "synonyms": x.synonyms, "variants": x.variants,
                   "description": x.description, "pinned": x.threads, "always": x.always, "learned": x.learned.is_some() || x.kind == "learned", "sections": n,
                   "archived": if x.always { Value::Null } else { archived(&th) }, "threads": th, "clients": cl, "topics": topics})
        })
        .collect();
    let lv: Vec<Value> = t
        .layers
        .iter()
        .filter(|(k, _)| visible(k))
        .map(|(k, l)| json!({"id": k, "name": l.name, "goal": l.goal, "audience": l.audience, "summary": l.summary, "demoted": l.demoted, "provisional": l.provisional, "built_from": l.built_from}))
        .collect();
    // Each layer's drift (out of date or not), for its banner.
    let snoozed: Value = std::fs::read(drift_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
    let mut lv = lv;
    if let Ok(d) = drift_kept(&t, &snoozed) {
        for x in lv.iter_mut() {
            x["drift"] = d.iter().find(|y| y["layer"] == x["id"]).cloned().unwrap_or(Value::Null);
        }
    }
    let moves = moves_in(&all, &t, &dismissed_moves());
    Ok(json!({"exists": true, "version": t.version, "provisional": t.provisional(), "layers": lv, "scope": scope.map(|s| json!({"kind": s.0, "id": s.1})), "moves": moves,
              "groups": GROUPS.iter().map(|(k, l)| json!({"id": k, "label": l})).collect::<Vec<_>>(),
              "topics": topics, "terms": terms, "coverage": coverage_kept()?.as_ref().clone(), "review_flags": t.review_flags, "history": t.history}))
}

/// The terms that hint a call's transcription: pinned to its threads, always-on ones, and its clients'
/// layers' terms (the most mentioned first), then shared terms mentioned in its threads.
pub fn terms_for_threads(threads: &[String]) -> Vec<String> {
    let Some(t) = current() else { return vec![] };
    let clients: BTreeSet<String> = threads.iter().filter_map(|id| crate::projects::load(id).ok().and_then(|p| p.client)).collect();
    let Ok(v) = view() else { return vec![] };
    let all: Vec<&Value> = v["terms"].as_array().into_iter().flatten().collect();
    let mut out: Vec<String> = vec![];
    let mut add = |name: &str| {
        if !out.iter().any(|x| x.eq_ignore_ascii_case(name)) {
            out.push(name.to_string());
        }
    };
    let in_threads = |x: &Value| x["threads"].as_array().into_iter().flatten().any(|t| threads.iter().any(|w| t == w.as_str()));
    for x in &all {
        if x["always"] == true || x["pinned"].as_array().into_iter().flatten().any(|t| threads.iter().any(|w| t == w.as_str())) {
            add(x["name"].as_str().unwrap_or(""));
        }
    }
    // A closed thread's own terms stay out of a new call's hint.
    let mut mine: Vec<&&Value> = all.iter().filter(|x| x["layer"].as_str().is_some_and(|l| clients.contains(l)) && x["archived"].is_null()).collect();
    mine.sort_by_key(|x| std::cmp::Reverse((in_threads(x), x["sections"].as_i64().unwrap_or(0))));
    for x in mine {
        add(x["name"].as_str().unwrap_or(""));
    }
    for x in all.iter().filter(|x| x["layer"] == SHARED && in_threads(x)) {
        add(x["name"].as_str().unwrap_or(""));
    }
    let _ = t;
    out.retain(|x| !x.is_empty());
    out
}

/// The user's review (Brain's rule: Claude only adds; these are the user's): rename, merge into,
/// move (a level-2 topic under another), remove, add, set, accept (a layer, or all). `layer` picks
/// the layer when a name is in more than one. Each is a new version; renames and merges keep tags.
pub fn edit(op: &Value) -> Result<Taxonomy> {
    let _lock = write_lock()?;
    let mut t = current().ok_or_else(|| anyhow!("there is no taxonomy yet"))?;
    let what = op["op"].as_str().unwrap_or("");
    let s = |k: &str| op[k].as_str().map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
    let is_term = op["term"].as_bool().unwrap_or(false);
    if what == "accept" {
        match s("layer") {
            Some(l) => t.layers.get_mut(&l).ok_or_else(|| anyhow!("no layer {l:?}"))?.provisional = false,
            None => t.layers.values_mut().for_each(|l| l.provisional = false),
        }
        return save(t, json!({"op": "accept", "by": "you", "layer": s("layer")}));
    }
    let target = s("label").or_else(|| s("name")).ok_or_else(|| anyhow!("which topic or term?"))?;
    // The layer: given, else the one layer that has this name.
    let layer = match s("layer") {
        Some(l) => l,
        None if what == "add" => SHARED.to_string(),
        None => {
            let ls: BTreeSet<String> = if is_term {
                t.terms.iter().filter(|x| nid(&x.name) == nid(&target)).map(|x| x.layer.clone()).collect()
            } else {
                t.topics.iter().filter(|x| nid(&x.label) == nid(&target)).map(|x| x.layer.clone()).collect()
            };
            match ls.len() {
                0 => bail!("no {} {target:?}", if is_term { "term" } else { "topic" }),
                1 => ls.into_iter().next().unwrap(),
                _ => bail!("{target:?} is in more than one layer ({}): say which", ls.into_iter().collect::<Vec<_>>().join(", ")),
            }
        }
    };
    let key = |x: &str| if is_term { term_id(&layer, x) } else { topic_id(&layer, x) };
    let same = |a: &str, b: &str| nid(a) == nid(b);
    match what {
        "add" if is_term => {
            if t.terms.iter().any(|x| x.layer == layer && same(&x.name, &target)) {
                bail!("{target:?} is a term already");
            }
            t.terms.push(Term { layer: layer.clone(), name: target.clone(), kind: s("kind").unwrap_or_else(|| "other".into()), description: s("description").unwrap_or_default(),
                                variants: op["variants"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect(),
                                threads: op["threads"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect(), always: op["always"] == true, learned: None,
                                synonyms: op["synonyms"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect(),
                                group: s("group").filter(|g| GROUPS.iter().any(|(k, _)| k == g)).unwrap_or_default() });
        }
        "add" => {
            if t.topics.iter().any(|x| x.layer == layer && same(&x.label, &target)) {
                bail!("{target:?} is a topic already");
            }
            let parent = s("parent");
            t.topics.push(Topic { layer: layer.clone(), label: target.clone(), level: if parent.is_some() { 2 } else { 1 }, parent, description: s("description").unwrap_or_default(), synonyms: vec![] });
        }
        "rename" => {
            let to = s("to").ok_or_else(|| anyhow!("rename to what?"))?;
            if is_term {
                let x = t.terms.iter_mut().find(|x| x.layer == layer && same(&x.name, &target)).ok_or_else(|| anyhow!("no term {target:?}"))?;
                let old = std::mem::replace(&mut x.name, to.clone());
                x.synonyms.retain(|s| nid(s) != nid(&to));
                x.variants.retain(|s| nid(s) != nid(&to));
                if !x.synonyms.iter().any(|s| nid(s) == nid(&old)) {
                    x.synonyms.push(old);
                }
            } else {
                let x = t.topics.iter_mut().find(|x| x.layer == layer && same(&x.label, &target)).ok_or_else(|| anyhow!("no topic {target:?}"))?;
                let old = std::mem::replace(&mut x.label, to.clone());
                x.synonyms.retain(|s| nid(s) != nid(&to));
                add_synonym(x, &old);
                for k in t.topics.iter_mut().filter(|k| k.layer == layer && k.parent.as_deref().is_some_and(|p| same(p, &target))) {
                    k.parent = Some(to.clone());
                }
            }
            set_alias(&mut t.aliases, key(&target), key(&to));
        }
        "merge" => {
            let into = s("into").ok_or_else(|| anyhow!("merge into what?"))?;
            if is_term {
                let into_layer = s("into_layer").unwrap_or_else(|| layer.clone());
                if into_layer != layer && into_layer != SHARED {
                    bail!("a term merges into its own layer's or the practice's");
                }
                let from = t.terms.iter().position(|x| x.layer == layer && same(&x.name, &target)).ok_or_else(|| anyhow!("no term {target:?}"))?;
                let from = t.terms.remove(from);
                let x = t.terms.iter_mut().find(|x| x.layer == into_layer && same(&x.name, &into)).ok_or_else(|| anyhow!("no term {into:?}"))?;
                // Its name and other names become this one's; its spellings stay spellings.
                for n in std::iter::once(from.name.clone()).chain(from.synonyms.clone()) {
                    if !x.is(&n) {
                        x.synonyms.push(n);
                    }
                }
                for v in from.variants.clone() {
                    if !x.is(&v) {
                        x.variants.push(v);
                    }
                }
                x.threads.extend(from.threads);
                x.threads.dedup();
                x.always |= from.always;
                if x.description.is_empty() {
                    x.description = from.description;
                }
                if into_layer != layer {
                    set_alias(&mut t.aliases, key(&target), term_id(&into_layer, &into));
                    return save(t, json!({"op": "merge", "by": "you", "layer": into_layer, "target": target, "change": op}));
                }
            } else {
                // Into a topic of this layer, or (a client's topic) into one of the practice's.
                let into_layer = s("into_layer").unwrap_or_else(|| layer.clone());
                if into_layer != layer && into_layer != SHARED {
                    bail!("a topic merges into its own layer's or the practice's");
                }
                let from = t.topics.iter().position(|x| x.layer == layer && same(&x.label, &target)).ok_or_else(|| anyhow!("no topic {target:?}"))?;
                let from = t.topics.remove(from);
                let x = t.topics.iter_mut().find(|x| x.layer == into_layer && same(&x.label, &into)).ok_or_else(|| anyhow!("no topic {into:?}"))?;
                add_synonym(x, &from.label);
                for syn in &from.synonyms {
                    add_synonym(x, syn);
                }
                // Its subtopics go under it — or, when it is a subtopic itself, under its level-1 topic.
                let home = if x.level == 1 { into.clone() } else { x.parent.clone().unwrap_or_else(|| into.clone()) };
                for k in t.topics.iter_mut().filter(|k| k.layer == layer && k.parent.as_deref().is_some_and(|p| same(p, &target))) {
                    k.parent = Some(home.clone());
                }
                set_alias(&mut t.aliases, key(&target), topic_id(&into_layer, &into));
                let layer = into_layer;
                t.layers.entry(layer.clone()).or_insert_with(|| {
                    let (name, goal, audience) = layer_facts(&layer);
                    Layer { name, goal, audience, ..Default::default() }
                });
                return save(t, json!({"op": "merge", "by": "you", "layer": layer, "target": target, "change": op}));
            }
            set_alias(&mut t.aliases, key(&target), key(&into));
        }
        "move" => {
            let to = s("parent");
            let x = t.topics.iter_mut().find(|x| x.layer == layer && same(&x.label, &target)).ok_or_else(|| anyhow!("no topic {target:?}"))?;
            x.level = if to.is_some() { 2 } else { 1 };
            x.parent = to;
        }
        "remove" => {
            if is_term {
                t.terms.retain(|x| !(x.layer == layer && same(&x.name, &target)));
            } else {
                t.topics.retain(|x| !(x.layer == layer && same(&x.label, &target)));
                for k in t.topics.iter_mut().filter(|k| k.layer == layer && k.parent.as_deref().is_some_and(|p| same(p, &target))) {
                    k.level = 1;
                    k.parent = None;
                }
            }
        }
        "set" if is_term => {
            let x = t.terms.iter_mut().find(|x| x.layer == layer && same(&x.name, &target)).ok_or_else(|| anyhow!("no term {target:?}"))?;
            if let Some(k) = s("kind") {
                x.kind = k.to_lowercase();
            }
            if let Some(d) = op.get("description").and_then(|d| d.as_str()) {
                x.description = d.trim().into();
            }
            if let Some(v) = op["variants"].as_array() {
                x.variants = v.iter().filter_map(|x| x.as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty()).collect();
            }
            if let Some(th) = op["threads"].as_array() {
                x.threads = th.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect();
            }
            if let Some(a) = op["always"].as_bool() {
                x.always = a;
            }
            if let Some(v) = op["synonyms"].as_array() {
                x.synonyms = v.iter().filter_map(|x| x.as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty() && nid(s) != nid(&target)).collect();
            }
            if let Some(g) = s("group") {
                anyhow::ensure!(GROUPS.iter().any(|(k, _)| *k == g), "a group is one of {:?}", GROUPS.map(|(k, _)| k));
                x.group = g;
            }
        }
        // One of a term's names is a thing of its own: a new term by that name (same layer and kind).
        "split" if is_term => {
            let alias = s("alias").ok_or_else(|| anyhow!("split off which name?"))?;
            let x = t.terms.iter_mut().find(|x| x.layer == layer && same(&x.name, &target)).ok_or_else(|| anyhow!("no term {target:?}"))?;
            let before = x.synonyms.len() + x.variants.len();
            x.synonyms.retain(|s| !same(s, &alias));
            x.variants.retain(|s| !same(s, &alias));
            if x.synonyms.len() + x.variants.len() == before {
                bail!("{alias:?} isn't one of {target:?}'s names");
            }
            let n = Term { layer: layer.clone(), name: alias.clone(), kind: x.kind.clone(), group: x.group.clone(), ..Default::default() };
            t.terms.push(n);
        }
        "scope" if is_term => {
            let to = s("to").ok_or_else(|| anyhow!("to the practice (shared) or which client?"))?;
            if to == layer || (to != SHARED && layer != SHARED) {
                bail!("a term moves between the practice and one client");
            }
            if t.terms.iter().any(|x| x.layer == to && same(&x.name, &target)) {
                bail!("{target:?} is a term there already: merge them instead");
            }
            let x = t.terms.iter_mut().find(|x| x.layer == layer && same(&x.name, &target)).ok_or_else(|| anyhow!("no term {target:?}"))?;
            x.layer = to.clone();
            set_alias(&mut t.aliases, term_id(&layer, &target), term_id(&to, &target));
            t.layers.entry(to.clone()).or_insert_with(|| {
                let (name, goal, audience) = layer_facts(&to);
                Layer { name, goal, audience, ..Default::default() }
            });
        }
        "set" => {
            let x = t.topics.iter_mut().find(|x| x.layer == layer && same(&x.label, &target)).ok_or_else(|| anyhow!("no topic {target:?}"))?;
            if let Some(d) = op.get("description").and_then(|d| d.as_str()) {
                x.description = d.trim().into();
            }
            if let Some(v) = op["synonyms"].as_array() {
                x.synonyms.clear();
                for syn in v.iter().filter_map(|x| x.as_str()) {
                    add_synonym(x, syn);
                }
            }
        }
        // A topic moved between the practice and a client (up: every client's; down: one client's), its
        // subtopics with it; its tags follow (a client's topic leaves other clients' sections: `build_graph`).
        "scope" => {
            let to = s("to").ok_or_else(|| anyhow!("to the practice (shared) or which client?"))?;
            if to == layer {
                bail!("{target:?} is there already");
            }
            if to != SHARED && layer != SHARED {
                bail!("from one client to another: move it up to the practice first");
            }
            if t.topics.iter().any(|x| x.layer == to && same(&x.label, &target)) {
                bail!("{target:?} is a topic there already: merge them instead");
            }
            let i = t.topics.iter().position(|x| x.layer == layer && same(&x.label, &target)).ok_or_else(|| anyhow!("no topic {target:?}"))?;
            let moving: Vec<usize> = std::iter::once(i).chain((0..t.topics.len()).filter(|j| t.topics[*j].layer == layer && t.topics[*j].parent.as_deref().is_some_and(|p| same(p, &target)))).collect();
            for j in moving {
                let old = topic_id(&t.topics[j].layer, &t.topics[j].label);
                t.topics[j].layer = to.clone();
                let new = topic_id(&to, &t.topics[j].label);
                set_alias(&mut t.aliases, old, new);
            }
            // Up to the practice: a parent that was the client's own isn't one there.
            if to == SHARED {
                let x = &t.topics[i];
                if x.level == 2 && !t.topics.iter().any(|y| y.layer == SHARED && y.level == 1 && x.parent.as_deref().is_some_and(|p| same(&y.label, p))) {
                    let x = &mut t.topics[i];
                    x.level = 1;
                    x.parent = None;
                }
            }
            // Down to a client: other clients' topics that sat under it become their own level-1 topics.
            if to != SHARED {
                for k in t.topics.iter_mut().filter(|k| k.layer != to && k.layer != SHARED && k.parent.as_deref().is_some_and(|p| same(p, &target))) {
                    k.level = 1;
                    k.parent = None;
                }
            }
            t.layers.entry(to.clone()).or_insert_with(|| {
                let (name, goal, audience) = layer_facts(&to);
                Layer { name, goal, audience, ..Default::default() }
            });
        }
        o => bail!("no taxonomy change {o:?}"),
    }
    t.layers.entry(layer.clone()).or_insert_with(|| {
        let (name, goal, audience) = layer_facts(&layer);
        Layer { name, goal, audience, ..Default::default() }
    });
    save(t, json!({"op": what, "by": "you", "layer": layer, "target": target, "change": op}))
}
