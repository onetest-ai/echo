//! The optional Knowledge client's access to Echo's native vault engine.

use super::*;

pub(super) fn describe(v: &Value) -> Result<String> {
    anyhow::ensure!(v.as_object().is_some_and(|o| o.is_empty()), "“knowledge” is {{}}");
    Ok("Reads and edits your connected Knowledge vaults, saves call minutes by your rules, imports the sources you choose, and runs guarded Ask, Ingest and Health on their pinned AI profiles".into())
}

pub fn authorize(id: &str) -> Result<()> {
    let p = enabled().into_iter().find(|p| p.id == id).ok_or_else(|| anyhow!("Knowledge plugin {id:?} is not enabled"))?;
    anyhow::ensure!(read_manifest(&plugin_dir(&p))?.uses.contains_key("knowledge"), "{id} did not ask for Knowledge access when installed");
    Ok(())
}

pub fn active() -> bool {
    enabled().iter().any(|p| read_manifest(&plugin_dir(p)).is_ok_and(|m| m.uses.contains_key("knowledge")))
}

#[cfg_attr(not(any(feature = "tray", feature = "js")), allow(dead_code))]
pub fn read(id: &str, route: &str) -> Result<Value> {
    authorize(id)?;
    if route != "api/kn" {
        let rest = route.strip_prefix("api/kn/").ok_or_else(|| anyhow!("not an agent-readable Knowledge view"))?;
        let path = crate::knowledge::links::percent_decode(rest.split_once('?').map_or(rest, |(p, _)| p));
        // The native UI's job dispatcher accepts every path beginning with jobs.
        anyhow::ensure!(!path.starts_with("jobs"), "not an agent-readable Knowledge view");
        let view = path.split_once('/').map(|(_, view)| view).unwrap_or_default();
        anyhow::ensure!(matches!(view, "tree" | "page" | "find" | "graph" | "checks" | "health" | "history" | "version")
            || view.strip_prefix("health/").is_some_and(|key| !key.is_empty()), "not an agent-readable Knowledge view");
    }
    read_view(route)
}

/// The trusted human UI also owns source pickers, job polling and terminal handoff.
#[cfg(feature = "tray")]
pub(crate) fn client_read(id: &str, route: &str) -> Result<Value> {
    authorize(id)?;
    read_view(route)
}

fn read_view(route: &str) -> Result<Value> {
    if route == "api/kn" {
        return Ok(crate::knowledge::api::state());
    }
    #[cfg(feature = "tray")]
    if let Some(query) = route.strip_prefix("api/kn-resume?") {
        let vault = query.split('&').find_map(|p| p.strip_prefix("vault=")).map(crate::knowledge::links::percent_decode).unwrap_or_default();
        return Ok(json!({"command": crate::terminal::vault_resume_command(&vault)}));
    }
    let rest = route.strip_prefix("api/kn/").ok_or_else(|| anyhow!("not a Knowledge view"))?;
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let path = crate::knowledge::links::percent_decode(path);
    match crate::knowledge::api::route(&path, query)? {
        crate::knowledge::api::Reply::Json(v) => Ok(v),
        crate::knowledge::api::Reply::Bytes(_, _) => bail!("read a vault image through its native asset URL"),
    }
}

pub fn stop(id: &str) {
    if authorize(id).is_err() {
        return;
    }
    stop_runs();
}

fn stop_runs() {
    crate::knowledge::api::take_drops(false);
    #[cfg(feature = "tray")]
    {
        crate::terminal::close_vaults();
        crate::knowledge::chat::close_all();
        crate::knowledge::ingest::close_all();
        crate::knowledge::lint::close_all();
    }
}

/// The app also observes changes made by another process (the CLI's disable, remove or update).
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn sync_lifecycle() {
    static LAST: Mutex<Option<String>> = Mutex::new(None);
    let current = enabled().iter().filter(|p| read_manifest(&plugin_dir(p)).is_ok_and(|m| m.uses.contains_key("knowledge"))).map(|p| json!([p.id, p.version, p.commit, p.files])).collect::<Vec<_>>();
    let current = serde_json::to_string(&current).unwrap_or_default();
    let changed = {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        last.replace(current.clone()).is_some_and(|old| old != current)
    };
    if changed { stop_runs(); }
}

