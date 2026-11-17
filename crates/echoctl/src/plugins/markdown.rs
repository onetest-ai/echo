//! A plugin's Markdown as safe HTML (`ctx.markdown(md, opts)`, host and window half): the minutes' writer
//! (`minutes::page_html`), its links and images resolved by the plugin beforehand (`ctx.markdown.links`) — Echo can't
//! call back into the plugin's JavaScript while it writes — and each `href` / `src` kept only as `allowed` lets it.

use serde_json::{Value, json};

use crate::minutes::PageDest;

/// `md` as HTML for plugin `id`; `opts`: `{breaks, links: {dest: href | null}, images: {dest: src | null}}` (a
/// destination as `links` gives it). `null`: a page that isn't there; an href Echo doesn't allow, or none given: text.
pub fn render(id: &str, md: &str, opts: &Value) -> String {
    let dest = |d: &str, image: bool| match opts[if image { "images" } else { "links" }].get(d) {
        Some(Value::Null) => PageDest::Missing,
        Some(Value::String(h)) if allowed(id, h, image) => PageDest::Href(h.clone()),
        _ => PageDest::Text,
    };
    crate::minutes::page_html(md, opts["breaks"].as_bool() == Some(true), &dest)
}

/// The destinations of `md`'s links and images, as written (a wikilink's target), each once: `{links, images}`.
pub fn links(md: &str) -> Value {
    let (links, images) = crate::minutes::page_dests(md);
    json!({"links": links, "images": images})
}

/// Whether plugin `id`'s page may carry `h`: a link to an `https://` page, an `echo://` link Echo knows or the plugin's
/// own route (`#<id>:…`); an image only from the plugin's folder route (`plugin/<id>/folder/…`, `window::plugins`) —
/// never `data:`, which would skip that route's checks (no SVG, 10 MB, the folder's grant), nor another host. Nothing
/// with whitespace, control characters or a backslash, which browsers drop or read as `/`.
fn allowed(id: &str, h: &str, image: bool) -> bool {
    if h.chars().any(|c| c.is_whitespace() || c.is_control() || c == '\\') {
        return false;
    }
    if image {
        let Some(rest) = h.strip_prefix('/').unwrap_or(h).strip_prefix(&format!("plugin/{id}/folder/")) else { return false };
        // The browser resolves `.` and `..` (`%2e` too) before it asks; Echo decodes `%2F` after: neither leaves the route.
        let decoded = decode(rest);
        return !rest.is_empty() && !rest.contains(['?', '#']) && !decoded.split(['/', '\\']).any(|s| s == "." || s == "..");
    }
    if let Some(host) = h.strip_prefix("https://") {
        return !host.is_empty() && !host.starts_with('/');
    }
    if h.starts_with("echo://") {
        return crate::links::Link::parse(h).is_some();
    }
    h.strip_prefix('#').and_then(|r| r.strip_prefix(id)).is_some_and(|r| r.starts_with(':'))
}

