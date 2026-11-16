//! Where a mail draft goes (#135): `echoctl compose` — the one way the user's sessions and the runs Echo starts write a
//! mail for the user — puts it in Microsoft 365 Drafts, opens it in the default mail app (`mailto:`) or in an app the
//! user chose (Settings → General → Where drafts go). Nothing is ever sent from here: the user sends it.
//!
//! In a run Echo started (`ECHOCTL_RUN_DRAFTS` set), each draft is noted in the run's folder, so the run's outcome says
//! where it is and links to it.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::config::Config;

/// Set by Echo for a run it starts (`claudejob`): the file each draft of the run is noted in (one JSON line each).
pub const RUN_DRAFTS: &str = "ECHOCTL_RUN_DRAFTS";

/// Where drafts go.
#[derive(Debug, Clone, PartialEq)]
pub enum Where {
    /// Microsoft 365 Drafts (Outlook), on Echo's sign-in.
    M365,
    /// Whatever handles `mailto:` on this computer.
    MailApp,
    /// An app the user chose: its name (macOS, `open -a`) or its program (Windows, run with the `mailto:` link).
    App(String),
}

/// The setting as stored (`config.drafts`): `m365`, `mail-app` or `app:<name or program>`, checked. None: the default.
pub fn setting(typed: &str) -> Result<Option<String>> {
    let t = typed.trim();
    Ok(match t {
        "" | "default" => None,
        "m365" | "mail-app" => Some(t.to_string()),
        _ => {
            let app = t.strip_prefix("app:").map(str::trim).filter(|a| !a.is_empty() && !a.chars().any(char::is_control));
            let Some(app) = app else { bail!("where drafts go: m365, mail-app or app:<app> (e.g. app:Microsoft Outlook)") };
            // Windows runs the program itself: it must be one, there.
            #[cfg(windows)]
            if !(app.to_ascii_lowercase().ends_with(".exe") && Path::new(app).is_absolute() && Path::new(app).is_file()) {
                bail!("{app}: give the mail app's program, its full path (…\\outlook.exe)");
            }
            Some(format!("app:{app}"))
        }
    })
}

/// Where drafts go now: the user's choice, else Microsoft 365 when signed in, else the default mail app.
pub fn chosen(cfg: &Config, signed_in: bool) -> Where {
    match cfg.drafts.as_deref() {
        Some("m365") => Where::M365,
        Some("mail-app") => Where::MailApp,
        Some(a) if a.starts_with("app:") => Where::App(a[4..].to_string()),
        _ if signed_in => Where::M365,
        _ => Where::MailApp,
    }
}

/// A `mailto:` link for the draft.
pub fn mailto(to: &[String], cc: &[String], subject: &str, body: &str) -> String {
    let enc = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'@' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    };
    let mut url = format!("mailto:{}?subject={}&body={}", to.iter().map(|t| enc(t)).collect::<Vec<_>>().join(","), enc(subject), enc(body));
    if !cc.is_empty() {
        url.push_str(&format!("&cc={}", cc.iter().map(|t| enc(t)).collect::<Vec<_>>().join(",")));
    }
    url
}

/// `echoctl compose`: the draft where the user wants it (`chosen`); a Microsoft 365 draft that can't be made opens in
/// the default mail app instead, and says why. Never sends. Returns what was done (`{draft, where, url?, note?}`).
pub fn compose(to: &[String], cc: &[String], subject: &str, body: &str, ai: Option<&str>) -> Result<Value> {
    let mut body = body.to_string();
    // A model wrote it: the draft says so, wherever it goes (`deliver::ai_notice`).
    if let Some(m) = ai.map(str::trim).filter(|m| !m.is_empty()) {
        body = format!("{}\n\n{}", body.trim_end(), crate::deliver::ai_notice(m));
    }
    let cfg = Config::load().unwrap_or_default();
    let mut out = match chosen(&cfg, crate::graph::account().is_some()) {
        Where::M365 => match crate::m365::draft_text(to, cc, subject, &body) {
            Ok(v) => json!({"draft": true, "where": "m365", "id": v["id"], "url": v["url"]}),
            Err(e) => {
                open(&mailto(to, cc, subject, &body), None)?;
                json!({"draft": true, "where": "mail-app", "note": format!("not in Microsoft 365 Drafts ({e:#}): opened in the mail app instead")})
            }
        },
        Where::MailApp => {
            open(&mailto(to, cc, subject, &body), None)?;
            json!({"draft": true, "where": "mail-app"})
        }
        Where::App(a) => match open(&mailto(to, cc, subject, &body), Some(&a)) {
            Ok(()) => json!({"draft": true, "where": "app", "app": app_name(&a)}),
            // The app is gone or won't open: the default mail app instead, saying why.
            Err(e) => {
                open(&mailto(to, cc, subject, &body), None)?;
                json!({"draft": true, "where": "mail-app", "note": format!("{} didn't open ({e:#}): opened in the mail app instead", app_name(&a))})
            }
        },
    };
    out["to"] = json!(to);
    out["subject"] = json!(subject);
    note_in_run(&out);
    Ok(out)
}

