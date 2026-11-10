//! Widgets: what each part of Echo offers to Today — a catalog, and a provider per widget that returns its data.
//! Today's page and `echoctl widget show` read the same data; plugins will add theirs to the catalog (#315).
//! A provider never calls a model: what needs one is a job whose result a provider reads.

use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::{json, Value};

/// A widget the catalog offers: where it comes from, what it shows, the sizes it takes (S, M, L: 1, 2, 3 columns),
/// how tall it is (`hug` its content, `fill` its column) and whether it is temporal (on Today only while its data
/// isn't `empty`).
#[derive(Serialize, Clone, Copy, Debug)]
pub struct Widget {
    pub id: &'static str,
    pub part: &'static str,
    pub label: &'static str,
    pub about: &'static str,
    pub kind: &'static str,
    pub sizes: &'static [&'static str],
    pub height: &'static str,
    pub temporal: bool,
}

// One line a widget in the catalog below: its fields in the struct's order.
#[allow(clippy::too_many_arguments)]
const fn w(id: &'static str, part: &'static str, label: &'static str, about: &'static str, kind: &'static str, sizes: &'static [&'static str], height: &'static str, temporal: bool) -> Widget {
    Widget { id, part, label, about, kind, sizes, height, temporal }
}

/// Echo's own widgets, for what Today shows now. The rest of the agreed set comes with their phases.
pub const CORE: &[Widget] = &[
    w("core:calendar.agenda", "Calendar", "Your day · agenda", "where you'll be, a reason on each line", "day", &["S", "M"], "fill", false),
    w("core:calendar.calendar", "Calendar", "Your day · calendar", "by the hour, overlaps side by side", "day", &["S", "M"], "fill", false),
    w("core:calendar.strip", "Calendar", "Your day · strip", "the whole day in one bar", "day", &["M", "L"], "hug", false),
    w("core:calendar.next", "Calendar", "Next meeting", "what to walk in knowing, who's coming", "next", &["S", "M", "L"], "hug", true),
    w("core:tasks.mine", "Tasks", "My work · this week", "late first, then by date; whose each one is", "work", &["S", "M"], "fill", false),
    w("core:needs.now", "Needs you", "Needs you now", "the last 24 hours, where you watch", "needs", &["M", "L"], "hug", true),
    w("core:routines.cards", "Routines", "Routine cards", "what your routines delivered to Today, until read", "cards", &["M", "L"], "hug", true),
];

/// The catalog: Echo's widgets (and, later, the installed plugins').
pub fn catalog() -> Vec<Widget> {
    CORE.to_vec()
}

pub fn find(id: &str) -> Result<Widget> {
    CORE.iter().find(|w| w.id == id).copied().ok_or_else(|| anyhow!("no widget {id:?}: `echoctl widget list` names them"))
}

/// What a provider reads beyond its own part: the Tasks view, made by the caller (the window from its calendar, the
/// CLI from the agenda) — so both show the same work.
pub struct Inputs<'a> {
    pub tasks: &'a Value,
}

/// A widget's data. A temporal widget's says `empty` when it has nothing (Today takes no space for it).
pub fn data(id: &str, i: &Inputs) -> Result<Value> {
    let w = find(id)?;
    let mut v = match w.kind {
        "day" => day(),
        "next" => next_meeting(&crate::today::day()),
        "work" => crate::today::work(i.tasks),
        "needs" => crate::needs::view(),
        "cards" => json!({ "cards": crate::today::cards() }),
        k => return Err(anyhow!("widget {id}: no provider for kind {k}")),
    };
    if w.temporal {
        let empty = match w.kind {
            "next" => v["next"].is_null(),
            "needs" => v["items"].as_array().is_none_or(|a| a.is_empty()),
            "cards" => v["cards"].as_array().is_none_or(|a| a.is_empty()),
            _ => false,
        };
        v["empty"] = json!(empty);
    }
    Ok(v)
}

/// Your day (agenda, calendar and strip show the same day): its meetings and calls, and whether the calendar
/// couldn't be read today.
fn day() -> Value {
    json!({ "day": crate::today::day(), "calendar_stale": crate::calendar::unread_today() })
}

/// The next meeting: the first of the day's meetings, by start, not over yet — one under way counts, so a long one
/// already started comes before one about to start (overlaps are the day's choices widget's); a meeting with no end
/// counts until it starts.
fn next(day: &[Value], now: chrono::DateTime<chrono::Utc>) -> Value {
    let at = |v: &Value, k: &str| v[k].as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|t| t.with_timezone(&chrono::Utc));
    let n = day.iter().find(|x| x["kind"] == "meeting" && at(x, "end").or_else(|| at(x, "start")).is_some_and(|t| t > now));
    json!({ "next": n })
}

