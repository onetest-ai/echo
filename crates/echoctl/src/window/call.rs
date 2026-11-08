//! Calls: the list, a call, its minutes and files, and what the page does to a call (`act`).

use super::*;
use super::calendar::{import_transcript, RAW_EVENTS};
#[cfg(feature = "brain")]
use super::brain::{brain_sync_now, open_in_app};

pub(super) fn minutes(meta: &Value) -> Option<i64> {
    let t = |k: &str| meta[k].as_str().and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok());
    Some(((t("ended_at")? - t("started_at")?).num_seconds() + 30).max(60) / 60)
}

pub(super) fn status(dir: &Path, recording: Option<&str>) -> &'static str {
    if recording.is_some_and(|r| Path::new(r).ends_with(dir.file_name().unwrap_or_default())) {
        "recording"
    } else if dir.join(crate::transcribe_marker()).exists() {
        "transcribing"
    } else if dir.join("notes.md").exists() {
        "notes"
    } else if dir.join("transcript.md").exists() {
        "transcript"
    } else if dir.join("transcribe.error").exists() {
        "failed"
    } else {
        "audio"
    }
}

pub(super) fn people(dir: &Path) -> Vec<String> {
    let m = read_json(&dir.join("meeting.json"));
    let mut out: Vec<String> = std::iter::once(&m["organizer"])
        .chain(m["attendees"].as_array().into_iter().flatten())
        .filter_map(|p| p["name"].as_str().filter(|n| !n.is_empty()).map(str::to_owned))
        .collect();
    out.dedup();
    out
}

/// How many calls a page holds when the page doesn't say.
pub(super) const CALLS_PAGE: usize = 60;

/// A page of the calls list (`/api/calls`), newest first: `limit` calls (≤ 500) older than `before` (a call's folder
/// name — names start with their time), those whose title, people or meeting subject hold every word of `q`. `next` is
/// the cursor of the following page (none: the end); `total` how many match.
pub(super) fn calls_page(q: &str) -> Value {
    let limit = query_param(q, "limit").and_then(|l| l.parse::<usize>().ok()).filter(|l| *l > 0).unwrap_or(CALLS_PAGE).min(500);
    let before = query_param(q, "before").filter(|b| !b.is_empty());
    let words: Vec<String> = query_param(q, "q").unwrap_or_default().to_lowercase().split_whitespace().map(str::to_owned).collect();
    let mut dirs: Vec<PathBuf> = calls::all_calls();
    dirs.reverse();
    // A search reads each call's own words (title, people, subject) — never a whole summary for a call it drops.
    if !words.is_empty() {
        dirs.retain(|d| {
            let meta = read_json(&d.join("meta.json"));
            let meeting = read_json(&d.join("meeting.json"));
            let hay = format!("{} {} {} {}", meta["title"].as_str().unwrap_or(""), d.file_name().map(|n| n.to_string_lossy().replace('-', " ")).unwrap_or_default(),
                meeting["subject"].as_str().unwrap_or(""), people(d).join(" ")).to_lowercase();
            words.iter().all(|w| hay.contains(w.as_str()))
        });
    }
    let total = dirs.len();
    let start = match &before {
        Some(b) => dirs.iter().position(|d| d.file_name().is_some_and(|n| n.to_string_lossy().as_ref() < b.as_str())).unwrap_or(dirs.len()),
        None => 0,
    };
    let page: Vec<&PathBuf> = dirs.iter().skip(start).take(limit).collect();
    let next = (start + page.len() < dirs.len()).then(|| page.last().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().into_owned())).flatten();
    json!({ "calls": page.into_iter().map(|d| call_summary(d)).collect::<Vec<_>>(), "next": next, "total": total })
}

pub(super) fn call_summary(dir: &Path) -> Value {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let meta = read_json(&dir.join("meta.json"));
    let recording = watch::running_watcher().and_then(|w| w.recording);
    let title = meta["title"].as_str().filter(|t| !t.is_empty()).map(str::to_owned).unwrap_or_else(|| {
        name.get(18..).filter(|t| !t.is_empty()).map(|t| t.replace('-', " ")).unwrap_or_else(|| "Call".into())
    });
    json!({
        "name": name,
        "title": title,
        "started_at": meta["started_at"],
        "minutes": minutes(&meta),
        "status": status(dir, recording.as_deref()),
        "people": people(dir),
        "frames": dir.join("frames.jsonl").exists(),
        "chat": dir.join("chat.json").exists(),
        "imported": crate::teams_transcript::imported(&meta),
        "minutes_at": minutes_at(dir, &meta),
        // The user alone (`memo.rs`): a mic in the list, its threads found from what was said.
        "memo": meta["kind"] == "memo",
    })
}