/// An app's name as the user knows it (`Microsoft Outlook`, `OUTLOOK` of `…\OUTLOOK.EXE`).
fn app_name(a: &str) -> String {
    Path::new(a).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| a.to_string())
}

/// Open a `mailto:` link in the default mail app, or in `app`. A development build with `ECHOCTL_TEST_MAIL_DRAFTS` set
/// (test data) writes it to that file instead: no mail app opens.
fn open(url: &str, app: Option<&str>) -> Result<()> {
    #[cfg(any(debug_assertions, test))]
    if let Some(f) = std::env::var_os("ECHOCTL_TEST_MAIL_DRAFTS") {
        let mut log = fs::OpenOptions::new().create(true).append(true).open(&f)?;
        writeln!(log, "{}{url}", app.map(|a| format!("[{a}] ")).unwrap_or_default())?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let mut c = std::process::Command::new("open");
        if let Some(a) = app {
            c.args(["-a", a]);
        }
        anyhow::ensure!(c.arg(url).status()?.success(), "`open` couldn't open it");
    }
    #[cfg(windows)]
    match app {
        Some(a) => drop(std::process::Command::new(a).arg(url).spawn()?),
        None => drop(std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).status()?),
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = (url, app);
    Ok(())
}

/// In a run Echo started: the draft noted for the run's outcome.
fn note_in_run(v: &Value) {
    if let Some(f) = std::env::var_os(RUN_DRAFTS) {
        if let Ok(mut log) = fs::OpenOptions::new().create(true).append(true).open(&f) {
            let _ = writeln!(log, "{v}");
        }
    }
}

/// The drafts a run made, for its outcome (`today::yolo_task`): those it noted in `file`, at most 10. The run can write
/// that file too, so nothing in it is taken on its word: a Microsoft 365 draft is linked only as Microsoft 365 gives it
/// (`draft_link`: there, by its id, still a draft); every other line is text the page escapes, with no link.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn of_run(file: &Path, draft_link: impl Fn(&str) -> Option<String>) -> Vec<Value> {
    let lines: Vec<Value> = fs::read_to_string(file).map(|t| t.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).take(10).collect()).unwrap_or_default();
    lines
        .into_iter()
        .map(|d| {
            let s = |k: &str| d[k].as_str().map(|v| v.chars().filter(|c| !c.is_control()).take(200).collect::<String>());
            let to: Vec<String> = d["to"].as_array().into_iter().flatten().filter_map(|t| t.as_str()).take(10).map(|t| t.chars().take(200).collect()).collect();
            let url = (d["where"] == "m365").then(|| s("id")).flatten().and_then(|id| draft_link(&id));
            json!({"where": s("where"), "app": s("app"), "subject": s("subject"), "to": to, "note": s("note"), "url": url})
        })
        .collect()
}

