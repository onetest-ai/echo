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
            let mut v = crate::prep::load(id).unwrap_or_else(|| json!({ "none": true, "writing": crate::prep::writing(id) }));
            // Who is invited now, with their answers (the prep's own list is from when it was written).
            if let Some(a) = crate::prep::attendees(id) {
                v["attendees"] = a;
            }
            json_reply(v)
        }
        // A mail of Needs you now, to read it here (its new part as text, escaped by the page).
        "/api/needs/mail" => {
            let id = query_param(req.uri().query().unwrap_or(""), "id").unwrap_or_default();
            json_reply(crate::graph::message(&id).map(|m| json!({"subject": m["subject"], "from": m["from"], "at": m["at"], "text": m["text"], "url": m["url"]})).unwrap_or_else(|e| json!({ "error": format!("{e:#}") })))
        }
        // The widgets Today can show (the catalog: Echo's, later the plugins').
        "/api/widgets" => json_reply(json!(crate::widgets::catalog())),
        "/api/today" => {
            let mut v = crate::today::view(&tasks_view());
            // Which widgets Today shows, in what order and rows (this computer's `today-layout.json`).
            let layout = crate::today_layout::load();
            // Next meeting only when it is on Today: its prep and the invite's answers are read for it.
            if layout.iter().any(|i| i.widget == "core:calendar.next") {
                let day: Vec<Value> = v["day"].as_array().cloned().unwrap_or_default();
                v["next"] = crate::widgets::next_meeting(&day)["next"].clone();
            }
            // Threads at a glance and Done this week only when one is on Today (each thread's work is read for them).
            // Each with its own period (a widget's setting): `glances` by widget.
            let glances: serde_json::Map<String, Value> = layout
                .iter()
                .filter(|i| i.widget == "core:tasks.glance" || i.widget == "core:tasks.done")
                .map(|i| (i.widget.clone(), crate::glance::today(crate::today_layout::params_of(&i.widget).get("period").and_then(Value::as_str).unwrap_or("week"))))
                .collect();
            if !glances.is_empty() {
                v["glances"] = json!(glances);
            }
            // Plugins' widgets on Today: their last data, by id (each asked again in the background when stale).
            let tasks = json!({});
            let plugins: serde_json::Map<String, Value> = layout
                .iter()
                .filter(|i| i.widget.starts_with("plugin:"))
                .filter_map(|i| crate::widgets::data(&i.widget, &crate::widgets::Inputs { tasks: &tasks }).ok().map(|d| (i.widget.clone(), d)))
                .collect();
            if !plugins.is_empty() {
                v["plugins"] = json!(plugins);
            }
            // Recorded this week, only when it is on Today.
            if layout.iter().any(|i| i.widget == "core:calls.week") {
                v["calls_week"] = crate::calls_week::today();
            }
            v["layout"] = json!(layout);
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
        // Today's edit mode: add, move, size, remove a widget, or reset — the same changes as `echoctl today`.
        "today_layout" => match today_layout_op(v) {
            Ok(_) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        // Today → Needs you now: I'll do it (a task, in a thread) or Won't do; Refresh reads Teams and mail again.
        "needs_decide" => match crate::needs::decide(v["id"].as_str().unwrap_or(""), v["how"].as_str().unwrap_or(""), v["thread"].as_str(), v["title"].as_str(), None) {
            Ok(_) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        // Claude's dialog on an item (✦): Yolo in the background (the outcome above My work), the item decided.
        #[cfg(feature = "js")]
        "needs_yolo" => match crate::today::yolo_needs(v["ask"].as_str().unwrap_or(""), v["id"].as_str().unwrap_or(""), v["skill"].as_str().unwrap_or(""), v["text"].as_str().unwrap_or("")) {
            Ok(()) => true.into(),
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
        // The day's choices: Accept, Maybe or Decline an invite, sent to its organiser by the user's click.
        "event_respond" => match crate::m365::event_respond(v["event"].as_str().unwrap_or(""), v["how"].as_str().unwrap_or(""), true) {
            Ok(r) => notice(&format!("{} — {}", r["meeting"].as_str().unwrap_or("the meeting"), match r["answer"].as_str() { Some("accepted") => "accepted", Some("declined") => "declined", _ => "maybe" })),
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

/// A change to Today's layout from edit mode (`{cmd: "today_layout", op, widget, size?, target?, side?}`).
fn today_layout_op(v: &Value) -> anyhow::Result<Vec<crate::today_layout::Item>> {
    use crate::today_layout::{self as l, Side};
    let w = v["widget"].as_str().unwrap_or("");
    let side = match v["side"].as_str() {
        Some("before") => Some(Side::Before),
        Some("after") => Some(Side::After),
        Some("above") => Some(Side::Above),
        Some("below") => Some(Side::Below),
        _ => None,
    };
    let target = v["target"].as_str().filter(|t| !t.is_empty()).zip(side);
    match v["op"].as_str().unwrap_or("") {
        "add" => l::add(w, v["size"].as_str(), target),
        "move" => l::move_to(w, target),
        "params" => {
            let items = l::set_param(w, v["key"].as_str().unwrap_or(""), &match &v["value"] { Value::String(s) => s.clone(), x => x.to_string() })?;
            // A plugin's widget is asked again at once with its new settings.
            #[cfg(feature = "js")]
            if let Some((plugin, widget)) = w.strip_prefix("plugin:").and_then(|r| r.split_once(':')) {
                crate::jshost::widget_ask(plugin, widget);
            }
            Ok(items)
        }
        "size" => l::resize(w, v["size"].as_str().unwrap_or("")),
        "height" => l::set_height(w, v["height"].as_u64().and_then(|h| u8::try_from(h).ok()).unwrap_or(0)),
        "remove" => l::remove(w),
        "reset" => l::reset(),
        op => anyhow::bail!("Today's layout: no change {op:?}"),
    }
}

