//! Knowledge vault routes and actions.

use super::*;

pub(super) fn route(path: &str, req: &Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>> {
    Some(match path {
        // Knowledge (spec K4–K9): the vaults, a vault's tree, pages, graph, checks, history, images; the folder picked.
        "/api/kn" => json_reply(crate::knowledge::api::state()),
        "/api/kn-resume" => json_reply(json!({ "command": query_param(req.uri().query().unwrap_or(""), "vault").and_then(|v| crate::terminal::vault_resume_command(&v)) })),
        p if p.starts_with("/api/kn/") => match crate::knowledge::api::route(&p["/api/kn/".len()..], req.uri().query().unwrap_or("")) {
            Ok(crate::knowledge::api::Reply::Json(v)) => json_reply(v),
            Ok(crate::knowledge::api::Reply::Bytes(b, mime)) => reply(200, mime, b),
            Err(_) if p.ends_with("/asset") => reply(404, "text/plain", b"not found".to_vec()),
            Err(e) => json_reply(json!({ "error": format!("{e:#}") })),
        },
        _ => return None,
    })
}

pub(super) fn handle(v: &Value) -> Option<Handled> {
    Some(match v["cmd"].as_str().unwrap_or("") {
        // Knowledge: the system's folder dialog (the page polls `/api/kn/pick`), a file shown in Finder / Explorer or
        // opened in Obsidian, and every other `kn_*` action as a job the page polls (`/api/kn/jobs`).
        "kn_pick" => {
            crate::knowledge::api::pick(if v["for"] == "new" { "A folder for the new vault (new or empty)" } else { "A folder of notes to connect — nothing in it changes" });
            false.into()
        }
        "kn_reveal" => {
            if let Ok(p) = crate::knowledge::api::file_of(v["id"].as_str().unwrap_or(""), v["path"].as_str().unwrap_or("")) {
                if p.is_dir() { crate::app::reveal(&p) } else { crate::app::reveal_file(&p) }
            }
            false.into()
        }
        "kn_obsidian" => {
            if let Ok(u) = crate::knowledge::api::obsidian_uri(v["id"].as_str().unwrap_or(""), v["path"].as_str().unwrap_or("")) {
                open_external(&u);
            }
            false.into()
        }
        // Whether the page takes files dropped on the window now (Ingest shown or its sheet open): on or off only.
        "kn_drops" => {
            crate::knowledge::api::take_drops(v["on"] == true);
            false.into()
        }
        // Files were dropped on the window (Echo's own word, from the drop: never their paths): the page opens Ingest's
        // sheet with them.
        "kn_dropped" => Handled { refresh: false, reply: Some(format!("window.echo && window.echo.knDropped && window.echo.knDropped({})", crate::knowledge::api::drop_value())) },
        cmd if cmd.starts_with("kn_") => {
            let (op, cmd, args) = (cmd.to_string(), cmd.to_string(), v.clone());
            let seq = crate::knowledge::api::start_job(&op, move || {
                let r = crate::knowledge::api::act(&cmd, &args);
                crate::attention::stale();
                r
            });
            Handled { refresh: false, reply: Some(format!("window.echo && window.echo.knJob && window.echo.knJob({seq})")) }
        }
        _ => return None,
    })
}