/// The next meeting of a day already made (`today::day()`), with what its prep says.
///
/// The next meeting with what its prep says, for Next meeting's M and L: what to walk in knowing (the prep's In short,
/// what is expected from you), what to ask (its open questions), who is coming (the invite's answers, a minute old at
/// most). No model call: a prep is written by its own job.
pub fn next_meeting(day: &[Value]) -> Value {
    with_brief(next(day, chrono::Utc::now()))
}

fn with_brief(mut v: Value) -> Value {
    let Some(pid) = v["next"]["prep"]["id"].as_str().map(str::to_owned) else { return v };
    if let Some(p) = crate::prep::load(&pid) {
        v["next"]["brief"] = brief(&p);
    }
    v["next"]["people"] = crate::prep::attendees(&pid).unwrap_or(Value::Null);
    v
}

/// A prep's few lines for Next meeting: its In short, up to 3 things expected from you, up to 2 open questions.
fn brief(p: &Value) -> Value {
    let list = |k: &str| p[k].as_array().cloned().unwrap_or_default();
    let expected: Vec<Value> = list("expected").iter().take(3).map(|x| json!({"title": x["title"], "kind": x["kind"], "late": x["late"], "from": x["from"]})).collect();
    let ask: Vec<Value> = list("open").iter().filter(|x| x["kind"] == "question").take(2).map(|x| x["title"].clone()).collect();
    // The AI profile that wrote it: Ask Claude about it starts on that one (a call's text stays on its pinned profile).
    json!({ "summary": p["summary"], "expected": expected, "ask": ask, "profile": p["profile"] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_has_one_widget_per_id_each_with_sizes_it_can_take() {
        let ids: std::collections::BTreeSet<_> = CORE.iter().map(|w| w.id).collect();
        assert_eq!(ids.len(), CORE.len(), "ids are unique");
        for w in CORE {
            assert!(w.id.starts_with("core:") && !w.label.is_empty() && !w.about.is_empty(), "{w:?}");
            assert!(!w.sizes.is_empty() && w.sizes.iter().all(|s| ["S", "M", "L"].contains(s)), "{w:?}");
            assert!(["hug", "fill"].contains(&w.height), "{w:?}");
        }
        assert!(find("core:nope").unwrap_err().to_string().contains("echoctl widget list"));
    }

    #[test]
    fn next_is_the_first_meeting_not_over_and_none_once_the_day_is_done() {
        let at = |h: u32| format!("2026-11-12T{h:02}:00:00Z");
        let day = vec![
            json!({"kind": "meeting", "subject": "Acme weekly", "start": at(9), "end": at(10)}),
            json!({"kind": "call", "subject": "A call", "start": at(10), "end": at(11)}),
            json!({"kind": "meeting", "subject": "Globex review", "start": at(11), "end": at(12)}),
        ];
        let t = |h: u32, m: u32| chrono::DateTime::parse_from_rfc3339(&format!("2026-11-12T{h:02}:{m:02}:00Z")).unwrap().with_timezone(&chrono::Utc);
        assert_eq!(next(&day, t(10, 30))["next"]["subject"], "Globex review");
        assert_eq!(next(&day, t(11, 30))["next"]["subject"], "Globex review", "one under way counts");
        assert!(next(&day, t(12, 30))["next"].is_null());
    }

    #[test]
    fn next_meetings_brief_is_the_preps_in_short_three_things_expected_and_two_questions() {
        let p = json!({
            "summary": "Pricing first.",
            "expected": [{"title": "A", "kind": "task", "late": true}, {"title": "B", "kind": "reply", "from": "Kim Park"}, {"title": "C"}, {"title": "D"}],
            "open": [{"kind": "risk", "title": "SSO may slip"}, {"kind": "question", "title": "Who signs off?"}, {"kind": "question", "title": "Monday?"}, {"kind": "question", "title": "Third?"}],
        });
        let b = brief(&p);
        assert_eq!(b["summary"], "Pricing first.");
        assert_eq!(b["expected"].as_array().unwrap().iter().map(|x| x["title"].as_str().unwrap()).collect::<Vec<_>>(), ["A", "B", "C"]);
        assert_eq!((b["expected"][0]["late"].as_bool(), b["expected"][1]["from"].as_str()), (Some(true), Some("Kim Park")));
        assert_eq!(b["ask"], json!(["Who signs off?", "Monday?"]), "questions only, two at most");
        assert_eq!(brief(&json!({}))["expected"], json!([]));
    }

    #[test]
    fn each_widget_answers_on_an_empty_data_folder_and_the_temporal_ones_say_empty() {
        let (_env, _) = crate::paths::testenv::folders("widgets-data");
        let tasks = json!({"piles": {}, "asks": [], "projects": []});
        for w in CORE {
            let v = data(w.id, &Inputs { tasks: &tasks }).unwrap_or_else(|e| panic!("{}: {e:#}", w.id));
            assert_eq!(v["empty"].as_bool(), w.temporal.then_some(true), "{}: {v}", w.id);
        }
        assert!(data("core:tasks.mine", &Inputs { tasks: &tasks }).unwrap()["mine"]["items"].is_array());
    }
}
