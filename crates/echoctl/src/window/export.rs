//! Export as HTML: a dashboard's file (Today, a thread's or a client's, built by the page: 48-export.js) saved into the
//! user's Downloads under a name of its own, then shown there in Finder / Explorer.

use super::*;
use anyhow::Context;

/// The largest file the page may send.
const MAX_BYTES: usize = 20 << 20;

/// The page's actions of this area.
pub(super) fn handle(v: &Value) -> Option<Handled> {
    if v["cmd"] != "export_save" {
        return None;
    }
    let saved = save(&crate::paths::downloads_dir(), v["title"].as_str().unwrap_or(""), v["html"].as_str().unwrap_or(""), chrono::Local::now().naive_local());
    Some(match saved {
        Ok(path) => {
            // Test data: logged, nothing opened.
            if !crate::app::open_log_only(&format!("reveal {}", path.display())) {
                crate::app::reveal_file(&path);
            }
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            Handled { refresh: false, reply: Some(format!("window.echo && window.echo.notice({})", json!(format!("Saved to Downloads: {name}")))) }
        }
        Err(e) => Handled { refresh: false, reply: Some(format!("window.echo && window.echo.notice({})", json!(format!("Couldn't save the export: {e:#}")))) },
    })
}

/// "Echo - <title> - 2026-11-14 1430": the title kept to letters, digits, spaces and a few marks any file system takes
/// (80 characters at most; nothing left: "Dashboard").
fn file_stem(title: &str, at: chrono::NaiveDateTime) -> String {
    let kept: String = title.chars().map(|c| if c.is_alphanumeric() || " -_,()&+'".contains(c) { c } else { ' ' }).collect();
    let words = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    let short: String = words.chars().take(80).collect();
    let short = short.trim();
    format!("Echo - {} - {}", if short.is_empty() { "Dashboard" } else { short }, at.format("%Y-%m-%d %H%M"))
}

/// Write `html` into `dir` as `<stem>.html`, `<stem> (2).html`, … the first name not taken; the path written.
fn save(dir: &Path, title: &str, html: &str, at: chrono::NaiveDateTime) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(!html.trim().is_empty(), "the page sent nothing to save");
    anyhow::ensure!(html.len() <= MAX_BYTES, "it is over {} MB", MAX_BYTES >> 20);
    // macOS asks the user whether Echo may use Downloads; a no says where to allow it, not a raw error.
    let denied = |e: &std::io::Error| e.kind() == std::io::ErrorKind::PermissionDenied;
    let allow = "Echo isn't allowed to use Downloads — allow it in System Settings → Privacy & Security → Files and Folders";
    if let Err(e) = fs::create_dir_all(dir) {
        if denied(&e) {
            anyhow::bail!("{allow}");
        }
        return Err(e).with_context(|| format!("creating {}", dir.display()));
    }
    let stem = file_stem(title, at);
    let path = (1..1000)
        .map(|n| dir.join(if n == 1 { format!("{stem}.html") } else { format!("{stem} ({n}).html") }))
        .find(|p| !p.exists())
        .context("every name for it is taken")?;
    if let Err(e) = crate::paths::write_atomic(&path, html.as_bytes()) {
        if denied(&e) {
            anyhow::bail!("{allow}");
        }
        return Err(e).with_context(|| format!("writing {}", path.display()));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 11, 14).unwrap().and_hms_opt(14, 30, 5).unwrap()
    }

    #[test]
    fn an_exports_name_is_echo_the_page_and_the_minute_with_only_safe_characters() {
        assert_eq!(file_stem("Acme · Overview", at()), "Echo - Acme Overview - 2026-11-14 1430");
        assert_eq!(file_stem("Today", at()), "Echo - Today - 2026-11-14 1430");
        assert_eq!(file_stem("../a/b\\c:d*e?\"f<g>|h\n", at()), "Echo - a b c d e f g h - 2026-11-14 1430");
        assert_eq!(file_stem("Ünïcode Клиент · Brief", at()), "Echo - Ünïcode Клиент Brief - 2026-11-14 1430");
        assert_eq!(file_stem("··· / ···", at()), "Echo - Dashboard - 2026-11-14 1430");
        assert_eq!(file_stem(&"x".repeat(300), at()).len(), "Echo - ".len() + 80 + " - 2026-11-14 1430".len());
    }

    #[test]
    fn an_export_is_saved_into_the_folder_given_never_over_another_and_refused_empty_or_too_big() {
        let (_env, root) = crate::paths::testenv::folders("export-save");
        let dir = root.join("Downloads");
        let first = save(&dir, "Acme · Overview", "<!doctype html><p>one", at()).unwrap();
        assert_eq!(first, dir.join("Echo - Acme Overview - 2026-11-14 1430.html"));
        assert_eq!(fs::read_to_string(&first).unwrap(), "<!doctype html><p>one");
        let second = save(&dir, "Acme · Overview", "<!doctype html><p>two", at()).unwrap();
        assert_eq!(second, dir.join("Echo - Acme Overview - 2026-11-14 1430 (2).html"));
        assert_eq!(fs::read_to_string(&first).unwrap(), "<!doctype html><p>one", "the first one kept");
        assert!(save(&dir, "Today", "  ", at()).is_err());
        assert!(save(&dir, "Today", &"x".repeat(MAX_BYTES + 1), at()).is_err());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2, "nothing else written");
    }
}
