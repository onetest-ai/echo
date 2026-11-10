//! The taxonomy: the graph, and where its terms are mentioned (split out of `taxonomy.rs`, moved as it was).

use super::*;

/// The clients each section belongs to (`-`: none), from the store's scopes.
pub(super) fn chunk_clients(c: &Connection) -> Result<BTreeMap<i64, BTreeSet<String>>> {
    let mut out: BTreeMap<i64, BTreeSet<String>> = BTreeMap::new();
    let mut st = c.prepare("SELECT chunk_id, id FROM scopes WHERE kind = 'client'")?;
    for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
        let (k, v) = row?;
        out.entry(k).or_default().insert(v);
    }
    Ok(out)
}

/// The layers a section's vocabulary comes from: the shared one and its clients' (that have a layer).
pub(super) fn layers_for(t: &Taxonomy, clients: Option<&BTreeSet<String>>) -> Vec<String> {
    let mut out = vec![SHARED.to_string()];
    for c in clients.into_iter().flatten() {
        if c != "-" && t.layers.contains_key(c) {
            out.push(c.clone());
        }
    }
    out
}

/// The graph tables from a taxonomy: topics (intent_l1 / intent_l2, `subclass_of`), entity kinds and
/// terms; tags of a node renamed or merged move to its new id (aliases); tags of a node gone are dropped.
pub fn build_graph(c: &Connection, t: &Taxonomy, before: Option<&Taxonomy>) -> Result<()> {
    let tx = c.unchecked_transaction()?;
    // An alias whose old id names a node again (it came back) moves nothing.
    let live: BTreeSet<String> = t.topics.iter().map(|x| topic_id(&x.layer, &x.label)).chain(t.terms.iter().map(|x| term_id(&x.layer, &x.name))).collect();
    for (old, new) in t.aliases.iter().filter(|(old, _)| !live.contains(*old)) {
        tx.execute("UPDATE chunk_topics SET category_id = ?2 WHERE category_id = ?1", params![old, new])?;
        tx.execute("UPDATE graph_edges SET target = ?2 WHERE target = ?1 AND rel IN ('about', 'mentions')", params![old, new])?;
        tx.execute("INSERT OR REPLACE INTO graph_aliases(alias_id, node_id) VALUES (?1, ?2)", params![old, new])?;
    }
    tx.execute("DELETE FROM graph_nodes", [])?;
    tx.execute("DELETE FROM graph_edges WHERE rel = 'subclass_of'", [])?;
    let mut ids = BTreeSet::new();
    for tp in &t.topics {
        let id = topic_id(&tp.layer, &tp.label);
        ids.insert(id.clone());
        let parent = parent_of(t, tp).map(|p| topic_id(&p.layer, &p.label));
        tx.execute(
            "INSERT OR REPLACE INTO graph_nodes(id, label, kind, parent, description) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, tp.label, if tp.level == 1 { "intent_l1" } else { "intent_l2" }, parent, tp.description],
        )?;
        if let Some(p) = parent {
            tx.execute("INSERT INTO graph_edges(source, target, rel) VALUES (?1, ?2, 'subclass_of')", params![id, p])?;
        }
    }
    // A tag that followed an alias to a topic of the other level takes that level (drift counts by it).
    tx.execute("UPDATE chunk_topics SET kind = (SELECT n.kind FROM graph_nodes n WHERE n.id = chunk_topics.category_id)
                WHERE kind IN ('intent_l1', 'intent_l2') AND category_id IN (SELECT id FROM graph_nodes WHERE kind IN ('intent_l1', 'intent_l2'))", [])?;
    // Brain's entity kinds as nodes, each term under its kind.
    let kinds: BTreeSet<&str> = t.terms.iter().map(|x| x.kind.as_str()).filter(|k| !k.is_empty()).collect();
    for k in kinds {
        tx.execute("INSERT OR REPLACE INTO graph_nodes(id, label, kind, parent, description) VALUES (?1, ?1, 'entity_kind', NULL, NULL)", params![k])?;
    }
    for tm in &t.terms {
        let id = term_id(&tm.layer, &tm.name);
        ids.insert(id.clone());
        tx.execute("INSERT OR REPLACE INTO graph_nodes(id, label, kind, parent, description) VALUES (?1, ?2, 'term', ?3, ?4)", params![id, tm.name, tm.kind, tm.description])?;
    }
    // A client's topic never tags another client's section (a topic moved, a merge): the isolation holds.
    for tp in t.topics.iter().filter(|x| x.layer != SHARED) {
        tx.execute(
            "DELETE FROM chunk_topics WHERE category_id = ?1 AND NOT EXISTS (SELECT 1 FROM scopes s WHERE s.chunk_id = chunk_topics.chunk_id AND s.kind = 'client' AND s.id = ?2)",
            params![topic_id(&tp.layer, &tp.label), tp.layer],
        )?;
    }
    // Tags of nodes no longer there.
    let gone: Vec<String> = tx.prepare("SELECT DISTINCT category_id FROM chunk_topics")?.query_map([], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?.into_iter().filter(|i| !ids.contains(i)).collect();
    for g in gone {
        tx.execute("DELETE FROM chunk_topics WHERE category_id = ?1", [&g])?;
        tx.execute("DELETE FROM graph_edges WHERE target = ?1 AND rel IN ('about', 'mentions')", [&g])?;
    }
    let sha = taxonomy_sha(t);
    let shared = t.layers.get(SHARED).cloned().unwrap_or_default();
    let goal = if shared.goal.is_empty() { goal() } else { shared.goal };
    for (k, v) in [("taxonomy_version", t.version.to_string()), ("taxonomy_sha256", sha), ("goal", goal), ("audience", "the user (their own calls)".to_string()), ("name", "Echo — the user's calls".to_string())] {
        tx.execute("INSERT INTO meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![k, v])?;
    }
    tx.commit()?;
    // Mentions: only the terms this version changed (a term gone took its tags with it above); everything when
    // there is no version before, or the store never had its mentions read.
    let scanned: bool = c.query_row("SELECT EXISTS (SELECT 1 FROM term_scan) OR NOT EXISTS (SELECT 1 FROM chunks)", [], |r| r.get(0)).unwrap_or(false);
    match before.filter(|_| scanned) {
        Some(b) => mentions_of(c, t, &changed_terms(b, t))?,
        None => mentions(c, t)?,
    }
    // Done: the store's mentions are this version's (else the next sync redoes them, `mentions_after_sync`).
    store::set_meta(c, "mentions_for", &taxonomy_sha(t))?;
    Ok(())
}

pub(super) fn word_in(hay: &str, needle: &str) -> bool {
    word_in_lower(&hay.to_lowercase(), &needle.to_lowercase())
}

/// `word_in` on text and name already in lowercase.
pub(super) fn word_in_lower(h: &str, n: &str) -> bool {
    if n.chars().count() < 2 {
        return false;
    }
    let mut from = 0;
    while let Some(i) = h[from..].find(n) {
        let s = from + i;
        let e = s + n.len();
        let before = h[..s].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
        let after = h[e..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        if before && after {
            return true;
        }
        from = s + n.len().max(1);
    }
    false
}

/// Which sections mention which terms (by name or a variant, as whole words) — a client's terms only in
/// that client's sections, shared ones anywhere: deterministic, no model. Every section read once against an index
/// of the terms' names by their first word (tens of thousands of terms, millions of sections: never each against
/// each). `term_scan` remembers the text each section was read in, so a sync reads only what changed
/// (`mentions_changed`) and an edit only the terms it changed (`mentions_of`).
pub fn mentions(c: &Connection, t: &Taxonomy) -> Result<()> {
    let tx = c.unchecked_transaction()?;
    tx.execute_batch(TERM_SCAN)?;
    tx.execute("DELETE FROM chunk_topics WHERE kind = 'term'", [])?;
    tx.execute("DELETE FROM graph_edges WHERE rel = 'mentions'", [])?;
    tx.execute("DELETE FROM term_scan", [])?;
    tx.commit()?;
    scan_unread(c, t)?;
    Ok(())
}

/// After a sync (`brain::sync`): the sections written since they were last read for terms (every write of a
/// section clears its `term_scan` row). Under the taxonomy's lock, with its current version; when the store's
/// mentions aren't that version's (an edit that stopped half way, a store from before `mentions_for`), all of them
/// again (`build_graph`).
pub fn mentions_after_sync(c: &Connection) -> Result<usize> {
    let _lock = write_lock()?;
    let Some(t) = current() else { return Ok(0) };
    if store::meta(c, "taxonomy_sha256").is_none() || store::meta(c, "mentions_for").as_deref() != Some(taxonomy_sha(&t).as_str()) {
        build_graph(c, &t, None)?;
        return Ok(0);
    }
    mentions_changed(c, &t)
}

/// The sections not read for terms since they were last written (their old mentions dropped).
pub fn mentions_changed(c: &Connection, t: &Taxonomy) -> Result<usize> {
    c.execute_batch(TERM_SCAN)?;
    // Every section read (as many rows as sections, and none of a section gone): nothing to do — a count, not a scan.
    let (chunks, read): (i64, i64) = c.query_row("SELECT (SELECT count(*) FROM chunks), (SELECT count(*) FROM term_scan)", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
    if chunks == read && c.query_row("SELECT NOT EXISTS (SELECT 1 FROM term_scan WHERE chunk_id NOT IN (SELECT id FROM chunks) LIMIT 1)", [], |r| r.get::<_, bool>(0))? {
        return Ok(0);
    }
    // Sections gone: their scan state goes too (their tags went with them).
    c.execute("DELETE FROM term_scan WHERE chunk_id NOT IN (SELECT id FROM chunks)", [])?;
    scan_unread(c, t)
}

/// Read every section without a `term_scan` row (or read in another text), 20,000 a transaction: a first read of
/// millions never holds the store for minutes.
pub(super) fn scan_unread(c: &Connection, t: &Taxonomy) -> Result<usize> {
    let idx = NameIndex::new(t);
    let mut n = 0;
    loop {
        let tx = c.unchecked_transaction()?;
        let k = scan_chunks(&tx, t, &idx, "WHERE NOT EXISTS (SELECT 1 FROM term_scan s WHERE s.chunk_id = c.id AND s.sha = coalesce(c.sha, '')) LIMIT 20000")?;
        tx.commit()?;
        n += k;
        if k < 20000 {
            return Ok(n);
        }
    }
}

/// A taxonomy's identity (its whole content): the graph's `taxonomy_sha256`, the mentions' `mentions_for`.
pub(super) fn taxonomy_sha(t: &Taxonomy) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(serde_json::to_vec(t).unwrap_or_default()).iter().map(|b| format!("{b:02x}")).collect()
}

pub(super) const TERM_SCAN: &str = "CREATE TABLE IF NOT EXISTS term_scan(chunk_id INTEGER PRIMARY KEY, sha TEXT NOT NULL);";

/// Terms by the rarest word of each of their names (the word fewest names share: "10002" over "kiosk", "store"
/// over "ai"), lowercase letters and digits: a section's words find the few terms it may mention, each then checked
/// whole (`word_in`).
pub(super) struct NameIndex {
    by_word: HashMap<String, Vec<(usize, String, Vec<String>)>>,
}

pub(super) fn words(s: &str) -> impl Iterator<Item = String> + '_ {
    s.split(|ch: char| !ch.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase)
}

impl NameIndex {
    fn new(t: &Taxonomy) -> Self {
        let names: Vec<(usize, &String)> = t.terms.iter().enumerate().flat_map(|(i, tm)| tm.names().filter(|n| n.chars().count() >= 2).map(move |n| (i, n))).collect();
        let mut df: HashMap<String, usize> = HashMap::new();
        for (_, n) in &names {
            for w in words(n).collect::<BTreeSet<_>>() {
                *df.entry(w).or_default() += 1;
            }
        }
        let mut by_word: HashMap<String, Vec<(usize, String, Vec<String>)>> = HashMap::new();
        for (i, n) in names {
            let ws: Vec<String> = words(n).collect();
            if let Some(key) = ws.iter().min_by_key(|w| (df.get(*w).copied().unwrap_or(0), std::cmp::Reverse(w.len()))) {
                by_word.entry(key.clone()).or_default().push((i, n.to_lowercase(), ws.clone()));
            }
        }
        NameIndex { by_word }
    }
    /// The terms (by index) this text mentions, whole words.
    fn found(&self, text: &str) -> BTreeSet<usize> {
        let lower = text.to_lowercase();
        let have: std::collections::HashSet<String> = words(&lower).collect();
        let mut out = BTreeSet::new();
        for w in &have {
            for (i, name, ws) in self.by_word.get(w).into_iter().flatten() {
                if !out.contains(i) && ws.iter().all(|x| have.contains(x)) && word_in_lower(&lower, name) {
                    out.insert(*i);
                }
            }
        }
        out
    }
}

/// Read the sections `filter` picks (all: "") for terms, streaming: record their mentions and what was read.
pub(super) fn scan_chunks(tx: &Connection, t: &Taxonomy, idx: &NameIndex, filter: &str) -> Result<usize> {
    let mut st = tx.prepare(&format!(
        "SELECT c.id, coalesce(c.title, '') || ' ' || coalesce(c.text, ''), coalesce(c.sha, ''),
                (SELECT group_concat(s.id, char(31)) FROM scopes s WHERE s.chunk_id = c.id AND s.kind = 'client') FROM chunks c {filter}"
    ))?;
    let mut del = tx.prepare("DELETE FROM chunk_topics WHERE chunk_id = ?1 AND kind = 'term'")?;
    let mut del_e = tx.prepare("DELETE FROM graph_edges WHERE source = ?1 AND rel = 'mentions'")?;
    let mut tag = tx.prepare("INSERT INTO chunk_topics(chunk_id, category_id, category_label, kind) VALUES (?1, ?2, ?3, 'term')")?;
    let mut edge = tx.prepare("INSERT INTO graph_edges(source, target, rel) VALUES (?1, ?2, 'mentions')")?;
    let mut done = tx.prepare("INSERT OR REPLACE INTO term_scan(chunk_id, sha) VALUES (?1, ?2)")?;
    let mut n = 0;
    let mut rows = st.query([])?;
    while let Some(r) = rows.next()? {
        let (cid, text, sha, cl): (i64, String, String, Option<String>) = (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?);
        del.execute([cid])?;
        del_e.execute([format!("chunk:{cid}")])?;
        let clients: BTreeSet<&str> = cl.as_deref().unwrap_or("").split('\u{1f}').filter(|x| !x.is_empty()).collect();
        for i in idx.found(&text) {
            let tm = &t.terms[i];
            if tm.layer != SHARED && !clients.contains(tm.layer.as_str()) {
                continue;
            }
            let id = term_id(&tm.layer, &tm.name);
            tag.execute(params![cid, id, tm.name])?;
            edge.execute(params![format!("chunk:{cid}"), id])?;
        }
        done.execute(params![cid, sha])?;
        n += 1;
    }
    Ok(n)
}

/// After an edit: the mentions of these terms (by id) found again — their candidates from the full-text index,
/// each checked whole as above; the others' stay as they were.
pub(super) fn mentions_of(c: &Connection, t: &Taxonomy, ids: &BTreeSet<String>) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let tx = c.unchecked_transaction()?;
    for id in ids {
        tx.execute("DELETE FROM chunk_topics WHERE category_id = ?1 AND kind = 'term'", [id])?;
        tx.execute("DELETE FROM graph_edges WHERE target = ?1 AND rel = 'mentions'", [id])?;
    }
    let mut tag = tx.prepare("INSERT OR IGNORE INTO chunk_topics(chunk_id, category_id, category_label, kind) VALUES (?1, ?2, ?3, 'term')")?;
    let mut edge = tx.prepare("INSERT INTO graph_edges(source, target, rel) VALUES (?1, ?2, 'mentions')")?;
    for tm in t.terms.iter().filter(|x| ids.contains(&term_id(&x.layer, &x.name))) {
        let id = term_id(&tm.layer, &tm.name);
        let mut found: BTreeSet<i64> = BTreeSet::new();
        for n in tm.names().filter(|n| n.chars().count() >= 2) {
            let ws: Vec<String> = words(n).collect();
            if ws.is_empty() {
                continue;
            }
            let phrase = format!("\"{}\"", ws.join(" ").replace('"', "\"\""));
            let sql = format!(
                "SELECT c.id, coalesce(c.title, '') || ' ' || coalesce(c.text, '') FROM chunks_fts f JOIN chunks c ON c.id = f.rowid WHERE chunks_fts MATCH ?1{}",
                if tm.layer == SHARED { "" } else { " AND EXISTS (SELECT 1 FROM scopes s WHERE s.chunk_id = c.id AND s.kind = 'client' AND s.id = ?2)" }
            );
            let mut st = tx.prepare(&sql)?;
            let mut rows = if tm.layer == SHARED { st.query(params![phrase])? } else { st.query(params![phrase, tm.layer])? };
            while let Some(r) = rows.next()? {
                let (cid, text): (i64, String) = (r.get(0)?, r.get(1)?);
                if !found.contains(&cid) && word_in(&text, n) {
                    found.insert(cid);
                }
            }
        }
        for cid in found {
            tag.execute(params![cid, id, tm.name])?;
            edge.execute(params![format!("chunk:{cid}"), id])?;
        }
    }
    drop((tag, edge));
    tx.commit()?;
    Ok(())
}

/// The terms an edit changed (new, or another name, spelling or layer), by id.
pub(super) fn changed_terms(before: &Taxonomy, after: &Taxonomy) -> BTreeSet<String> {
    let sig = |x: &Term| {
        let mut n: Vec<String> = x.names().map(|n| n.to_lowercase()).collect();
        n.sort();
        n
    };
    let old: HashMap<String, Vec<String>> = before.terms.iter().map(|x| (term_id(&x.layer, &x.name), sig(x))).collect();
    after.terms.iter().filter(|x| old.get(&term_id(&x.layer, &x.name)) != Some(&sig(x))).map(|x| term_id(&x.layer, &x.name)).collect()
}