/// When automatic minutes come for a call that ended in the last 2 hours and has none yet (#42: the list's clock).
pub(super) fn minutes_at(dir: &Path, meta: &Value) -> Value {
    let recent = meta["ended_at"].is_string() && crate::notes::ended_at(dir).is_some_and(|e| chrono::Local::now().signed_duration_since(e) < chrono::Duration::hours(2));
    if !recent || dir.join("notes.md").exists() {
        return Value::Null;
    }
    let a = crate::notes::auto_state(dir);
    if a["due"] == "at" { a["at"].clone() } else { Value::Null }
}

pub(super) fn call_detail(dir: &Path) -> Value {
    // The revision first: anything that changes while the rest is read gives a newer one at the
    // next poll, so the page fetches again (taken last, it could stand for files read before).
    let rev = call_rev(dir);
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut v = call_summary(dir);
    let segments = match read_json(&dir.join("segments.json")) {
        Value::Array(a) => Value::Array(a),
        _ => Value::Array(
            fs::read_to_string(dir.join("live.jsonl"))
                .unwrap_or_default()
                .lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect(),
        ),
    };
    let frames: Vec<Value> = fs::read_to_string(dir.join("frames.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .map(|f| json!({ "t": f["t"], "url": format!("{}file/{}/{}", base_url(), urlencode(&name), f["file"].as_str().unwrap_or("")) }))
        .collect();
    v["notes"] = json!(fs::read_to_string(dir.join("notes.md")).ok());
    // The user's notes; one that can't be read is said, never shown as empty (the editor would start
    // from nothing).
    match fs::read_to_string(dir.join(calls::USER_NOTES)) {
        Ok(t) => v["my_notes"] = json!(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => v["my_notes"] = Value::Null,
        Err(e) => v["my_notes_error"] = json!(e.to_string()),
    }
    v["segments"] = segments;
    // A memo's parts by thread (times per thread chip); empty until its minutes split it.
    if v["memo"] == true {
        v["memo_parts"] = json!(crate::memo::parts(dir));
    }
    v["chat"] = read_json(&dir.join("chat.json"));
    v["frames"] = json!(frames);
    // With ids (a list from before ids gets them now, once: `calls::actions`).
    match calls::actions(dir) {
        Ok(a) => v["actions"] = json!(a),
        Err(e) => v["actions_error"] = json!(format!("{e:#}")),
    }
    v["meeting"] = read_json(&dir.join("meeting.json"));
    v["path"] = json!(dir.display().to_string());
    let run = crate::notes::status(dir);
    // The user's notes changed after the minutes were written — since the run that wrote them
    // started, or, for minutes without one (a session's, an older run's), since `notes.md` was
    // written: the Minutes tab offers an update.
    let mtime = |f: &str| fs::metadata(dir.join(f)).and_then(|m| m.modified()).ok().map(chrono::DateTime::<chrono::Local>::from);
    let written = match &run {
        Some(r) if r.state == "done" => r.started_at,
        _ => mtime("notes.md"),
    };
    v["notes_changed"] = json!(dir.join("notes.md").exists() && mtime(calls::USER_NOTES).zip(written).is_some_and(|(n, w)| n > w));
    v["notes_run"] = json!(run);
    v["auto"] = crate::notes::auto_state(dir);
    // The prep written before it (by its calendar event): the call's Prep tab.
    v["prep"] = crate::prep::of_call(dir).unwrap_or(Value::Null);
    v["minutes_rev"] = json!(crate::minutes::revision(dir));
    v["rev"] = json!(rev);
    v["has_log"] = json!(dir.join(crate::notes::LOG).exists());
    // What kind of meeting it was (the header's chip).
    let meeting = crate::meeting::MeetingContext::load(dir).ok().flatten();
    let (mt, types) = crate::calls::meeting_type_and_types(dir, meeting.as_ref());
    v["meeting_type"] = mt;
    v["meeting_types"] = json!(types.iter().map(|t| json!([t["id"], t["name"]])).collect::<Vec<_>>());
    // The template its minutes follow and why (a word of its title, the user's choice, Default).
    v["template"] = crate::calls::template_of(dir);
    v["meeting_subject"] = json!(meeting.and_then(|m| m.subject).filter(|s| !s.trim().is_empty()));
    // Its projects (and the ones it may belong to): the header's chips and suggestion bar.
    match crate::projects::of_call(dir) {
        Ok(ps) => {
            v["projects"] = ps.iter().map(|(p, w)| crate::projects::chip(p, w)).collect();
            v["suggested"] = json!(crate::projects::suggest(dir).unwrap_or_default());
            // What this call changed in each project's work.
            let stamp = calls::stamp_of(dir);
            let changes: Vec<Value> = ps
                .iter()
                .filter_map(|(p, _)| {
                    let mut c = crate::ledger::call_changes(&p.id, &stamp).ok()?;
                    c["project"] = json!(p.id);
                    // Not read yet: whether its sync is reading now, or when it will (the call settles
                    // first, `projectsync::SETTLE` after it ended; the scheduler looks every 5 minutes).
                    if c["read"] != true {
                        c["syncing"] = json!(crate::projectsync::state(&p.id)["state"] == "running");
                        let ended = calls::read_json(&dir.join("meta.json"))["ended_at"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok());
                        if let Some(t) = ended.filter(|_| dir.join("segments.json").is_file()) {
                            c["reads_at"] = json!((t + crate::projectsync::SETTLE).to_rfc3339());
                        }
                    }
                    Some(c)
                })
                .collect();
            v["changes"] = json!(changes);
        }
        Err(e) => v["projects_error"] = json!(format!("{e:#}")),
    }
    let profile = crate::ai::call_profile_id(dir);
    v["live"] = json!({
        "profile_error": profile.as_ref().err().map(|e| format!("{e:#}")),
        "profile": profile.ok(),
        "notes": crate::livenotes::read_notes(dir),
        "answers": crate::livenotes::read_answers(dir),
        "usage": crate::livenotes::read_usage(dir),
        "status": crate::livenotes::status(dir),
        "prep": calls::prep(dir),
    });
    v
}

/// What everything `call_detail` returns depends on, as one short hash: every file of the call's
/// folder (name, size, time) and the parts kept in memory or computed — the live notes' state, the
/// minutes run's (a dead owner changes it without a file changing), the automatic minutes'. The
/// page fetches the call again only when it changes.
pub(super) fn call_rev(dir: &Path) -> String {
    use sha2::Digest as _;
    let mut h = sha2::Sha256::new();
    let mut files: Vec<(String, u64, u128)> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let m = e.metadata().ok()?;
                    let t = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
                    Some((e.file_name().to_string_lossy().into_owned(), m.len(), t))
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    for (name, len, t) in files {
        h.update(format!("{name}\0{len}\0{t}\n"));
    }
    h.update(crate::livenotes::status(dir).to_string());
    h.update(json!(crate::notes::status(dir)).to_string());
    h.update(crate::notes::auto_state(dir).to_string());
    h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect()
}

pub(super) fn agenda() -> Value {
    let now = chrono::Utc::now();
    let mut meetings: Vec<Value> = watch::agenda()
        .into_iter()
        .filter(|m| m.end.as_deref().and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok()).is_none_or(|e| e.with_timezone(&chrono::Utc) > now))
        .map(|m| {
            let recap = m.subject.as_deref().map(calls::recap).unwrap_or_default();
            json!({ "id": m.id, "subject": m.subject, "start": m.start, "end": m.end, "join_url": m.join_url,
                    "attendees": m.attendees.len(), "recap": recap })
        })
        .collect();
    meetings.sort_by(|a, b| a["start"].as_str().cmp(&b["start"].as_str()));
    Value::Array(meetings)
}

// ── calendar: Microsoft 365 events with what Echo knows about each ─────────────────────────

/// The page's other actions; true if the page should refresh.
pub(super) fn act(v: &Value) -> bool {
    // By name, else by its stamp: a call retitled since the page (or Needs you) last saw it.
    let call = call_named(v);
    match v["cmd"].as_str().unwrap_or("") {
        "delete" => call.is_some_and(|d| calls::delete(&d.display().to_string()).is_ok()),
        "reveal" => {
            if let Some(d) = call {
                crate::app::reveal(&d);
            }
            false
        }
        // Write now: a run of this call waiting for another call's recording or transcript starts anyway
        // (Claude Code as configured only: the run decides, `notes::wait_for_local_model`).
        "notes_now" => {
            let Some(d) = call else { return false };
            if crate::notes::status(&d).is_none_or(|r| r.state != "queued") {
                return true;
            }
            let _ = std::fs::write(d.join(crate::notes::NOW_REQUEST), b"");
            true
        }
        // Not yet (#42): 15 minutes more, until the user says, or as before.
        "minutes_later" => {
            let Some(d) = call else { return false };
            // A number of minutes (0: as before); only an explicit null is "until I say".
            let minutes = match &v["minutes"] {
                Value::Number(m) => match m.as_f64() {
                    Some(f) if f.is_finite() => Some(f.round() as i64),
                    _ => return false,
                },
                Value::Null if v.get("minutes").is_some() => None,
                _ => return false,
            };
            if let Err(e) = crate::notes::set_later(&d, minutes) {
                eprintln!("minutes later: {e:#}");
            }
            true
        }
        "write_notes" => {
            let Some(d) = call else { return false };
            // Another profile for this run: it becomes the call's (`notes-run.json` records which
            // profile and model wrote the minutes). Not while a run of this call is on.
            if let Some(id) = profile(v) {
                if crate::notes::status(&d).is_some_and(|r| crate::notes::pending(&r)) {
                    return true; // the page shows that run
                }
                if crate::ai::call_profile_id(&d).ok().as_deref() != Some(id.as_str()) {
                    if let Err(e) = crate::ai::set_call_profile(&d, &id) {
                        crate::notes::refuse(&d, &format!("{e:#}"));
                        return true;
                    }
                }
            }
            // Write again with… another template: the call's from now on — chosen last, once nothing else can refuse.
            if let Some(t) = v["template"].as_str().filter(|t| !t.is_empty()) {
                if crate::notes::status(&d).is_some_and(|r| crate::notes::pending(&r)) {
                    return true;
                }
                if let Err(e) = crate::calls::choose_template(&d, t) {
                    crate::notes::refuse(&d, &format!("{e:#}"));
                    return true;
                }
            }
            crate::notes::start_in_background(d);
            true
        }
        "set_profile" => {
            let (Some(d), Some(id)) = (call, profile(v)) else { return false };
            // From now on an older choice's sign-in check opens nothing.
            let name = d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            crate::signin::choose(&format!("call:{name}"), &id);
            match crate::ai::set_call_profile(&d, &id) {
                // While it records, the watcher applies it and tells the live notes itself (so
                // no job starts on the old profile in between) — and signs it in if needed.
                Ok(crate::ai::ProfileChange::Applied) => {
                    crate::livenotes::profile_changed(&d);
                    std::thread::spawn(move || crate::signin::applied(&name, &id));
                }
                Ok(crate::ai::ProfileChange::Requested) => {}
                Err(e) => crate::livenotes::report(&d, &format!("couldn't switch to {id}: {e:#}")),
            }
            true
        }
        // Signing a Claude profile in (`signin.rs`): the flows run on threads of their own.
        "sign_in" => {
            if let Some(id) = profile(v) {
                crate::signin::start(&id);
            }
            true
        }
        "sign_in_if_needed" => {
            let (Some(id), Some(purpose)) = (profile(v), v["purpose"].as_str().filter(|p| !p.is_empty()).map(str::to_owned)) else { return false };
            // Registered here, in the order the choices came; checked on a thread of its own.
            let request = crate::signin::choose(&purpose, &id);
            std::thread::spawn(move || crate::signin::ensure_request(&id, &purpose, request));
            true
        }
        "sign_in_cancel" => {
            if let Some(id) = profile(v) {
                crate::signin::cancel(&id);
            }
            true
        }
        "check_sign_in" => {
            if let Some(p) = profile(v).and_then(|id| crate::ai::Settings::load().ok()?.get(&id).ok()) {
                crate::signin::refresh(&p, true);
            }
            true
        }
        "accept_account" => {
            let (Some(id), Some(shown)) = (profile(v), v["account"].as_str().filter(|a| !a.is_empty())) else { return false };
            crate::signin::accept(&id, shown);
            true
        }
        "live_ask" => {
            let Some(d) = call else { return false };
            let text = v["text"].as_str().unwrap_or("");
            match v["kind"].as_str().and_then(crate::livenotes::AskKind::parse) {
                Some(kind) => {
                    if let Err(e) = crate::livenotes::ask(&d, kind, text) {
                        crate::livenotes::refused(&d, kind, text, &format!("{e:#}"));
                    }
                }
                None => crate::livenotes::refused(&d, crate::livenotes::AskKind::Ask, text, "unknown kind of question"),
            }
            true
        }
        "live_pause" | "live_resume" => {
            let Some(d) = call else { return false };
            crate::livenotes::pause(&d, v["cmd"] == "live_pause");
            true
        }
        "show_log" => {
            if let Some(d) = call {
                open_file(&d.join(crate::notes::LOG));
            }
            false
        }
        // Recordings from the window use the default AI profile (the account chip sets it); the app pins it at the
        // start. A profile per recording is the CLI's (`echoctl arm --ai`).
        "record" => watch::arm(None, None, languages(v), None).is_ok(),
        // A memo: the user alone, now (the Record button's language and profile, as a call's).
        "record_memo" => match watch::memo_start(languages(v), None) {
            Ok(()) => true,
            Err(e) => {
                crate::app::notify("Memo not started", &format!("{e:#}"));
                false
            }
        },
        "stop" => watch::stop().is_ok(),
        "schedule" | "unschedule" => {
            let Some(id) = v["id"].as_str() else { return false };
            if v["cmd"] == "unschedule" {
                return crate::schedule::remove_id(id).is_ok();
            }
            let raw = RAW_EVENTS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|e| e["id"].as_str() == Some(id)).cloned();
            let meeting = raw.as_ref().and_then(crate::calendar::to_meeting)
                .or_else(|| watch::agenda().into_iter().find(|m| m.id.as_deref() == Some(id)));
            meeting.is_some_and(|mut m| {
                m.languages = languages(v).unwrap_or_default();
                crate::schedule::add(m).is_ok()
            })
        }
        "copy" => {
            if let Some(text) = v["text"].as_str() {
                copy_to_clipboard(text);
            }
            false
        }
        "calendar_import" => v["id"].as_str().is_some_and(import_transcript),
        "teams_transcript_dismiss" => v["id"].as_str().is_some_and(|id| crate::teams_transcript::dismiss(id).is_ok()),
        "arm_meeting" | "join_meeting" => {
            let id = v["id"].as_str().unwrap_or("");
            let Some(mut m) = watch::agenda().into_iter().find(|m| m.id.as_deref() == Some(id)) else { return false };
            m.languages = languages(v).unwrap_or_default();
            if v["cmd"] == "join_meeting" { watch::join(m).is_ok() } else { watch::arm(m.subject.clone(), Some(m), None, None).is_ok() }
        }
        // A Brain section's file (a thread folder's), opened in its own app: only a file the registry
        // holds, never a path the page names.
        #[cfg(feature = "brain")]
        "open_brain_file" => {
            if let Some(p) = v["id"].as_str().and_then(crate::brain::files::folder::path_of) {
                open_in_app(&p);
            }
            false
        }
        // A folder for a thread's sources, chosen in the system's own dialog (never typed by the page).
        #[cfg(feature = "brain")]
        "source_folder_pick" => {
            let id = v["id"].as_str().unwrap_or("").to_string();
            std::thread::spawn(move || {
                if let Some(dir) = crate::app::choose_folder("A folder for this thread — Echo reads its files into the Brain") {
                    let dir = dunce::canonicalize(&dir).unwrap_or(dir);
                    let sid = dir.to_string_lossy().to_string();
                    let _ = crate::projects::update(&id, |p| {
                        if !p.sources.iter().any(|s| s.kind == "folder" && s.id == sid) {
                            p.sources.push(crate::projects::Source { kind: "folder".into(), id: sid.clone(), name: dir.file_name().map(|n| n.to_string_lossy().to_string()), ..Default::default() });
                        }
                        Ok(())
                    });
                    brain_sync_now();
                }
            });
            false
        }
        "open_url" => {
            // The web, and a note in Obsidian (`obsidian://open?vault=…&file=…`: it only opens the note).
            if let Some(u) = v["url"].as_str().filter(|u| u.starts_with("https://") || u.starts_with("obsidian://open?") || u.starts_with("msteams:/")) {
                open_external(u);
            }
            false
        }
        _ => false,
    }
}

/// A text file in the user's default editor.
pub(super) fn open_file(path: &Path) {
    if !path.is_file() {
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-t").arg(path).spawn();
    #[cfg(windows)]
    let _ = std::process::Command::new("notepad").arg(path).spawn();
}


/// The echo:// requests of this area (`respond` asks each area in turn).
pub(super) fn route(path: &str, req: &Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>> {
    Some(match path {
        // The calls list, a page at a time, newest first: `?limit=&before=<name>&q=` → {calls, next, total}.
        "/api/calls" => json_reply(calls_page(req.uri().query().unwrap_or(""))),
        "/api/agenda" => json_reply(agenda()),
        p if p.starts_with("/api/call/") => {
            let rest = &p["/api/call/".len()..];
            let (name, rev) = rest.strip_suffix("/rev").map_or((rest, false), |n| (n, true));
            match call_dir(name) {
                Some(dir) if rev => json_reply(json!({ "rev": call_rev(&dir) })),
                Some(dir) => json_reply(call_detail(&dir)),
                None => reply(404, "text/plain", b"no such call".to_vec()),
            }
        }
        p if p.starts_with("/minutes/") => match call_dir(&p["/minutes/".len()..]).map(|d| crate::minutes::render_for_window(&d)) {
            // The window's theme when the user chose one (Settings → General → Appearance), else the system's.
            Some(Ok((html, csp))) => page_reply(crate::minutes::with_theme(html, query_param(req.uri().query().unwrap_or(""), "theme").as_deref()), &csp),
            Some(Err(e)) => reply(500, "text/plain; charset=utf-8", format!("The minutes can't be shown: {e:#}").into_bytes()),
            None => reply(404, "text/plain", b"no such call".to_vec()),
        },
        p if p.starts_with("/file/") => match frame_file(&p["/file/".len()..]) {
            Some(file) => match fs::read(&file) {
                Ok(bytes) => reply(200, mime_of(&file), bytes),
                Err(_) => reply(404, "text/plain", b"not found".to_vec()),
            },
            None => reply(403, "text/plain", b"forbidden".to_vec()),
        },
        _ => return None,
    })
}

/// The page's commands of this area (`handle` asks each area in turn).
pub(super) fn handle(v: &Value) -> Option<Handled> {
    Some(match v["cmd"].as_str().unwrap_or("") {
        // The calendar's chip: a meeting's type, the user's, for its whole series.
        "set_event_type" => match crate::calls::set_event_type(v["key"].as_str().unwrap_or(""), v["kind"].as_str().unwrap_or("")) {
            Ok(()) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        // The call header's meeting type: the user's, kept from then on.
        "set_meeting_type" => match crate::calls::set_meeting_type(v["call"].as_str().unwrap_or(""), v["kind"].as_str().unwrap_or("")) {
            Ok(_) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        // A call opened with its minutes: they are read (Today's "new").
        "minutes_seen" => {
            if let Some(d) = call_named(v) {
                let _ = crate::today::minutes_seen(&d);
            }
            false.into()
        }
        // The user's notes from the Notes tab (`calls::save_my_notes`); the page learns what the
        // file holds now.
        "save_my_notes" => {
            let id = v["id"].clone();
            let name = v["call"].clone();
            let answer = |r: Value| Handled { refresh: true, reply: Some(format!("window.echo && window.echo.saved({r})")) };
            let Some(dir) = call_named(v) else {
                return Some(answer(json!({ "call": name, "id": id, "error": "this call can't be found any more" })));
            };
            match calls::save_my_notes(&dir, v["text"].as_str().unwrap_or(""), v["base"].as_str().unwrap_or("")) {
                Ok(s) => answer(json!({ "call": name, "id": id, "stored": s.stored, "kept": s.kept })),
                Err(e) => answer(json!({ "call": name, "id": id, "error": format!("{e:#}") })),
            }
        }
        "open_minutes" | "reveal_minutes" => {
            let Some(dir) = call_named(v) else { return Some(false.into()) };
            match crate::minutes::write(&dir) {
                Ok(page) if v["cmd"] == "open_minutes" => crate::app::open_path(&page),
                Ok(page) => crate::app::reveal_file(&page),
                Err(e) => return Some(notice(&format!("{e:#}"))),
            }
            false.into()
        }
        _ => return None,
    })
}
