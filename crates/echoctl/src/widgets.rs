//! Widgets: what each part of Echo offers to Today — a catalog, and a provider per widget that returns its data.
//! Today's page and `echoctl widget show` read the same data; plugins will add theirs to the catalog (#315).
//! A provider never calls a model: what needs one is a job whose result a provider reads.

use std::borrow::Cow;

use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::{json, Value};

/// A widget the catalog offers: where it comes from, what it shows, the sizes it takes (S, M, L: 1, 2, 3 columns),
/// how tall it is (`hug` its content, `fill` its column), how many rows of Today's grid it may take (`heights`: 1, 2)
/// and whether it is temporal (on Today only while its data
/// isn't `empty`).
/// A plugin's widget (`plugin:<plugin>:<widget>`) is owned text from its manifest; Echo's own are constants.
#[derive(Serialize, Clone, Debug)]
pub struct Widget {
    pub id: Cow<'static, str>,
    pub part: Cow<'static, str>,
    pub label: Cow<'static, str>,
    pub about: Cow<'static, str>,
    pub kind: Cow<'static, str>,
    pub sizes: Cow<'static, [&'static str]>,
    pub height: Cow<'static, str>,
    pub heights: Cow<'static, [u8]>,
    pub temporal: bool,
    /// A plugin's widget: how often Echo asks its provider again, in seconds (Echo's own: 0, unused).
    #[serde(skip)]
    #[cfg_attr(not(feature = "js"), allow(dead_code))]
    pub refresh: u64,
    /// What the user may set on it (`today_layout::set_param`), given to its provider.
    pub params: Vec<crate::plugins::ParamDef>,
}

// One line a widget in the catalog below: its fields in the struct's order.
#[allow(clippy::too_many_arguments)]
fn w(id: &'static str, part: &'static str, label: &'static str, about: &'static str, kind: &'static str, sizes: &'static [&'static str], height: &'static str, temporal: bool) -> Widget {
    Widget { id: Cow::Borrowed(id), part: Cow::Borrowed(part), label: Cow::Borrowed(label), about: Cow::Borrowed(about), kind: Cow::Borrowed(kind), sizes: Cow::Borrowed(sizes), height: Cow::Borrowed(height), heights: Cow::Borrowed(&[1]), temporal, refresh: 0, params: vec![] }
}

/// The period Threads at a glance and Done this week count: this week (from Monday) or this month.
fn period() -> crate::plugins::ParamDef {
    crate::plugins::ParamDef { key: "period".into(), label: "Period".into(), kind: "choice".into(), options: vec!["week".into(), "month".into()], default: Some(json!("week")) }
}

/// Echo's own widgets.
pub fn core() -> Vec<Widget> {
    let with = |mut w: Widget, p: Vec<crate::plugins::ParamDef>| {
        w.params = p;
        w
    };
    // One or two rows high.
    let two = |mut w: Widget| {
        w.heights = Cow::Borrowed(&[1, 2]);
        w
    };
    vec![
    two(w("core:calendar.agenda", "Calendar", "Your day · agenda", "where you'll be, a reason on each line", "day", &["S", "M"], "fill", false)),
    two(w("core:calendar.calendar", "Calendar", "Your day · calendar", "by the hour, overlaps side by side", "day", &["S", "M"], "fill", false)),
    w("core:calendar.strip", "Calendar", "Your day · strip", "the whole day in one bar", "day", &["M", "L"], "hug", false),
    two(w("core:calendar.choices", "Calendar", "The day's choices", "meetings at once and invites to answer, with what Echo suggests", "choices", &["M", "L"], "hug", true)),
    two(w("core:calendar.next", "Calendar", "Next meeting", "what to walk in knowing, who's coming", "next", &["S", "M", "L"], "hug", true)),
    two(with(w("core:tasks.glance", "Tasks", "Threads at a glance", "a bar a thread: done this week, to do, waiting", "glance", &["M", "L"], "hug", false), vec![period()])),
    with(w("core:tasks.done", "Tasks", "Done this week", "how many tasks were done since Monday, and which", "glance", &["S", "M"], "hug", false), vec![period()]),
    w("core:calls.week", "Calls", "Recorded this week", "calls recorded since Monday, their time, how many have minutes", "calls", &["S", "M"], "hug", false),
    two(w("core:tasks.mine", "Tasks", "My work · this week", "late first, then by date; whose each one is", "work", &["S", "M"], "fill", false)),
    two(w("core:needs.now", "Needs you", "Needs you now", "the last 24 hours, where you watch", "needs", &["M", "L"], "hug", true)),
    two(w("core:routines.cards", "Routines", "Routine cards", "what your routines delivered to Today, until read", "cards", &["M", "L"], "hug", true)),
    ]
}

/// The catalog: Echo's widgets, then each enabled plugin's (`plugin:<plugin>:<widget>`, under the plugin's name).
pub fn catalog() -> Vec<Widget> {
    let mut all = core();
    // Each routine (or other source) that sends Today a card: its card as a widget of its own (`Routine cards` shows
    // the rest).
    for (slug, key) in crate::today::card_sources() {
        all.push(Widget {
            id: Cow::Owned(format!("{CARD}{slug}")),
            part: Cow::Borrowed("Routines"),
            label: Cow::Owned(key),
            about: Cow::Borrowed("its card, until you read it"),
            kind: Cow::Borrowed("card1"),
            sizes: Cow::Borrowed(&["M", "L"]),
            height: Cow::Borrowed("hug"),
            heights: Cow::Borrowed(&[1]),
            temporal: true,
            refresh: 0,
            params: vec![],
        });
    }
    for (p, m) in crate::plugins::enabled_manifests() {
        for d in m.widgets {
            all.push(Widget {
                id: Cow::Owned(format!("plugin:{}:{}", p, d.id)),
                part: Cow::Owned(m.name.clone()),
                label: Cow::Owned(d.label),
                about: Cow::Owned(d.about),
                kind: Cow::Owned(d.kind),
                sizes: Cow::Owned(d.sizes.iter().filter_map(|s| ["S", "M", "L"].into_iter().find(|x| x == s)).collect()),
                height: Cow::Owned(d.height),
                heights: Cow::Owned(d.heights),
                // A plugin's widget is on Today while its plugin has given it something to show.
                temporal: true,
                refresh: d.refresh.unwrap_or(60).max(60),
                params: d.params,
            });
        }
    }
    all
}

/// A card source's widget id: this, then its slug (`today::card_slug`).
pub const CARD: &str = "core:routines.card.";

pub fn find(id: &str) -> Result<Widget> {
    let hit = if id.starts_with("plugin:") || id.starts_with(CARD) { catalog().into_iter().find(|w| w.id == id) } else { core().into_iter().find(|w| w.id == id) };
    hit.ok_or_else(|| anyhow!("no widget {id:?}: `echoctl widget list` names them"))
}

/// What a provider reads beyond its own part: the Tasks view, made by the caller (the window from its calendar, the
/// CLI from the agenda) — so both show the same work.
pub struct Inputs<'a> {
    pub tasks: &'a Value,
}

