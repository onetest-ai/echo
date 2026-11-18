//! The plugin's store kept tidy — moved out of `jshost.rs` as it was.

use super::*;

/// When a host half starts: what lay in its store's `cache/` over 7 days, and downloads left half-written (`.part`,
/// over an hour: a fetch takes at most 60 s), go. The rest of the store is the plugin's to clean.
pub(super) fn sweep_store(id: &str) {
    fn walk(dir: &Path, f: &mut dyn FnMut(&Path, std::time::Duration)) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let Ok(t) = e.file_type() else { continue };
            if t.is_dir() {
                walk(&e.path(), f);
            } else if t.is_file() {
                let age = e.metadata().and_then(|m| m.modified()).ok().and_then(|m| m.elapsed().ok()).unwrap_or_default();
                f(&e.path(), age);
            }
        }
    }
    const DAY: u64 = 86_400;
    let base = plugins::data_dir(id);
    let cache = base.join("cache");
    walk(&base, &mut |p, age| {
        // Only Echo's own downloads: `<name>.<pid>-<n>.part`.
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let ours = name.strip_suffix(".part").and_then(|r| r.rsplit_once('.')).and_then(|(_, t)| t.split_once('-')).is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && (a.to_owned() + b).bytes().all(|c| c.is_ascii_digit()));
        let stale_part = ours && age.as_secs() > 3600;
        if stale_part || (p.starts_with(&cache) && age.as_secs() > 7 * DAY) {
            let _ = std::fs::remove_file(p);
        }
    });
    // The folders of `cache/` left empty (a snapshot swept), deepest first; `cache/` itself stays.
    fn prune(dir: &Path) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                prune(&e.path());
                let _ = std::fs::remove_dir(e.path());
            }
        }
    }
    prune(&cache);
}
