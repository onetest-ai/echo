//! Settings → Activity (#859): what Echo ran, from its traces (`telemetry`).

use super::*;

/// The echo:// requests of this area (`respond` asks each area in turn).
pub(super) fn route(path: &str, req: &Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>> {
    let q = req.uri().query().unwrap_or("");
    Some(match path {
        // The traces of the last `days` (1–90), newest first, every one: the page sums them by day, job and kind.
        "/api/activity" => {
            let days = query_param(q, "days").and_then(|d| d.parse().ok()).unwrap_or(crate::telemetry::KEEP_DAYS).clamp(1, crate::telemetry::NUMBERS_DAYS);
            json_reply(crate::telemetry::list(days, None, None, 20_000))
        }
        // One trace, nested (its text escaped by the page).
        "/api/activity/trace" => {
            let id = query_param(q, "id").unwrap_or_default();
            json_reply(crate::telemetry::show(&id).unwrap_or_else(|e| json!({"error": format!("{e:#}")})))
        }
        _ => return None,
    })
}