/// `%XX` decoded (anything else kept), as the window decodes a request's path.
fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match (b[i], s.get(i + 1..i + 3).and_then(|x| u8::from_str_radix(x, 16).ok())) {
            (b'%', Some(v)) => {
                out.push(v);
                i += 3;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(md: &str, opts: Value) -> String {
        render("acme-vault", md, &opts)
    }

    #[test]
    fn links_lists_each_destination_as_written_and_nothing_inside_code() {
        let md = "[[Page]] [[Page|label]] [[Other#Plan]] ![[assets/logo.png]] [x](https://acme.example/a) ![p](pics/a.png)\n\n`[[Code]]`\n\n```\n[[Fenced]]\n```\n\n[[Page]]";
        assert_eq!(
            links(md),
            json!({"links": ["Page", "Other#Plan", "https://acme.example/a"], "images": ["assets/logo.png", "pics/a.png"]})
        );
    }

    #[test]
    fn a_wikilink_becomes_the_link_the_plugin_resolved_with_its_label() {
        let html = page(
            "See [[Page]], [[Page|the plan]] and [[Other#Plan]].",
            json!({"links": {"Page": "#acme-vault:main/Page", "Other#Plan": "#acme-vault:main/Other?h=Plan"}}),
        );
        assert!(html.contains("<a href=\"#acme-vault:main/Page\">Page</a>"), "{html}");
        assert!(html.contains("<a href=\"#acme-vault:main/Page\">the plan</a>"), "{html}");
        assert!(html.contains("<a href=\"#acme-vault:main/Other?h=Plan\">Other#Plan</a>"), "{html}");
    }

    #[test]
    fn a_missing_page_is_a_link_without_href_the_plugin_catches() {
        let html = page("[[New \"idea\"|start it]] and ![[gone.png]]", json!({"links": {"New \"idea\"": null}, "images": {"gone.png": null}}));
        assert!(html.contains("<a class=\"md-missing\" data-md-missing=\"New &quot;idea&quot;\">start it</a>"), "{html}");
        assert!(html.contains("<span class=\"figref md-missing\" data-md-missing=\"gone.png\">[gone.png]</span>"), "{html}");
        assert!(!html.contains("href"), "{html}");
    }

    #[test]
    fn an_embed_and_an_image_show_only_from_the_plugins_own_folder_route() {
        let src = "plugin/acme-vault/folder/%2FUsers%2Fann%2Fvault/assets/logo.png";
        let html = page("![[logo.png]] ![Plan](plan.png)", json!({"images": {"logo.png": src, "plan.png": format!("/{src}")}}));
        assert!(html.contains(&format!("<img src=\"{src}\" alt=\"logo.png\" loading=\"lazy\">")), "{html}");
        assert!(html.contains(&format!("<img src=\"/{src}\" alt=\"Plan\" loading=\"lazy\">")), "{html}");
    }

    #[test]
    fn echo_keeps_only_https_echo_links_and_the_plugins_own_routes() {
        for ok in ["https://acme.example/x?a=1&b=2", "#acme-vault:main/Page", "echo://thread/atlas"] {
            assert!(allowed("acme-vault", ok, false), "{ok}");
        }
        for bad in [
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "java\nscript:alert(1)",
            " javascript:alert(1)",
            "data:text/html,<b>x</b>",
            "http://acme.example/",
            "HTTPS://acme.example/",
            "https:///acme.example",
            "https://",
            "//acme.example/x",
            "https://acme.example/a b",
            "https://acme.example/\ta",
            "https://acme.example\\@evil.example",
            "#other:main/Page",
            "#acme-vault-x:main",
            "#acme-vault",
            "#calls",
            "Page.md",
            "../Page",
            "echo://nothing-echo-knows",
            "plugin/acme-vault/folder/%2Fv/a.png",
        ] {
            assert!(!allowed("acme-vault", bad, false), "{bad:?}");
        }
    }

    #[test]
    fn an_image_from_anywhere_else_is_refused() {
        assert!(allowed("acme-vault", "plugin/acme-vault/folder/%2Fv/a.png", true));
        for bad in [
            "data:image/png;base64,iVBORw0KGgo=",
            "data:image/svg+xml,<svg onload=alert(1)>",
            "https://acme.example/a.png",
            "//acme.example/a.png",
            "plugin/other/folder/%2Fv/a.png",
            "plugin/acme-vault/store/a.png",
            "plugin/acme-vault/folder/",
            "plugin/acme-vault/folder/../../other/folder/%2Fv/a.png",
            "plugin/acme-vault/folder/%2e%2e/%2E%2E/other/a.png",
            "plugin/acme-vault/folder/%2Fv%2F..%2F..%2Fsecret.png",
            "plugin/acme-vault/folder/%2Fv/a%5C..%5Cb.png",
            "plugin/acme-vault/folder/%2Fv/a b.png",
            "plugin/acme-vault/folder/%2Fv/a.png?x",
            "plugin/acme-vault/folder/%2Fv/a.png#x",
            "plugin/acme-vault/folder\\%2Fv/a.png",
            "#acme-vault:main",
        ] {
            assert!(!allowed("acme-vault", bad, true), "{bad:?}");
        }
    }

    #[test]
    fn a_refused_or_unresolved_destination_stays_text() {
        let html = page(
            "[a](javascript:alert(1)) [b](Page.md) [[Unmapped]] [c](https://acme.example/\") ![d](data:image/png;base64,AA)",
            json!({"links": {"javascript:alert(1)": "javascript:alert(1)", "Page.md": "#other:main/Page", "https://acme.example/\"": "https://acme.example/\""}, "images": {"data:image/png;base64,AA": "data:image/png;base64,AA"}}),
        );
        assert!(!html.contains("javascript:alert(1)\""), "{html}");
        assert!(!html.contains("href=\"#other"), "{html}");
        assert!(html.contains("<a href=\"https://acme.example/&quot;\" rel=\"noopener noreferrer\">c</a>"), "{html}");
        assert!(!html.contains("<img"), "{html}");
        assert!(html.contains(">a ") || html.contains("<p>a "), "{html}");
        assert!(html.contains("Unmapped") && html.contains("[d]"), "{html}");
    }

    #[test]
    fn raw_html_and_heading_attributes_stay_text_and_breaks_are_kept_when_asked() {
        let md = "# Plan {#top}\n\n<script>alert(1)</script>\n\nline one\nline two <img src=x onerror=alert(1)> [00:12]";
        let html = page(md, json!({"breaks": true}));
        assert!(html.contains("<h1>Plan {#top}</h1>"), "{html}");
        assert!(html.contains("&lt;script&gt;") && !html.contains("<script"), "{html}");
        assert!(html.contains("line one<br>line two &lt;img"), "{html}");
        assert!(!html.contains("class=\"ts\"") && html.contains("[00:12]"), "{html}");
        let html = page(md, json!({}));
        assert!(html.contains("line one\nline two"), "{html}");
    }

    #[test]
    fn an_echo_link_is_marked_for_the_window_and_https_opens_outside() {
        let html = page("[t](t) [w](w)", json!({"links": {"t": "echo://thread/atlas", "w": "https://acme.example/w"}}));
        assert!(html.contains("<a href=\"echo://thread/atlas\" data-echo-link>t</a>"), "{html}");
        assert!(html.contains("<a href=\"https://acme.example/w\" rel=\"noopener noreferrer\">w</a>"), "{html}");
    }
}
