//! Today: the day, its preps, Needs you now.

use super::*;
use super::calendar::IMPORTS;
use super::threads::tasks_view;


/// The echo:// requests of this area (`respond` asks each area in turn).
pub(super) fn route(path: &str, req: &Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>> {
    Some(match path {
        // Today's meetings still to come, with their threads and preps (Calls → Coming up today).
        "/api/upcoming" => json_reply(crate::prep::upcoming()),
        p if p.starts_with("/api/prep/") => {
            let id = &p["/api/prep/".len()..];
            json_reply(crate::prep::load(id).unwrap_or_else(|| json!({ "none": true, "writing": crate::prep::writing(id) })))
        }
        // A mail of Needs you now, to read it here (its new part as text, escaped by the page).
        "/api/needs/mail" => {
            let id = query_param(req.uri().query().unwrap_or(""), "id").unwrap_or_default();
            json_reply(crate::graph::message(&id).map(|m| json!({"subject": m["subject"], "from": m["from"], "at": m["at"], "text": m["text"], "url": m["url"]})).unwrap_or_else(|e| json!({ "error": format!("{e:#}") })))
        }
        "/api/today" => {
            let mut v = crate::today::view(&tasks_view());
            // A Teams transcript being imported, or why it wasn't (the calendar's own state, by event).
            let imports = IMPORTS.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default();
            for x in v["day"].as_array_mut().into_iter().flatten() {
                if let Some(st) = x["event"].as_str().and_then(|e| imports.get(e)) {
                    x["import"] = st.clone();
                }
            }
            json_reply(v)
        }
        _ => return None,
    })
}

/// The page's commands of this area (`handle` asks each area in turn).
pub(super) fn handle(v: &Value) -> Option<Handled> {
    Some(match v["cmd"].as_str().unwrap_or("") {
        // Today → Needs you now: I'll do it (a task, in a thread) or Won't do; Refresh reads Teams and mail again.
        "needs_decide" => match crate::needs::decide(v["id"].as_str().unwrap_or(""), v["how"].as_str().unwrap_or(""), v["thread"].as_str(), v["title"].as_str(), None) {
            Ok(_) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        // Won't do, with the user's comment (required): decided, then Claude notes who it's for in the background.
        #[cfg(feature = "js")]
        "needs_wont" => match crate::today::wont_do(v["ask"].as_str().unwrap_or(""), v["id"].as_str().unwrap_or(""), v["note"].as_str().unwrap_or("")) {
            Ok(()) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        // A Won't do line's draft: sent by the user's click (its text as they left it), or dropped.
        "wont_send" => match crate::today::wont_send(v["ask"].as_str().unwrap_or(""), v["text"].as_str().unwrap_or("")) {
            Ok(()) => notice("Sent"),
            Err(e) => notice(&format!("{e:#}")),
        },
        "ask_drop" => match crate::today::ask_drop(v["ask"].as_str().unwrap_or("")) {
            Ok(()) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        // Read and reply (a mail of Needs you now): the user's words, sent by their click.
        "needs_reply" => match crate::m365::mail_reply_text(v["message"].as_str().unwrap_or(""), v["all"] == true, v["text"].as_str().unwrap_or(""), None, true) {
            Ok(_) => {
                let _ = crate::needs::decide(v["id"].as_str().unwrap_or(""), "wont", None, None, None);
                notice("Sent")
            }
            Err(e) => notice(&format!("{e:#}")),
        },
        "needs_refresh" => {
            std::thread::spawn(|| {
                if let Err(e) = crate::needs::refresh() {
                    eprintln!("needs you now: {e:#}");
                }
            });
            false.into()
        }
        // My work's Claude dialog → Yolo: the task, its skill and the user's words, done by Claude on its own.
        #[cfg(feature = "js")]
        "task_yolo" => match crate::today::yolo_task(v["ask"].as_str().unwrap_or(""), v["project"].as_str().unwrap_or(""), v["node"].as_str().unwrap_or(""), v["skill"].as_str().unwrap_or(""), v["text"].as_str().unwrap_or("")) {
            Ok(()) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        // Today: Got it on a card (hidden until its source sends the next).
        "today_seen" => match crate::today::seen(v["key"].as_str().unwrap_or("")) {
            Ok(()) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        _ => return None,
    })
}