/// A widget's data. A temporal widget's says `empty` when it has nothing (Today takes no space for it).
pub fn data(id: &str, i: &Inputs) -> Result<Value> {
    let w = find(id)?;
    if let Some(rest) = id.strip_prefix("plugin:") {
        let (plugin, widget) = rest.split_once(':').unwrap_or((rest, ""));
        return Ok(plugin_data(plugin, widget, &w));
    }
    let mut v = match w.kind.as_ref() {
        "day" | "choices" => day(),
        "next" => next_meeting(&crate::today::day()),
        "work" => crate::today::work(i.tasks),
        "needs" => crate::needs::view(),
        "cards" => json!({ "cards": crate::today::cards() }),
        "card1" => json!({ "cards": crate::today::cards().into_iter().filter(|c| id.strip_prefix(CARD) == c["slug"].as_str()).collect::<Vec<_>>() }),
        "glance" => crate::glance::today(crate::today_layout::params_of(id).get("period").and_then(Value::as_str).unwrap_or("week")),
        "calls" => crate::calls_week::today(),
        k => return Err(anyhow!("widget {id}: no provider for kind {k}")),
    };
    if w.temporal {
        let empty = match w.kind.as_ref() {
            "next" => v["next"].is_null(),
            "needs" => v["items"].as_array().is_none_or(|a| a.is_empty()),
            "cards" | "card1" => v["cards"].as_array().is_none_or(|a| a.is_empty()),
            "choices" => !has_choices(v["day"].as_array().map(Vec::as_slice).unwrap_or(&[]), chrono::Utc::now()),
            _ => false,
        };
        v["empty"] = json!(empty);
    }
    Ok(v)
}

/// A plugin's widget: the last data its plugin gave (`ctx.widgets.set`, or its provider's answer), with when; asking
/// its provider again, in the background, once that is older than the widget's `refresh` (60 s at least). Never waits
/// for the plugin: Today draws the last answer at once.
#[cfg(not(feature = "js"))]
fn plugin_data(_plugin: &str, _widget: &str, w: &Widget) -> Value {
    json!({"kind": w.kind, "label": label_for(w), "data": null, "empty": true})
}

