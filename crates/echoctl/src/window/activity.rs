//! Settings → Activity (#859): what Echo ran, from its traces (`telemetry`).

use super::*;

/// The echo:// requests of this area (`respond` asks each area in turn).
pub(super) fn route(path: &str, req: &Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>> {
    let q = req.uri().query().unwrap_or("");
    Some(match path {
        // The traces of the last `days` (1–90), newest first, every one (90 days of index lines are small): the page sums them by day, job and kind.
        "/api/activity" => {
            let days = query_param(q, "days").and_then(|d| d.parse().ok()).unwrap_or(crate::telemetry::KEEP_DAYS).clamp(1, crate::telemetry::NUMBERS_DAYS);
            json_reply(crate::telemetry::list(days, None, None, usize::MAX))
        }
        // Where traces are sent, and how it goes (the secret only as "set"), with the last "Send a test"'s outcome.
        "/api/activity/export" => {
            let mut v = crate::telemetry::export::status();
            v["test"] = TEST.lock().map(|t| t.clone()).unwrap_or(Value::Null);
            json_reply(v)
        }
        // One trace, nested (its text escaped by the page).
        "/api/activity/trace" => {
            let id = query_param(q, "id").unwrap_or_default();
            json_reply(crate::telemetry::show(&id).unwrap_or_else(|e| json!({"error": format!("{e:#}")})))
        }
        _ => return None,
    })
}

/// The last "Send a test": `{busy}`, `{ok, id}` or `{error}`.
static TEST: std::sync::Mutex<Value> = std::sync::Mutex::new(Value::Null);

/// The page's actions here: where traces go (`activity_export`, the form as shown), and a test send.
pub(super) fn handle(v: &Value) -> Option<Handled> {
    Some(match v["cmd"].as_str().unwrap_or("") {
        "activity_export" => match crate::telemetry::export::set(&v["settings"]) {
            Ok(_) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        "activity_export_test" => {
            if let Ok(mut t) = TEST.lock() {
                *t = json!({"busy": true});
            }
            std::thread::spawn(|| {
                let r = crate::telemetry::export::send_newest();
                if let Ok(mut t) = TEST.lock() {
                    *t = match r {
                        Ok(id) => json!({"ok": true, "id": id}),
                        Err(e) => json!({"error": format!("{e:#}")}),
                    };
                }
            });
            true.into()
        }
        _ => return None,
    })
}