/// A Microsoft 365 draft's link, as Microsoft 365 gives it: the message by its id is there and still a draft.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn m365_draft_link(id: &str) -> Option<String> {
    let m = crate::graph::request("GET", &format!("/me/messages/{}?$select=isDraft,webLink", crate::m365::enc(id)), crate::graph::Body::None, None, &[]).ok()?;
    (m["isDraft"] == true).then(|| m["webLink"].as_str().filter(|u| u.starts_with("https://")).map(str::to_owned)).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Environment variables set for a test, put back when it ends — a failed assertion too.
    struct Vars(Vec<&'static str>);
    impl Vars {
        fn set(vars: &[(&'static str, &Path)]) -> Vars {
            for (k, v) in vars {
                std::env::set_var(k, v);
            }
            Vars(vars.iter().map(|(k, _)| *k).collect())
        }
    }
    impl Drop for Vars {
        fn drop(&mut self) {
            for k in &self.0 {
                std::env::remove_var(k);
            }
        }
    }

    #[test]
    fn drafts_go_where_the_user_said_else_to_microsoft_365_when_signed_in() {
        let cfg = |d: Option<&str>| Config { drafts: d.map(str::to_owned), ..Default::default() };
        assert_eq!(chosen(&cfg(None), true), Where::M365, "signed in: Microsoft 365 Drafts");
        assert_eq!(chosen(&cfg(None), false), Where::MailApp, "not signed in: the default mail app");
        assert_eq!(chosen(&cfg(Some("mail-app")), true), Where::MailApp);
        assert_eq!(chosen(&cfg(Some("m365")), false), Where::M365, "the user's choice (compose falls back if it can't)");
        assert_eq!(chosen(&cfg(Some("app:Spark")), true), Where::App("Spark".into()));
        assert_eq!(setting(" m365 ").unwrap().as_deref(), Some("m365"));
        assert_eq!(setting("default").unwrap(), None);
        assert!(setting("pigeon").is_err() && setting("app:").is_err() && setting("app:Mail\u{7}").is_err());
        #[cfg(target_os = "macos")]
        assert_eq!(setting("app: Microsoft Outlook").unwrap().as_deref(), Some("app:Microsoft Outlook"));
        #[cfg(windows)]
        {
            assert!(setting("app:Outlook").is_err(), "Windows: the program's full path");
            assert!(setting(r"app:C:\nowhere\outlook.exe").is_err(), "a program that is there");
            let exe = std::env::current_exe().unwrap();
            assert_eq!(setting(&format!("app:{}", exe.display())).unwrap(), Some(format!("app:{}", exe.display())));
        }
    }

    #[test]
    fn a_draft_is_a_mailto_link_noted_for_the_run_and_never_sent() {
        let (_env, root) = crate::paths::testenv::folders("drafts-compose");
        let url = mailto(&["kim@acme.example".into()], &["lee@acme.example".into()], "Training slots?", "Hi Kim,\nwhich slots work?");
        assert_eq!(url, "mailto:kim@acme.example?subject=Training%20slots%3F&body=Hi%20Kim%2C%0Awhich%20slots%20work%3F&cc=lee@acme.example");
        // The default mail app, as test data catches it; the run's outcome learns where the draft is.
        Config { drafts: Some("mail-app".into()), ..Default::default() }.save().unwrap();
        let (log, run) = (root.join("mail-drafts.log"), root.join("drafts.jsonl"));
        let body = "Hi Kim,\nwhich slots work?";
        let _vars = Vars::set(&[("ECHOCTL_TEST_MAIL_DRAFTS", &log), (RUN_DRAFTS, &run)]);
        let v = compose(&["kim@acme.example".into()], &[], "Training slots?", body, Some("sonnet")).unwrap();
        assert!(fs::read_to_string(&log).unwrap().contains("Written%20by%20AI%20%28Claude%2C%20sonnet%29"), "a model's draft says so");
        assert_eq!((v["where"].as_str(), v["draft"].as_bool()), (Some("mail-app"), Some(true)));
        assert!(fs::read_to_string(&log).unwrap().starts_with("mailto:kim@acme.example?subject=Training%20slots%3F"));
        // A chosen app gets the same link.
        Config { drafts: Some("app:Spark".into()), ..Default::default() }.save().unwrap();
        compose(&["kim@acme.example".into()], &[], "Again", body, None).unwrap();
        assert!(fs::read_to_string(&log).unwrap().lines().nth(1).unwrap().starts_with("[Spark] mailto:"));
        // Microsoft 365 chosen, not signed in: the mail app instead, and the outcome says why.
        Config { drafts: Some("m365".into()), ..Default::default() }.save().unwrap();
        let v = compose(&["kim@acme.example".into()], &[], "Third", body, None).unwrap();
        assert_eq!(v["where"], "mail-app");
        assert!(v["note"].as_str().unwrap().contains("not in Microsoft 365 Drafts"), "{v}");
        let noted = of_run(&run, |_| None);
        assert_eq!(noted.iter().map(|d| d["subject"].as_str().unwrap()).collect::<Vec<_>>(), ["Training slots?", "Again", "Third"]);
    }

    #[test]
    fn a_link_the_run_wrote_itself_is_never_shown() {
        let (_env, root) = crate::paths::testenv::folders("drafts-forged");
        let run = root.join("drafts.jsonl");
        // The run can write its own drafts file: a forged line with a link of its own, and a real draft's id.
        fs::write(&run, [
            json!({"draft": true, "where": "m365", "url": "https://evil.example/login", "subject": "Re: invoice"}).to_string(),
            json!({"draft": true, "where": "m365", "id": "AAMk-real", "url": "https://evil.example/2", "subject": "Slots", "to": ["kim@acme.example"]}).to_string(),
            json!({"draft": true, "where": "m365", "id": "AAMk-gone", "url": "https://evil.example/3", "subject": "Gone"}).to_string(),
        ].join("\n")).unwrap();
        let graph = |id: &str| (id == "AAMk-real").then(|| "https://outlook.office365.com/owa/?ItemID=AAMk-real".to_string());
        let d = of_run(&run, graph);
        assert_eq!(d[0]["url"], Value::Null, "no id: no link");
        assert_eq!(d[1]["url"], "https://outlook.office365.com/owa/?ItemID=AAMk-real", "the link Microsoft 365 gives, not the file's");
        assert_eq!(d[2]["url"], Value::Null, "not a draft there (sent, deleted, made up): no link");
    }
}
