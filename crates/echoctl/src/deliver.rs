//! Sending text to the user (`docs/plugins-design.md`, the `deliver` capability). Echo's own ways —
//! their Teams chat with themselves, their own mailbox (or, for mail only, the addresses the user set on
//! a routine: `Msg::mail_to`) — and the ways plugins add (`ctx.deliver.provide`: Telegram, from its
//! plugin), which Echo hands the message to (`jshost::provider_send`). Plugins send through it
//! (`ctx.deliver.send`); never asked by a model, never to an address a model chose.
//!
//! - **Teams**: Graph `POST /chats/48:notes/messages` — the chat with yourself — through the Microsoft
//!   365 sign-in (its token carries `Chat.ReadWrite`).
//! - **Mail**: Graph `POST /me/sendMail` (`Mail.Send`) to the account's own address, not kept in Sent —
//!   or to the routine's `mail_to`, kept in Sent like any mail of the user's.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

/// Echo's own ways to send; a plugin adds others (`ctx.deliver.provide`).
pub const BUILTIN: [&str; 2] = ["teams", "mail"];

/// A way to send's name, as manifests and routines write it.
pub fn valid_kind(k: &str) -> bool {
    (1..=24).contains(&k.len()) && k.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// What can be sent now, for Settings and the plugins (no secrets): Echo's ways and the plugins'.
pub fn status() -> Value {
    let ms = crate::graph::account();
    #[cfg(feature = "js")]
    let provided = crate::jshost::providers();
    #[cfg(not(feature = "js"))]
    let provided: Vec<Value> = vec![];
    json!({"teams": ms.is_some(), "mail": ms.is_some(), "account": ms, "provided": provided})
}

/// Can `kind` be sent to now? (Why not, otherwise.)
pub fn ready(kind: &str) -> Result<()> {
    match kind {
        "teams" | "mail" if crate::graph::account().is_some() => Ok(()),
        "teams" | "mail" => bail!("not signed in to Microsoft 365 (Settings → Calendar & chat)"),
        #[cfg(feature = "js")]
        k => crate::jshost::provider_ready(k),
        #[cfg(not(feature = "js"))]
        k => bail!("“{k}” isn't a way Echo sends"),
    }
}

/// The line under anything a model wrote (`ai`: which model, "" when not known).
pub fn ai_notice(model: &str) -> String {
    let who = if model.trim().is_empty() { "Claude".to_string() } else { format!("Claude, {}", model.trim()) };
    format!("Written by AI ({who}) from Echo's data — it can contain mistakes. Check what matters before you act on it.")
}

/// What to send.
#[derive(Default)]
pub struct Msg<'a> {
    pub title: &'a str,
    pub markdown: &'a str,
    /// One line on where it comes from ("Routine · Weekdays at 08:30"): under the title of a mail, at
    /// the foot of a chat message.
    pub about: Option<&'a str>,
    /// The model, when a model wrote it: the message then says so (`ai_notice`).
    pub ai: Option<&'a str>,
    /// Mail only: other recipients the user set (empty: the user).
    pub mail_to: &'a [String],
}

/// A mail address as a routine may name one: plain `name@domain.tld`, nothing that could carry more.
pub fn valid_address(a: &str) -> bool {
    let a = a.trim();
    let Some((user, domain)) = a.split_once('@') else { return false };
    a.len() <= 254
        && !user.is_empty()
        && user.bytes().all(|b| b.is_ascii_alphanumeric() || b"._%+-'".contains(&b))
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
}

