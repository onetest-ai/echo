//! What the window shows of the dictionary: one term with its latest sections (Settings → Vocabulary's panel). No
//! model: all from the store.

use anyhow::Result;
use rusqlite::params;
use serde_json::{json, Value};

use super::store::{self, Scope};

/// One topic or term (by node id): the topic or term itself as the taxonomy now has it (`item`; the panel reads it
/// from here — the dictionary comes a page at a time), and its latest sections (within `scope`), with their document
/// and link.
pub fn node(id: &str, scope: Scope) -> Result<Value> {
    let c = store::open(&super::store_path())?;
    let view = super::browse::view(scope)?;
    let item = ["topics", "terms"].iter().flat_map(|k| view[*k].as_array().cloned().unwrap_or_default()).find(|x| x["id"] == id).unwrap_or(Value::Null);
    let rows: Vec<Value> = c
        .prepare(
            "SELECT ch.id, ch.source, ch.title, ch.text, ch.link, ch.speaker, d.title, d.event_date FROM chunk_topics k JOIN chunks ch ON ch.id = k.chunk_id
             LEFT JOIN documents d ON d.source = ch.source WHERE k.category_id = ?1
             AND (?2 IS NULL OR EXISTS (SELECT 1 FROM scopes s WHERE s.chunk_id = ch.id AND s.kind = ?2 AND s.id = ?3))
             ORDER BY ch.event_date DESC, ch.ord LIMIT 12",
        )?
        .query_map(params![id, scope.map(|s| s.0), scope.map(|s| s.1)], |r| {
            Ok((
                (r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?),
                (r.get::<_, Option<String>>(6)?, r.get::<_, Option<String>>(7)?),
            ))
        })?
        .map(|r| {
            r.map(|((cid, src, title, text, link, speaker), (doc, date))| {
                json!({"chunk_id": cid.to_string(), "source": src, "doc": doc, "date": date, "section": title, "text": text.chars().take(240).collect::<String>(), "link": link, "speaker": speaker})
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(json!({"id": id, "item": item, "sections": rows}))
}
