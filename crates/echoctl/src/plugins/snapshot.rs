//! Settings → Plugins' view of the plugins (moved out of `plugins.rs` as it was).

use super::*;

/// Settings → Plugins: installed, offered, marketplaces, and what runs.
pub fn snapshot() -> Value {
    let r = match Registry::load() {
        Ok(r) => r,
        Err(e) => return json!({"error": format!("{e:#}")}),
    };
    let mut markets = vec![];
    let mut discover = vec![];
    for (m, built_in) in r.all_markets() {
        let c = catalog(&m.name);
        // What it offers (`echoctl plugin marketplace list`, #589): installed true from it, false from another one.
        let offers: Vec<Value> = c.as_ref().map(|c| c.plugins.iter().map(|o| json!({
            "id": o.id, "name": o.manifest.as_ref().map_or(o.id.as_str(), |mf| mf.name.as_str()),
            "version": o.manifest.as_ref().map(|mf| mf.version.clone()), "featured": o.featured,
            "installed": r.installed.get(&o.id).map(|p| p.market == m.name), "error": o.error,
        })).collect()).unwrap_or_default();
        markets.push(json!({
            "name": m.name, "source": m.source, "built_in": built_in, "hidden": m.hidden,
            "fetched_at": c.as_ref().map(|c| c.fetched_at.clone()), "commit": c.as_ref().map(|c| c.commit.clone()),
            "error": c.as_ref().and_then(|c| c.error.clone()), "count": c.as_ref().map(|c| c.plugins.len()).unwrap_or(0),
            "title": c.as_ref().map(|c| c.name.clone()).filter(|n| !n.is_empty()), "plugins": offers,
        }));
        if m.hidden {
            continue;
        }
        for o in c.map(|c| c.plugins).unwrap_or_default() {
            let (name, version, summary, uses, why) = match &o.manifest {
                Some(mf) => (mf.name.clone(), mf.version.clone(), mf.summary.clone(), sheet(mf).unwrap_or_default(), why_not(mf)),
                None => (o.id.clone(), String::new(), String::new(), vec![], o.error.clone()),
            };
            discover.push(json!({
                "id": o.id, "name": name, "version": version, "summary": summary, "market": m.name, "built_in": built_in,
                "categories": o.categories, "featured": o.featured, "featured_at": o.featured_at, "uses": uses, "why_not": why,
                "installed": r.installed.get(&o.id).map(|p| p.market == m.name),
            }));
        }
    }
    let installed: Vec<Value> = r
        .installed
        .values()
        .map(|p| {
            let m = read_manifest(&plugin_dir(p)).ok();
            let m_part = m.as_ref().and_then(|m| m.part_problems.first().cloned()).or_else(|| agent::withheld(&p.id));
            let m_prefix = m.as_ref().and_then(agent::prefix_of);
            let summary = m.as_ref().map(|m| m.summary.clone()).unwrap_or_default();
            let client = m.as_ref().and_then(|m| m.client.clone());
            let caps: Vec<String> = m.map(|m| m.uses.keys().cloned().collect()).unwrap_or_default();
            json!({
                "id": p.id, "name": p.name, "version": p.version, "market": p.market, "enabled": p.enabled, "summary": summary, "caps": caps, "client": client,
                "commit": p.commit, "installed_at": p.installed_at, "uses": p.uses,
                "market_gone": r.market(&p.market).is_none(),
                "update": update_for(p).map(|u| review_json(&u)),
                "settings": settings_view(&p.id).unwrap_or_default(),
                "claude_problem": m_part,
                "prefix": m_prefix,
            })
        })
        .collect();
    json!({
        "installed": installed, "discover": discover, "markets": markets,
        "busy": BUSY.lock().map(|b| b.clone()).unwrap_or_default(),
        "errors": ERRORS.lock().map(|b| b.clone()).unwrap_or_default(),
    })
}