/// Its label on Today: one set to a thread or a client (a setting of type `thread` / `client`) names it — "GitHub at
/// a glance · Echo" — so a widget showing one thread says which. Whatever the plugin is.
/// [`label_on_today`] with its settings as Today has them — read only for a widget that has a thread or client setting.
fn label_for(w: &Widget) -> String {
    if !w.params.iter().any(|p| p.kind == "thread" || p.kind == "client") {
        return w.label.to_string();
    }
    label_on_today(w, &crate::today_layout::params_of(&w.id))
}

fn label_on_today(w: &Widget, set: &serde_json::Map<String, Value>) -> String {
    let named: Vec<String> = w
        .params
        .iter()
        .filter_map(|p| {
            let v = set.get(&p.key)?.as_str().filter(|s| !s.is_empty())?;
            match p.kind.as_str() {
                "thread" => crate::projects::load(v).ok().map(|t| t.name),
                "client" => crate::projects::clients().ok()?.into_iter().find(|c| c.id == v).map(|c| c.name),
                _ => None,
            }
        })
        .filter(|n| !n.trim().is_empty())
        .collect();
    if named.is_empty() {
        w.label.to_string()
    } else {
        format!("{} · {}", w.label, named.join(" · "))
    }
}

#[cfg(feature = "js")]
fn plugin_data(plugin: &str, widget: &str, w: &Widget) -> Value {
    let last = crate::jshost::widget_last(plugin, widget);
    let every = w.refresh.max(60);
    let stale = last.as_ref().and_then(|l| l["asked"].as_str().or_else(|| l["at"].as_str())).and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).is_none_or(|t| (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_seconds() >= every as i64);
    // Resized since its provider was last asked: asked again for the new size.
    let resized = last.as_ref().is_some_and(|l| l["size"].as_str() != Some(crate::today_layout::size_on_today(&format!("plugin:{plugin}:{widget}")).as_str()));
    if stale || resized {
        crate::jshost::widget_ask(plugin, widget);
    }
    let data = last.as_ref().map(|l| l["data"].clone()).unwrap_or(Value::Null);
    // Never answered yet (being asked now): Today holds its place until the answer comes.
    json!({"kind": w.kind, "label": label_for(w), "data": data, "at": last.as_ref().map(|l| l["at"].clone()), "error": last.as_ref().map(|l| l["error"].clone()), "empty": data.is_null(), "pending": last.is_none()})
}

