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
        // A thread's or a client's dashboard (`page=thread:<id>`): its layout, each plugin widget's data as that page asks
        // it (`all=1`, editing: every widget it offers, previewed), and the ids it offers.
        "/api/dashboard" => {
            let q = req.uri().query().unwrap_or("");
            let page = match crate::today_layout::Page::parse(&query_param(q, "page").unwrap_or_default()) {
                Ok(p) if p != crate::today_layout::Page::Today => p,
                _ => return Some(json_reply(json!({"error": "a dashboard is a thread's or a client's page"}))),
            };
            let layout = crate::today_layout::load_on(&page);
            let offered: Vec<String> = crate::widgets::catalog().into_iter().filter(|w| page.offers(w)).map(|w| w.id.to_string()).collect();
            let all = query_param(q, "all").is_some_and(|a| a == "1");
            // My work reads the Tasks view (only when it is drawn here).
            let mine = offered.iter().any(|id| id == "core:tasks.mine") && (all || layout.iter().any(|l| l.widget == "core:tasks.mine"));
            let tasks = if mine { tasks_view() } else { json!({}) };
            let i = crate::widgets::Inputs { tasks: &tasks };
            // Echo's own widgets it offers (those that learn a thread or a client): their data with the page's settings.
            let core: serde_json::Map<String, Value> = offered
                .iter()
                .filter(|id| id.starts_with("core:") && (all || layout.iter().any(|l| &l.widget == *id)))
                .filter_map(|id| crate::widgets::data_on(&page, id, &i).ok().map(|d| (id.clone(), d)))
                .collect();
            let plugins: serde_json::Map<String, Value> = offered
                .iter()
                .filter(|id| id.starts_with("plugin:"))
                .filter_map(|id| {
                    let placed = layout.iter().any(|l| &l.widget == id);
                    let d = if placed { crate::widgets::data_on(&page, id, &i) } else if all { crate::widgets::preview_on(&page, id, &i) } else { return None };
                    d.ok().map(|d| (id.clone(), d))
                })
                .collect();
            json_reply(json!({"page": page.key(), "layout": layout, "offered": offered, "core": core, "plugins": plugins}))
        }
        "/api/today" => {
            let tasks = tasks_view();
            let mut v = crate::today::view(&tasks);
            // Which widgets Today shows, in what order and rows (this computer's `today-layout.json`).
            let layout = crate::today_layout::load();
            // Your day, Next meeting and My work set to a thread or a client (⚙): each its own data, by widget (the
            // head and the others keep the whole day).
            let i = crate::widgets::Inputs { tasks: &tasks };
            let scoped: serde_json::Map<String, Value> = ["core:calendar.agenda", "core:calendar.calendar", "core:calendar.strip", "core:calendar.next", "core:tasks.mine"]
                .into_iter()
                .filter(|id| layout.iter().any(|l| l.widget == *id) && crate::widgets::scope_of(&crate::today_layout::params_of(id)).is_some())
                .filter_map(|id| crate::widgets::data(id, &i).ok().map(|d| (id.to_string(), d)))
                .collect();
            if !scoped.is_empty() {
                v["scoped"] = json!(scoped);
            }
            // `all=1` (Today's edit mode, Add a widget's previews): every widget's data, placed or not.
            let all = query_param(req.uri().query().unwrap_or(""), "all").is_some_and(|a| a == "1");
            let ids: Vec<String> = if all { crate::widgets::catalog().into_iter().map(|w| w.id.to_string()).collect() } else { layout.iter().map(|i| i.widget.clone()).collect() };
            let wants = |id: &str| ids.iter().any(|i| i == id);
            // Next meeting only when it is on Today: its prep and the invite's answers are read for it.
            if wants("core:calendar.next") {
                let day: Vec<Value> = v["day"].as_array().cloned().unwrap_or_default();
                v["next"] = crate::widgets::next_meeting(&day)["next"].clone();
            }
            // Threads at a glance and Done this week only when one is on Today (each thread's work is read for them).
            // Each with its own period (a widget's setting): `glances` by widget.
            let glances: serde_json::Map<String, Value> = ["core:tasks.glance", "core:tasks.done"]
                .into_iter()
                .filter(|id| wants(id))
                .map(|id| (id.to_string(), crate::widgets::glance_for(&crate::today_layout::params_of(id))))
                .collect();
            if !glances.is_empty() {
                v["glances"] = json!(glances);
            }
            // Plugins' widgets on Today: their last data, by id (each asked again in the background when stale).
            let tasks = json!({});
            let plugins: serde_json::Map<String, Value> = ids
                .iter()
                .filter(|id| id.starts_with("plugin:"))
                .filter_map(|id| {
                    let i = crate::widgets::Inputs { tasks: &tasks };
                    // On Today: asked again when stale; only previewed (edit mode): its last answer.
                    let d = if layout.iter().any(|l| &l.widget == id) { crate::widgets::data(id, &i) } else { crate::widgets::preview_data(id, &i) };
                    d.ok().map(|d| (id.clone(), d))
                })
                .collect();
            if !plugins.is_empty() {
                v["plugins"] = json!(plugins);
            }
            // Recorded this week, only when it is on Today.
            if wants("core:calls.week") {
                v["calls_week"] = crate::widgets::calls_for(&crate::today_layout::params_of("core:calls.week"));
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
        // Today → Needs you now: I'll do it (a task, in a thread), Won't do or Ignore; Refresh reads Teams and mail again.
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
    // Today's, or a thread's or client's dashboard (`page`: `thread:<id>`, `client:<id>`).
    let page = l::Page::parse(v["page"].as_str().unwrap_or(""))?;
    match v["op"].as_str().unwrap_or("") {
        "add" => l::add_on(&page, w, v["size"].as_str(), target),
        "move" => l::move_on(&page, w, target),
        "params" => {
            let items = l::set_param_on(&page, w, v["key"].as_str().unwrap_or(""), &match &v["value"] { Value::String(s) => s.clone(), x => x.to_string() })?;
            // A plugin's widget is asked again at once with its new settings.
            #[cfg(feature = "js")]
            if let Some((plugin, widget)) = w.strip_prefix("plugin:").and_then(|r| r.split_once(':')) {
                crate::jshost::widget_ask_on(plugin, widget, &page);
            }
            Ok(items)
        }
        "size" => l::resize_on(&page, w, v["size"].as_str().unwrap_or("")),
        "height" => l::set_height_on(&page, w, v["height"].as_u64().and_then(|h| u8::try_from(h).ok()).unwrap_or(0)),
        "remove" => l::remove_on(&page, w),
        "reset" => l::reset_on(&page),
        op => anyhow::bail!("Today's layout: no change {op:?}"),
    }
}

