//! The taxonomy: drift, and topics moved between the practice and a client (split out of `taxonomy.rs`, moved as it was).

use super::*;

// ── drift: does a layer's topics still fit its calls? (#47, no model) ────────────────────────────

/// A layer's sections judged since its build, at least, before its drift is judged.
pub(super) const DRIFT_MIN: i64 = 30;
/// Calls a layer was built from, at least, before its sections' fit is judged: a few calls about one
/// subject put most of their sections under one topic, and that is no fault of the topics.
pub(super) const DRIFT_MIN_CALLS: usize = 6;

/// The version a layer was last built in (its `build` in the history) and when, else 0 and "": every
/// section counts.
pub(super) fn last_build(t: &Taxonomy, layer: &str) -> (u64, String) {
    let h = t.history.iter().rev().find(|h| h["op"] == "build" && h["layer"] == layer);
    (h.and_then(|h| h["version"].as_u64()).unwrap_or(0), h.and_then(|h| h["at"].as_str()).unwrap_or("").to_string())
}

pub(super) fn drift_file() -> PathBuf {
    dir().join("drift.json")
}

/// Each layer's drift over the sections that came in since its last build (indexed after it and judged
/// in its version or later — never the build's own sections tagged again): stale when ≥ 30 of them and
/// more than 10% fit no topic, or one topic took more than 40% of the tagged ones (a level-1 topic with
/// subtopics only when one of its subtopics did); those two only for a layer built from 6 calls or more.
/// Or when its calls are 3× those it was built from (and 6 or more). "Not now" snoozes a layer until its
/// sections or its calls grow by half again.
pub(super) fn drift_in(c: &Connection, t: &Taxonomy, snoozed: &Value) -> Result<Vec<Value>> {
    // A section judged since the build, indexed after it (`?3`; "" for a build with no time: all).
    const NEW: &str = "CAST(v.taxonomy_version AS INTEGER) >= ?2 AND (?3 = '' OR EXISTS (SELECT 1 FROM chunks ch WHERE ch.id = v.chunk_id AND coalesce(ch.created_at, '') > ?3))";
    let mut out = vec![];
    for (id, l) in &t.layers {
        let client = if id == SHARED { "-" } else { id.as_str() };
        let (since, at) = last_build(t, id);
        let since = since as i64;
        let (judged, untagged, tagged): (i64, i64, i64) = c.query_row(
            &format!(
                "SELECT count(*), coalesce(sum(v.verdict = 'untagged'), 0), coalesce(sum(v.verdict = 'tagged'), 0) FROM chunk_verdicts v
                 JOIN scopes s ON s.chunk_id = v.chunk_id AND s.kind = 'client' AND s.id = ?1 WHERE {NEW}"
            ),
            params![client, since, at],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        // The new tagged sections per topic of a level, most first.
        let counts = |kind: &str| -> Result<Vec<(String, i64)>> {
            let sql = format!(
                "SELECT k.category_id, count(DISTINCT k.chunk_id) FROM chunk_topics k
                 JOIN chunk_verdicts v ON v.chunk_id = k.chunk_id AND v.verdict = 'tagged' AND {NEW}
                 JOIN scopes s ON s.chunk_id = k.chunk_id AND s.kind = 'client' AND s.id = ?1
                 WHERE k.kind = ?4 GROUP BY 1 ORDER BY 2 DESC, 1"
            );
            Ok(c.prepare(&sql)?.query_map(params![client, since, at, kind], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?)
        };
        let top: Option<(String, i64)> = counts("intent_l1")?.into_iter().next();
        // A level-1 topic split into subtopics is too broad only when one subtopic is: its biggest.
        let of_id = |tid: &str| t.topics.iter().find(|x| topic_id(&x.layer, &x.label) == tid);
        let subs: Vec<String> = match top.as_ref().and_then(|(tid, _)| of_id(tid)) {
            Some(l1) => t.topics.iter().filter(|x| x.level == 2 && parent_of(t, x) == Some(l1)).map(|x| topic_id(&x.layer, &x.label)).collect(),
            None => vec![],
        };
        let sub_top: Option<(String, i64)> = if subs.is_empty() { None } else { Some(counts("intent_l2")?.into_iter().find(|(k, _)| subs.contains(k)).unwrap_or_default()) };
        // Sections tagged with that level-1 topic but none of its subtopics (the classifier may stop at
        // level 1): a bucket of their own, as big a catch-all as any subtopic.
        let direct: i64 = if subs.is_empty() {
            0
        } else {
            let marks = (0..subs.len()).map(|i| format!("?{}", i + 4)).collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT count(DISTINCT k.chunk_id) FROM chunk_topics k
                 JOIN chunk_verdicts v ON v.chunk_id = k.chunk_id AND v.verdict = 'tagged' AND {NEW}
                 JOIN scopes s ON s.chunk_id = k.chunk_id AND s.kind = 'client' AND s.id = ?1
                 WHERE k.kind = 'intent_l2' AND k.category_id IN ({marks})"
            );
            let mut values: Vec<&dyn rusqlite::ToSql> = vec![&client, &since, &at];
            values.extend(subs.iter().map(|x| x as &dyn rusqlite::ToSql));
            let with_sub: i64 = c.query_row(&sql, values.as_slice(), |r| r.get(0))?;
            top.as_ref().map(|(_, n)| (n - with_sub).max(0)).unwrap_or(0)
        };
        let calls: i64 = c.query_row(
            "SELECT count(DISTINCT ch.source) FROM chunks ch JOIN scopes s ON s.chunk_id = ch.id AND s.kind = 'client' AND s.id = ?1
             WHERE ch.source LIKE 'call/%' AND ch.status = 'ACTIVE'",
            [client],
            |r| r.get(0),
        )?;
        let pct = |n: i64, of: i64| if of > 0 { (1000 * n / of) as f64 / 10.0 } else { 0.0 };
        let label = |tid: &str| of_id(tid).map(|x| x.label.clone()).unwrap_or_else(|| tid.to_string());
        let top_label = top.as_ref().map(|(tid, _)| label(tid));
        let top_pct = top.as_ref().map(|(_, n)| pct(*n, tagged)).unwrap_or(0.0);
        // The topic the catch-all rule judges: the biggest level-1 topic, or its biggest subtopic.
        let (broad, broad_pct) = match &sub_top {
            Some((_, n)) if direct > *n => (top_label.clone().map(|l| format!("{l} (in none of its subtopics)")), pct(direct, tagged)),
            Some((tid, n)) => (Some(label(tid)), pct(*n, tagged)),
            None => (top_label.clone(), top_pct),
        };
        // The topic to split for it (#31): that subtopic, else the level-1 topic.
        let broad_topic = match &sub_top {
            Some((tid, n)) if direct <= *n => of_id(tid),
            _ => top.as_ref().and_then(|(tid, _)| of_id(tid)),
        }
        .map(|x| json!({"layer": x.layer, "label": x.label}));
        // A layer built from a few calls isn't judged by how its sections fit (unknown, 0: judged).
        let fit = l.built_from == 0 || l.built_from >= DRIFT_MIN_CALLS;
        let mut reasons: Vec<Value> = vec![];
        if fit && judged >= DRIFT_MIN && pct(untagged, judged) > 10.0 {
            reasons.push(json!({"rule": "nothing_fits", "text": format!("{}% of its new sections fit no topic: its topics miss what the calls talk about now", pct(untagged, judged).round())}));
        }
        if fit && judged >= DRIFT_MIN && tagged > 0 && broad_pct > 40.0 {
            reasons.push(json!({"rule": "catch_all", "topic": broad_topic, "text": format!("one topic, “{}”, took {}% of its new sections: too broad to tell them apart", broad.unwrap_or_default(), broad_pct.round())}));
        }
        if l.built_from > 0 && calls >= 6 && calls >= 3 * l.built_from as i64 {
            reasons.push(json!({"rule": "outgrown", "text": format!("its topics were built from {} call{}, but it has {calls} now", l.built_from, if l.built_from == 1 { "" } else { "s" })}));
        }
        let z = &snoozed[id];
        let held = z.is_object() && (judged as f64) < 1.5 * z["sections"].as_f64().unwrap_or(0.0) && (calls as f64) < 1.5 * z["calls"].as_f64().unwrap_or(0.0).max(1.0);
        out.push(json!({"layer": id, "name": if l.name.is_empty() { id.clone() } else { l.name.clone() }, "built_version": since, "built_from": l.built_from,
                        "sections": judged, "untagged": untagged, "untagged_pct": pct(untagged, judged), "tagged": tagged,
                        "top": top_label.map(|x| json!({"label": x, "pct": top_pct})), "calls": calls,
                        "stale": !reasons.is_empty() && !held, "snoozed": held && !reasons.is_empty(), "reasons": reasons}));
    }
    Ok(out)
}

/// Every layer's drift (`echoctl brain taxonomy drift`, Needs you, the Topics tab).
pub fn drift() -> Result<Vec<Value>> {
    let Some(t) = current() else { return Ok(vec![]) };
    let snoozed: Value = std::fs::read(drift_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
    Ok(drift_kept(&t, &snoozed)?.as_ref().clone())
}

/// "Not now": the layer isn't called stale again until its new sections or its calls grow by half.
pub fn snooze(layer: &str) -> Result<()> {
    let d = drift()?.into_iter().find(|x| x["layer"] == layer).ok_or_else(|| anyhow!("no layer {layer:?}"))?;
    let mut all: Value = std::fs::read(drift_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
    all[layer] = json!({"sections": d["sections"], "calls": d["calls"], "at": chrono::Utc::now().to_rfc3339()});
    std::fs::create_dir_all(dir())?;
    crate::paths::write_atomic(&drift_file(), &serde_json::to_vec_pretty(&all)?)?;
    Ok(())
}

// ── moves: a topic up to the practice, or down to one client (no model; the user decides) ──────

/// A practice topic needs at least this many tagged sections before "only one client uses it" counts.
pub(super) const DEMOTE_MIN: i64 = 10;

pub(super) fn moves_file() -> PathBuf {
    dir().join("moves.json")
}

/// Topics that look in the wrong place, as suggestions: **up** — clients' topics that name the same
/// thing (a label or synonym in common, or nearly the same label) belong to the practice; **down** — a
/// practice topic whose tagged sections (≥ 10) all come from one client belongs to it. Turned down
/// ones (`dismiss_move`) stay quiet.
pub(super) fn moves_in(st: &Stats, t: &Taxonomy, dismissed: &BTreeSet<String>) -> Vec<Value> {
    let mut out = vec![];
    let clients: Vec<&Topic> = t.topics.iter().filter(|x| x.layer != SHARED).collect();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    // A client's topic that the practice already has (a build from before #33 made them) is one too: up = into it.
    let all_topics: Vec<&Topic> = t.topics.iter().collect();
    for a in &clients {
        let same: Vec<&&Topic> = all_topics.iter().filter(|b| b.layer != a.layer && (names(b, &a.label) || a.synonyms.iter().any(|s| names(b, s)) || ratio(&a.label, &b.label) >= SAME)).collect();
        if same.is_empty() || seen.contains(&topic_id(&a.layer, &a.label)) {
            continue;
        }
        let all: Vec<&Topic> = std::iter::once(*a).chain(same.iter().map(|b| **b)).collect();
        for x in &all {
            seen.insert(topic_id(&x.layer, &x.label));
        }
        let key = format!("up:{}", topic_id(&a.layer, &a.label));
        if dismissed.contains(&key) {
            continue;
        }
        let name = |l: &str| t.layers.get(l).map(|x| x.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| l.to_string());
        out.push(json!({"key": key, "dir": "up", "label": a.label, "layer": a.layer,
                        "topics": all.iter().map(|x| json!({"label": x.label, "layer": x.layer, "layer_name": name(&x.layer)})).collect::<Vec<_>>(),
                        "why": if all.iter().any(|x| x.layer == SHARED) { format!("your practice has it already: {}", all.iter().map(|x| format!("{} ({})", x.label, if x.layer == SHARED { "practice".to_string() } else { name(&x.layer) })).collect::<Vec<_>>().join(", ")) }
                               else { format!("{} clients have it: {}", all.len(), all.iter().map(|x| format!("{} ({})", x.label, name(&x.layer))).collect::<Vec<_>>().join(", ")) }}));
    }
    // Terms: one name in several clients (or a client's and the practice's) is one thing of the practice.
    let mut tseen: BTreeSet<String> = BTreeSet::new();
    let lname = |l: &str| t.layers.get(l).map(|x| x.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| if l == SHARED { "your practice".into() } else { l.to_string() });
    // Terms that share a name by any of their names, found through one index (tens of thousands of terms: never each
    // against each).
    let mut by_name: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
    for (i, x) in t.terms.iter().enumerate() {
        for n in x.names() {
            let e = by_name.entry(nid(n)).or_default();
            if e.last() != Some(&i) {
                e.push(i);
            }
        }
    }
    for a in t.terms.iter().filter(|x| x.layer != SHARED) {
        if tseen.contains(&term_id(&a.layer, &a.name)) {
            continue;
        }
        let mut near: Vec<usize> = a.names().flat_map(|n| by_name.get(&nid(n)).cloned().unwrap_or_default()).collect();
        near.sort_unstable();
        near.dedup();
        let same: Vec<&Term> = near.into_iter().map(|i| &t.terms[i]).filter(|b| b.layer != a.layer && (b.is(&a.name) || a.is(&b.name))).collect();
        if same.is_empty() {
            continue;
        }
        let all: Vec<&Term> = std::iter::once(a).chain(same).collect();
        for x in &all {
            tseen.insert(term_id(&x.layer, &x.name));
        }
        let key = format!("up:{}", term_id(&a.layer, &a.name));
        if dismissed.contains(&key) {
            continue;
        }
        out.push(json!({"key": key, "dir": "up", "what": "term", "label": a.name, "layer": a.layer,
                        "topics": all.iter().map(|x| json!({"label": x.name, "layer": x.layer, "layer_name": lname(&x.layer)})).collect::<Vec<_>>(),
                        "why": format!("{} has it: {}", all.iter().map(|x| lname(&x.layer)).collect::<Vec<_>>().join(", "), all.iter().map(|x| x.name.clone()).collect::<Vec<_>>().join(" · "))}));
    }
    for x in t.topics.iter().filter(|x| x.layer == SHARED) {
        let id = topic_id(&x.layer, &x.label);
        let key = format!("down:{id}");
        if dismissed.contains(&key) {
            continue;
        }
        let by: Vec<(String, i64)> = st.clients.get(&id).map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect()).unwrap_or_default();
        let total: i64 = by.iter().map(|(_, n)| n).sum();
        if let ([(client, n)], true) = (by.as_slice(), total >= DEMOTE_MIN) {
            if client != "-" {
                let name = t.layers.get(client).map(|l| l.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| client.clone());
                out.push(json!({"key": key, "dir": "down", "label": x.label, "layer": SHARED, "to": client, "to_name": name,
                                "why": format!("all its {n} sections are {name}'s")}));
            }
        }
    }
    out
}

/// The suggested moves now (`echoctl brain taxonomy moves`, the Inbox, the Topics tab).
pub fn moves() -> Result<Vec<Value>> {
    let Some(t) = current() else { return Ok(vec![]) };
    Ok(moves_in(&*stats(None)?, &t, &dismissed_moves()))
}

pub(super) fn dismissed_moves() -> BTreeSet<String> {
    std::fs::read(moves_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// "Keep it where it is": that suggestion isn't made again.
pub fn dismiss_move(key: &str) -> Result<()> {
    let mut d = dismissed_moves();
    d.insert(key.to_string());
    std::fs::create_dir_all(dir())?;
    crate::paths::write_atomic(&moves_file(), &serde_json::to_vec_pretty(&d)?)?;
    Ok(())
}

/// Do a suggested move: up — the first client's topic goes to the practice and the others merge into it;
/// down — the practice's topic goes to that client. Says where it went (a term lands in the dictionary,
/// under its group — not in the Topics tree, which is where people looked for it, #32).
pub fn apply_move(key: &str) -> Result<String> {
    let m = moves()?.into_iter().find(|m| m["key"] == key).ok_or_else(|| anyhow!("that suggestion is gone"))?;
    if m["dir"] == "down" {
        edit(&json!({"op": "scope", "label": m["label"], "layer": SHARED, "to": m["to"]}))?;
        return Ok(format!("“{}” is now {}'s topic", m["label"].as_str().unwrap_or(""), m["to_name"].as_str().unwrap_or("the client")));
    }
    let all = m["topics"].as_array().cloned().unwrap_or_default();
    let term = m["what"] == "term";
    let (k, first) = (if term { "name" } else { "label" }, all.iter().find(|x| x["layer"] == SHARED).unwrap_or(&all[0]));
    if first["layer"] != SHARED {
        edit(&json!({"op": "scope", k: first["label"], "layer": first["layer"], "to": SHARED, "term": term}))?;
    }
    // The client copies merge into it — never another practice topic that happened to match too.
    for x in all.iter().filter(|x| *x != first && x["layer"] != SHARED) {
        edit(&json!({"op": "merge", k: x["label"], "layer": x["layer"], "into": first["label"], "into_layer": SHARED, "term": term}))?;
    }
    let name = first["label"].as_str().unwrap_or("");
    Ok(if term {
        let group = current().and_then(|t| t.terms.iter().find(|x| x.layer == SHARED && x.is(name)).map(|x| x.group().to_string())).unwrap_or_default();
        let shown = GROUPS.iter().find(|(k, _)| *k == group).map(|(_, l)| *l).unwrap_or("Other");
        format!("“{name}” is now in your practice's dictionary · {shown} (Brain → Terms)")
    } else {
        format!("“{name}” is now one of your practice's topics (Brain → Topics)")
    })
}