/// Whether the day still holds a choice (the window's `dayChoices` says which): an invite not answered and not over, or
/// two meetings you accepted at once, not over.
fn has_choices(day: &[Value], now: chrono::DateTime<chrono::Utc>) -> bool {
    let at = |v: &Value, k: &str| v[k].as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|t| t.with_timezone(&chrono::Utc));
    let span = |x: &Value| Some((at(x, "start")?, at(x, "end").or_else(|| at(x, "start"))?));
    let meeting = |x: &&Value| x["kind"] == "meeting";
    let open: Vec<(&Value, _)> = day.iter().filter(meeting).filter_map(|x| span(x).filter(|(_, e)| *e > now).map(|s| (x, s))).collect();
    if open.iter().any(|(x, _)| x["response"] == "notResponded") {
        return true;
    }
    let yes: Vec<_> = open.iter().filter(|(x, _)| !matches!(x["response"].as_str(), Some("tentativelyAccepted" | "declined"))).map(|(_, s)| *s).collect();
    yes.iter().enumerate().any(|(i, a)| yes[i + 1..].iter().any(|b| a.0 < b.1 && b.0 < a.1))
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
    // One the user goes to: not one they haven't answered, nor a maybe.
    let going = |x: &Value| !matches!(x["response"].as_str(), Some("notResponded" | "tentativelyAccepted"));
    let n = day.iter().find(|x| x["kind"] == "meeting" && going(x) && at(x, "end").or_else(|| at(x, "start")).is_some_and(|t| t > now));
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
    fn a_widget_set_to_a_thread_or_a_client_names_it_in_its_label() {
        let (_env, _) = crate::paths::testenv::folders("widgets-label-names-thread");
        let echo = crate::projects::create("Echo", None, None).unwrap();
        let acme = crate::projects::add_client("Acme", &["@acme.example".into()], false).unwrap();
        let param = |key: &str, kind: &str| crate::plugins::ParamDef { key: key.into(), label: key.into(), kind: kind.into(), options: vec![], default: None };
        let mut wd = w("plugin:gh-ish:glance", "plugins", "GitHub at a glance", "x", "stats", &["M"], "hug", false);
        wd.params = vec![param("thread", "thread"), param("ci", "bool")];
        let set = |v: Value| v.as_object().cloned().unwrap();
        assert_eq!(label_on_today(&wd, &set(json!({}))), "GitHub at a glance", "none set: its own label");
        assert_eq!(label_on_today(&wd, &set(json!({"thread": echo.id, "ci": true}))), "GitHub at a glance · Echo");
        assert_eq!(label_on_today(&wd, &set(json!({"thread": "gone-thread"}))), "GitHub at a glance", "a thread that isn't there: its own label");
        wd.params.push(param("client", "client"));
        assert_eq!(label_on_today(&wd, &set(json!({"thread": echo.id, "client": acme.id}))), "GitHub at a glance · Echo · Acme");
    }

    #[cfg(feature = "js")]
    #[test]
    fn a_widget_never_answered_whose_ask_failed_says_why_not_pending_and_one_answered_with_nothing_doesnt() {
        // Today draws the first as its frame with "Couldn't get its data — <why>" and takes no place for the second.
        let (_env, _) = crate::paths::testenv::folders("widgets-failed-first-ask");
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let dir = crate::plugins::data_dir("mix-demo");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("widgets.json"), json!({
            "hang": {"asked": now, "data": null, "error": "mix-demo didn't answer __widget:hang within 5 s"},
            "none": {"asked": now, "at": now, "data": null},
        }).to_string()).unwrap();
        let mut wd = w("plugin:mix-demo:hang", "plugins", "Never answers", "x", "stat", &["S", "M"], "hug", false);
        wd.refresh = 3600;
        let v = plugin_data("mix-demo", "hang", &wd);
        assert_eq!((v["empty"].as_bool(), v["pending"].as_bool(), v["at"].as_str()), (Some(true), Some(false), None), "{v}");
        assert_eq!(v["error"], "mix-demo didn't answer __widget:hang within 5 s");
        let v = plugin_data("mix-demo", "none", &wd);
        assert_eq!((v["empty"].as_bool(), v["pending"].as_bool(), v["error"].is_null()), (Some(true), Some(false), true), "{v}");
    }

    #[test]
    fn the_catalog_has_one_widget_per_id_each_with_sizes_it_can_take() {
        let core = core();
        let ids: std::collections::BTreeSet<_> = core.iter().map(|w| w.id.as_ref()).collect();
        assert_eq!(ids.len(), core.len(), "ids are unique");
        for w in &core {
            assert!(w.id.starts_with("core:") && !w.label.is_empty() && !w.about.is_empty(), "{w:?}");
            assert!(!w.sizes.is_empty() && w.sizes.iter().all(|s| ["S", "M", "L"].contains(s)), "{w:?}");
            assert!(["hug", "fill"].contains(&w.height.as_ref()), "{w:?}");
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
        // One not answered, or a maybe, isn't where you'll be next.
        let unsure = vec![json!({"kind": "meeting", "subject": "Ops", "start": at(11), "end": at(12), "response": "notResponded"}), json!({"kind": "meeting", "subject": "Lunch", "start": at(12), "end": at(13), "response": "tentativelyAccepted"}), json!({"kind": "meeting", "subject": "Demo", "start": at(13), "end": at(14), "response": "accepted"})];
        assert_eq!(next(&unsure, t(10, 0))["next"]["subject"], "Demo");
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
    fn the_days_choices_are_an_open_invite_or_two_accepted_at_once() {
        let now = chrono::Utc::now();
        let t = |h: i64| (now + chrono::Duration::hours(h)).to_rfc3339();
        let m = |s: i64, e: i64, r: &str| json!({"kind": "meeting", "start": t(s), "end": t(e), "response": r});
        assert!(!has_choices(&[m(1, 2, "accepted"), m(3, 4, "organizer")], now), "none at once");
        assert!(has_choices(&[m(1, 3, "accepted"), m(2, 4, "organizer")], now), "two accepted at once");
        assert!(!has_choices(&[m(1, 3, "accepted"), m(2, 4, "tentativelyAccepted")], now), "a maybe isn't a choice");
        assert!(has_choices(&[m(5, 6, "notResponded")], now), "an invite to answer");
        assert!(!has_choices(&[m(-3, -2, "notResponded"), m(-3, -1, "accepted"), m(-3, -1, "accepted")], now), "what is over");
    }

    #[test]
    fn each_widget_answers_on_an_empty_data_folder_and_the_temporal_ones_say_empty() {
        let (_env, _) = crate::paths::testenv::folders("widgets-data");
        let tasks = json!({"piles": {}, "asks": [], "projects": []});
        for w in core() {
            let v = data(&w.id, &Inputs { tasks: &tasks }).unwrap_or_else(|e| panic!("{}: {e:#}", w.id));
            assert_eq!(v["empty"].as_bool(), w.temporal.then_some(true), "{}: {v}", w.id);
        }
        assert!(data("core:tasks.mine", &Inputs { tasks: &tasks }).unwrap()["mine"]["items"].is_array());
    }
}