/// Send by `kind`.
pub fn send(kind: &str, m: &Msg) -> Result<()> {
    let notice = m.ai.map(ai_notice);
    let foot_lines: Vec<String> = [m.about.map(str::to_owned), notice.clone()].into_iter().flatten().collect();
    match kind {
        "teams" => {
            let foot: String = foot_lines.iter().map(|a| format!("<p><em>{}</em></p>", esc(a))).collect();
            let html = format!("<h3>{}</h3>{}{foot}", esc(m.title), crate::minutes::markdown_html(m.markdown));
            crate::graph::post("/chats/48:notes/messages", &json!({"body": {"contentType": "html", "content": html}})).context("Teams")?;
            Ok(())
        }
        "mail" => {
            use base64::Engine as _;
            let me = crate::graph::my_address()?;
            if let Some(bad) = m.mail_to.iter().find(|a| !valid_address(a)) {
                bail!("“{bad}” isn't a mail address");
            }
            let others = !m.mail_to.is_empty();
            let to: Vec<Value> = if others { m.mail_to.iter().map(|a| json!({"emailAddress": {"address": a.trim()}})).collect() } else { vec![json!({"emailAddress": {"address": me}})] };
            let html = mail_html(m.title, &crate::minutes::markdown_html(m.markdown), m.about, notice.as_deref(), others.then_some(me.as_str()));
            // Echo's mark, as an inline attachment (`cid:`): mail apps show those, not remote or data: images.
            let logo = json!({"@odata.type": "#microsoft.graph.fileAttachment", "name": "echo.png", "contentType": "image/png",
                              "contentBytes": base64::engine::general_purpose::STANDARD.encode(LOGO), "isInline": true, "contentId": "echo-logo"});
            // Mail to others is kept in Sent, as any mail of yours; a note to yourself isn't.
            let body = json!({"message": {"subject": m.title, "body": {"contentType": "HTML", "content": html}, "toRecipients": to, "attachments": [logo]}, "saveToSentItems": others});
            crate::graph::post("/me/sendMail", &body).context("mail")?;
            Ok(())
        }
        // A plugin's way (Telegram…): the plugin sends it.
        #[cfg(feature = "js")]
        k => crate::jshost::provider_send(k, m),
        #[cfg(not(feature = "js"))]
        k => bail!("“{k}” isn't a way Echo sends"),
    }
}

/// Echo's mark (the onetest octopus), 192 px, shown at 28.
const LOGO: &[u8] = include_bytes!("../assets/onetest-logo.png");

