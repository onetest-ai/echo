//! `echoctl plugin install <folder>` (#589): a plugin folder copied into My plugins, then installed from there.
//! A copy on its way in (`.new-…`) or out (`.old-…`) is a dot-folder, which My plugins doesn't list (`mine_catalog`).

use super::*;

/// Where each folder install's copy in My plugins came from, and what was copied: id → {from, hash}.
const SOURCES: &str = ".sources.json";

/// The plugin in `from` copied into My plugins as `plugins/<its id>` (where it came from, and its files' hash, kept in
/// `.sources.json` there) and My plugins read again: its id, the copy, and whether anything was copied (not when `from`
/// is My plugins' own folder). Installing it is then My plugins' install: the sheet, and a copy changed is its update.
/// Refused without a data folder chosen, for a folder that isn't a plugin or holds the data folder, and when My plugins
/// has a folder of that id that isn't this folder's last copy, as copied (the user's own or edited: never overwritten).
pub fn copy_into_mine(from: &Path) -> Result<(String, PathBuf, bool)> {
    if !paths::home_chosen() {
        bail!("choose Echo's data folder first (Settings → Data): a plugin from a folder goes into My plugins, in it");
    }
    let from = dunce::canonicalize(from).ok().filter(|f| f.is_dir()).ok_or_else(|| anyhow!("{} isn't a folder", from.display()))?;
    let m = read_package(&from).with_context(|| format!("{} isn't a plugin", from.display()))?;
    let mine = mine_dir();
    fs::create_dir_all(&mine)?;
    let mine = dunce::canonicalize(&mine)?;
    if mine.starts_with(&from) || dunce::canonicalize(paths::home()).is_ok_and(|h| h.starts_with(&from)) {
        bail!("{} holds Echo's data folder — give the plugin's own folder", from.display());
    }
    let dest = mine.join(&m.id);
    if dunce::canonicalize(&dest).is_ok_and(|d| d == from) {
        refresh(MINE)?;
        return Ok((m.id, dest, false));
    }
    let file = mine.join(SOURCES);
    let mut marks: BTreeMap<String, Value> = fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let source = from.display().to_string();
    if dest.exists() {
        let mark = marks.get(&m.id);
        let ours = mark.is_some_and(|k| k["from"] == source.as_str() && tree_hash(&dest).is_ok_and(|h| k["hash"] == h.as_str()));
        if !ours {
            bail!("My plugins already has a folder “{}” ({}) that isn't this folder's copy — move it away, or install it by its id", m.id, dest.display());
        }
    }
    check_tree(&from, PLUGIN_LIMIT).with_context(|| from.display().to_string())?;
    let tmp = mine.join(format!(".new-{}-{}", m.id, std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let hash = copy_tree(&from, &tmp).and_then(|()| tree_hash(&tmp));
    let hash = match hash {
        Ok(h) => h,
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp);
            return Err(e);
        }
    };
    // Marked before the swap: a mark for a copy not in place yet is harmless, a copy without its mark is refused next.
    marks.insert(m.id.clone(), json!({"from": source, "hash": hash}));
    paths::write_atomic(&file, serde_json::to_string_pretty(&marks)?.as_bytes())?;
    // The old copy moved aside, the new one in its place, then the old one removed.
    let old = mine.join(format!(".old-{}-{}", m.id, std::process::id()));
    if dest.exists() {
        fs::rename(&dest, &old)?;
    }
    if let Err(e) = fs::rename(&tmp, &dest) {
        let _ = fs::rename(&old, &dest);
        let _ = fs::remove_dir_all(&tmp);
        return Err(e.into());
    }
    let _ = fs::remove_dir_all(&old);
    refresh(MINE)?;
    Ok((m.id, dest, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(dir: &Path, id: &str, version: &str) -> PathBuf {
        let d = dir.join(id);
        fs::create_dir_all(d.join("skills")).unwrap();
        fs::write(d.join(MANIFEST), json!({"id": id, "name": format!("Plugin {id}"), "version": version, "summary": "A test"}).to_string()).unwrap();
        fs::write(d.join("skills/README.md"), "hi").unwrap();
        d
    }

    #[test]
    fn a_plugin_folder_is_copied_into_my_plugins_and_installed_from_there() {
        let (_env, root) = paths::testenv::folders("plugins-from-folder");
        let src = plugin(&root.join("src"), "pulse", "0.1.0");
        let (id, copy, copied) = copy_into_mine(&src).unwrap();
        assert_eq!((id.as_str(), copy.clone(), copied), ("pulse", dunce::canonicalize(mine_dir()).unwrap().join("pulse"), true));
        let rv = review("pulse", Some(MINE)).unwrap();
        assert_eq!(install("pulse", Some(MINE), &rv.uses).unwrap().market, MINE, "installed from My plugins' copy");
        // The same folder again: copied over; its change is the installed plugin's update.
        fs::write(src.join("skills/README.md"), "changed").unwrap();
        copy_into_mine(&src).unwrap();
        assert_eq!(fs::read_to_string(copy.join("skills/README.md")).unwrap(), "changed");
        assert!(update_for(&Registry::load().unwrap().installed["pulse"]).is_some());
        // Another folder with that id: My plugins' folder isn't its copy — refused, nothing changed.
        let other = plugin(&root.join("other"), "pulse", "0.2.0");
        assert!(copy_into_mine(&other).unwrap_err().to_string().contains("isn't this folder's copy"));
        assert!(fs::read_to_string(copy.join(MANIFEST)).unwrap().contains("0.1.0"));
        // The copy edited in My plugins: the next copy from its folder doesn't overwrite the edit.
        fs::write(copy.join("skills/README.md"), "edited here").unwrap();
        assert!(copy_into_mine(&src).is_err());
        assert_eq!(fs::read_to_string(copy.join("skills/README.md")).unwrap(), "edited here");
        // A folder the user made in My plugins is never replaced; given as the folder, nothing is copied.
        let own = plugin(&mine_dir(), "own", "0.1.0");
        assert!(copy_into_mine(&plugin(&root.join("third"), "own", "0.1.0")).is_err());
        assert!(!copy_into_mine(&own).unwrap().2);
        // Not a folder, not a plugin, or one that holds the data folder; a copy on its way in isn't listed.
        assert!(copy_into_mine(&root.join("nowhere")).is_err());
        assert!(copy_into_mine(&root.join("src")).unwrap_err().to_string().contains("isn't a plugin"));
        fs::write(root.join(MANIFEST), fs::read(src.join(MANIFEST)).unwrap()).unwrap();
        assert!(copy_into_mine(&root).unwrap_err().to_string().contains("holds Echo's data folder"));
        plugin(&mine_dir(), ".new-own-1", "0.1.0");
        assert!(!refresh(MINE).unwrap().plugins.iter().any(|o| o.id.starts_with('.')));
        // No data folder chosen: no My plugins to copy into.
        std::env::remove_var("ECHOCTL_HOME");
        assert!(copy_into_mine(&src).unwrap_err().to_string().contains("data folder"));
    }
}