pub fn minutes_written(dir: &Path) -> Vec<String> {
    if active() { crate::knowledge::export::minutes_written(dir) } else { vec![] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_views_refuse_an_uninstalled_or_undeclared_plugin() {
        let (_env, root) = crate::paths::testenv::folders("knowledge-plugin-grant");
        assert!(read("missing", "api/kn").unwrap_err().to_string().contains("not enabled"));
        let package = root.join("market/demo");
        fs::create_dir_all(&package).unwrap();
        fs::write(root.join("market/echo-marketplace.json"), r#"{"name":"test","plugins":[{"id":"demo","source":"./demo"}]}"#).unwrap();
        fs::write(package.join("echo-plugin.json"), r#"{"id":"demo","name":"Demo","version":"1.0.0"}"#).unwrap();
        market_add(root.join("market").to_str().unwrap(), Some("local")).unwrap();
        install("demo", Some("local"), &[]).unwrap();
        assert!(read("demo", "api/kn").unwrap_err().to_string().contains("did not ask"));
        assert!(!active());
    }

    #[test]
    fn declared_native_access_is_revoked_on_disable_without_removing_vault_state() {
        let (_env, root) = crate::paths::testenv::folders("knowledge-plugin-enabled");
        let package = root.join("market/knowledge");
        fs::create_dir_all(&package).unwrap();
        fs::write(root.join("market/echo-marketplace.json"), r#"{"name":"test","plugins":[{"id":"knowledge","source":"./knowledge"}]}"#).unwrap();
        fs::write(package.join("echo-plugin.json"), r#"{"id":"knowledge","name":"Knowledge","version":"0.1.0","uses":{"knowledge":{}}}"#).unwrap();
        market_add(root.join("market").to_str().unwrap(), Some("local")).unwrap();
        let agreed = sheet(&read_manifest(&package).unwrap()).unwrap();
        install("knowledge", Some("local"), &agreed).unwrap();
        let vault = root.join("vault");
        fs::create_dir_all(&vault).unwrap();
        fs::write(vault.join("index.md"), "# Synthetic notes\n").unwrap();
        crate::knowledge::add(&vault, crate::knowledge::AddOpts::default()).unwrap();
        let call = crate::knowledge::fixture::call("2026-11-16_090001-synthetic", "Synthetic sync", "system", crate::knowledge::fixture::MINUTES);
        assert!(active());
        assert_eq!(read("knowledge", "api/kn").unwrap()["vaults"].as_array().unwrap().len(), 1);
        assert!(read("knowledge", "api/kn/vault/tree").unwrap()["pages"].is_array());
        assert_eq!(read("knowledge", "api/kn/vault/page?path=index.md").unwrap()["text"], "# Synthetic notes\n");
        for route in [
            "api/kn/jobs", "api/kn/jobs/health", "api/kn/jobs-extra/tree", "api/kn/jobs%2Fpage", "api/kn/%6aobs/health", "api/kn/pick", "api/kn/vault/m365?q=synthetic", "api/kn/vault/%6d365?q=synthetic",
            "api/kn/vault%2fm365?q=synthetic", "api/kn/vault/%256d365", "api/kn/vault/calls", "api/kn/vault/preview?call=synthetic",
            "api/kn/vault/ingest", "api/kn/vault/chat/synthetic", "api/kn/vault/skills", "api/kn/vault/settings",
            "api/kn/vault/home", "api/kn-resume?vault=vault", "api/calls?q=synthetic", "api/kn/vault/health/",
        ] {
            assert!(read("knowledge", route).unwrap_err().to_string().contains("not an agent-readable Knowledge view"), "{route}");
        }
        assert!(read("knowledge", "api/settings").is_err());
        set_enabled("knowledge", false).unwrap();
        assert!(!active());
        assert!(read("knowledge", "api/kn").is_err());
        assert!(minutes_written(&call).is_empty());
        assert!(!call.join("knowledge.json").exists());
        assert!(vault.join("index.md").exists());
        set_enabled("knowledge", true).unwrap();
        assert_eq!(read("knowledge", "api/kn").unwrap()["vaults"].as_array().unwrap().len(), 1);
        assert_eq!(minutes_written(&call), vec!["vault"]);
        crate::paths::testenv::eventually("optional minutes export", || call.join("knowledge.json").exists());
        remove("knowledge", false).unwrap();
        assert!(!active());
        assert_eq!(crate::knowledge::list().unwrap().len(), 1);
        assert!(vault.join("index.md").exists());
    }
}