/// The mail's look is Echo's minutes page: the octopus and the day above one plain card, the text in
/// Echo's type, violet only where it means something (links, code, the AI mark).
///
/// Dark mode (researched 2026-11-03: Litmus, caniemail, Courier): Outlook — the new one on the Mac, the
/// web — recolours a mail *after* its CSS ("partial inversion": light backgrounds darkened to a mid grey,
/// dark text lightened), and doesn't reliably honour `prefers-color-scheme` or `color-scheme: light
/// only`. What survives it: no grey-on-grey hierarchy. Every text is Echo's ink (#18181d — inverted, it
/// turns light on Outlook's grey); rank comes from size, weight and case, never from a lighter grey (a
/// `#55555f` label is 7:1 on white but ~1.3:1 on the darkened card). Rules are a mid grey, visible on
/// both. No fills to darken (the reader's own background, the card's border); the AI mark and the tables'
/// head rows are saturated violet with white — reports' tinted heads, which no client inverts. Zebra rows
/// are a faint tint (darkened, they stay a faint step). Apple Mail honours the media query: Echo's dark colours.
const MAIL_CSS: &str = "
.e-text, .e-text td, .e-text th { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI Variable Text', 'Segoe UI', system-ui, Helvetica, Arial, sans-serif; color: #18181d; }
.e-text { font-size: 14.5px; line-height: 1.6; }
.e-text h1, .e-text h2 { font-size: 15.5px; font-weight: 700; margin: 26px 0 8px; color: #18181d; }
.e-text h3 { font-size: 14.5px; font-weight: 700; margin: 18px 0 6px; color: #18181d; }
.e-text p { margin: 0 0 10px; }
.e-text ul, .e-text ol { padding-left: 22px; margin: 0 0 14px; }
.e-text li { margin: 5px 0; }
.e-text strong, .e-text b { font-weight: 700; }
.e-text a { color: #5b34d6; }
.e-text code { font-family: ui-monospace, 'SF Mono', 'Cascadia Mono', Menlo, Consolas, monospace; font-size: 12.5px; color: #5b34d6; }
.e-text table { border-collapse: separate; border-spacing: 0; width: 100%; margin: 10px 0 18px; font-size: 13.5px; line-height: 1.45; border: 1px solid #c9c4dd; border-radius: 8px; overflow: hidden; }
.e-text th { text-align: left; font-size: 12.5px; font-weight: 650; color: #ffffff; background: #5b34d6; padding: 9px 12px; }
.e-text td { padding: 9px 12px; vertical-align: top; border-top: 1px solid #ddd8ec; }
.e-text tr.e-alt td { background: #f6f4fc; }
.e-text td:first-child { white-space: nowrap; font-weight: 700; font-variant-numeric: tabular-nums; }
.e-text blockquote { margin: 10px 0 14px; padding: 2px 14px; border-left: 2px solid #9a9aa5; }
.e-text hr { border: 0; border-top: 1px solid #b9b9c2; margin: 22px 0; }
@media (prefers-color-scheme: dark) {
  .e-card { background: #1b1b21 !important; border-color: #3a3a43 !important; }
  .e-ink, .e-text, .e-text td, .e-text th, .e-text h1, .e-text h2, .e-text h3 { color: #ececf1 !important; }
  .e-text a, .e-text code { color: #c4b0ff !important; }
  .e-text th { color: #ffffff !important; background: #5b34d6 !important; }
  .e-text table { border-color: #3a3a43 !important; }
  .e-text td, .e-text hr, .e-rule { border-color: #3a3a43 !important; }
  .e-text tr.e-alt td { background: #22222a !important; }
}
";

/// A mail of Echo's: the octopus, "Echo" and the day; the card (a violet bar, the title, what sent it,
/// the text, the AI notice); a foot saying who sent it and where to change it — laid out in tables, as
/// mail apps need. `from` = the user, when it goes to others.
/// Every second body row marked (`e-alt`), as mail apps drop `:nth-child`.
fn zebra(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 64);
    let mut n = 0usize;
    let mut rest = html;
    while let Some(i) = rest.find("<tr>") {
        out.push_str(&rest[..i]);
        // A head row isn't counted; each table starts again.
        let before = &out[out.rfind("<table").unwrap_or(0)..];
        if before.contains("<tbody>") {
            n += 1;
            out.push_str(if n.is_multiple_of(2) { "<tr class=\"e-alt\">" } else { "<tr>" });
        } else {
            n = 0;
            out.push_str("<tr>");
        }
        rest = &rest[i + 4..];
    }
    out.push_str(rest);
    out
}

fn mail_html(title: &str, body: &str, about: Option<&str>, notice: Option<&str>, from: Option<&str>) -> String {
    let body = zebra(body);
    let date = chrono::Local::now().format("%A, %-d %B").to_string();
    let font = "font-family:-apple-system,BlinkMacSystemFont,'Segoe UI Variable Display','Segoe UI',system-ui,Helvetica,Arial,sans-serif";
    let about_line = about.map(|a| format!(r#"<div class="e-ink" style="{font};font-size:13px;color:#18181d;margin-top:5px">{}</div>"#, esc(a))).unwrap_or_default();
    let notice_line = notice
        .map(|n| format!(r#"<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="margin-top:22px"><tr><td class="e-ink e-rule" style="border-top:1px solid #b9b9c2;padding:12px 0 0;{font};font-size:12.5px;line-height:1.5;color:#18181d"><b style="display:inline-block;font-size:10.5px;font-weight:700;letter-spacing:.04em;color:#ffffff;background:#5b34d6;border-radius:4px;padding:1px 6px;margin-right:7px">AI</b>{}</td></tr></table>"#, esc(n)))
        .unwrap_or_default();
    let foot = match from {
        Some(me) => format!("Sent by {} with Echo.", esc(me)),
        None => "Sent to you by Echo, on your computer. Change or stop it in Echo → Settings → Plugins → Scheduler.".to_string(),
    };
    format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="color-scheme" content="light dark"><meta name="supported-color-schemes" content="light dark"><style>{MAIL_CSS}</style></head>
<body style="margin:0;padding:0">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0"><tr><td align="center" style="padding:24px 12px 32px">
<table role="presentation" width="680" cellpadding="0" cellspacing="0" style="max-width:680px;width:100%">
<tr><td style="padding:0 4px 12px">
  <table role="presentation" width="100%" cellpadding="0" cellspacing="0"><tr>
    <td style="vertical-align:middle;width:32px"><img src="cid:echo-logo" width="24" height="24" alt="" style="display:block;border:0;border-radius:6px"></td>
    <td class="e-ink" style="vertical-align:middle;{font};font-size:14px;font-weight:700;color:#18181d">Echo</td>
    <td class="e-ink" align="right" style="vertical-align:middle;{font};font-size:12.5px;color:#18181d">{date}</td>
  </tr></table>
</td></tr>
<tr><td class="e-card" style="border:1px solid #b9b9c2;border-radius:10px;padding:28px 34px 26px">
  <div class="e-ink" style="{font};font-size:22px;font-weight:650;letter-spacing:-0.02em;line-height:1.25;color:#18181d">{title}</div>
  {about_line}
  <div class="e-text" style="font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',system-ui,Helvetica,Arial,sans-serif;font-size:14.5px;line-height:1.6;color:#18181d;margin-top:4px">{body}</div>
  {notice_line}
</td></tr>
<tr><td class="e-ink" style="padding:14px 4px 0;{font};font-size:11.5px;line-height:1.5;color:#18181d">{foot}</td></tr>
</table></td></tr></table></body></html>"#,
        title = esc(title),
        date = esc(&date),
    )
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mail_is_echos_card_with_the_text_escaped() {
        let h = mail_html("Plan <today>", &crate::minutes::markdown_html("## Today\n\n| a | b |\n|---|---|\n| 1 | 2 |"), Some("Routine · Weekdays at 08:30"), Some(&ai_notice("sonnet")), None);
        assert!(h.contains("Written by AI (Claude, sonnet)"));
        assert!(h.contains("Plan &lt;today&gt;") && h.contains("cid:echo-logo") && h.contains("Routine · Weekdays at 08:30"));
        assert!(h.contains("<table") && h.contains("e-text"));
        let z = zebra("<table><thead><tr><th>a</th></tr></thead><tbody><tr><td>1</td></tr><tr><td>2</td></tr><tr><td>3</td></tr></tbody></table><table><thead><tr><th>b</th></tr></thead><tbody><tr><td>1</td></tr><tr><td>2</td></tr></tbody></table>");
        assert_eq!(z.matches("e-alt").count(), 2, "{z}");
        assert!(valid_address("jane.doe+x@acme.example") && !valid_address("a@b") && !valid_address("x@y.com, z@w.com") && !valid_address("\"a\"@b.com"));
        // ECHOCTL_MAIL_PREVIEW=<file>: write it, to look at it in a browser.
        if let Some(f) = std::env::var_os("ECHOCTL_MAIL_PREVIEW") {
            let body = crate::minutes::markdown_html(&std::fs::read_to_string(std::env::var("ECHOCTL_MAIL_PREVIEW_MD").unwrap_or_default()).unwrap_or_default());
            std::fs::write(f, mail_html("План на день", &body, Some("Routine · Weekdays at 08:30"), Some(&ai_notice("sonnet")), None)).unwrap();
        }
    }

    #[test]
    fn only_known_ways() {
        let (_env, _) = crate::paths::testenv::folders("deliver");
        assert!(ready("slack").is_err());
        assert!(valid_kind("telegram") && !valid_kind("Tele gram") && !valid_kind(""));
    }
}
