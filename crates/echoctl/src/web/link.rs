//! Addresses a page links to: absolute http(s) only, as Echo writes them into Markdown (`web_link`).

/// The longest address kept: a longer one is no link.
pub const MAX_URL: usize = 2048;

use super::read::bad_char;

/// `http` or `https` and what follows `://`, for an address that starts so (in any case).
pub(super) fn web_scheme(u: &str) -> Option<(&'static str, &str)> {
    ["https", "http"].into_iter().find_map(|sch| {
        let n = sch.len() + 3;
        (u.get(..n).is_some_and(|p| p.eq_ignore_ascii_case(&format!("{sch}://")))).then(|| (sch, &u[n..]))
    })
}

/// `href` as an absolute http(s) address, if it is one — a relative one against the page's own http(s) address.
/// Anything else (`javascript:`, `mailto:`, `data:`, `file:`, this page's own anchors, an address with a user part, a
/// relative one on a page without an address) is text.
pub fn web_link(base: &str, href: &str) -> Option<String> {
    let h: String = href.chars().filter(|c| !matches!(c, '\t' | '\n' | '\r')).collect();
    let h = h.trim();
    if h.is_empty() || h.starts_with('#') {
        return None;
    }
    let base_web = web_scheme(base);
    let abs = if let Some((sch, rest)) = web_scheme(h) {
        format!("{sch}://{rest}")
    } else if let Some(rest) = h.strip_prefix("//") {
        format!("{}://{rest}", base_web.map_or("https", |(s, _)| s))
    } else if has_scheme(h) {
        return None;
    } else {
        let (sch, b) = base_web?;
        let (host, path) = first(b, &['/', '?', '#']).map_or((b, "/"), |i| (&b[..i], &b[i..]));
        let path = if path.starts_with('/') { &path[..first(path, &['?', '#']).unwrap_or(path.len())] } else { "/" };
        let joined = if h.starts_with('/') {
            h.to_string()
        } else if h.starts_with('?') {
            format!("{path}{h}")
        } else {
            format!("{}{h}", &path[..path.rfind('/').map_or(0, |i| i + 1)])
        };
        format!("{sch}://{host}{}", dots(&joined))
    };
    let (_, rest) = web_scheme(&abs)?;
    let host = &rest[..first(rest, &['/', '?', '#']).unwrap_or(rest.len())];
    // A user part makes a link pass for another host (`https://bank.example@evil.example`).
    if host.is_empty() || host.contains(['@', '\\']) || abs.len() > MAX_URL {
        return None;
    }
    // Nothing that would end the Markdown link or the line, split a table's cell (`|`) or pair with a code span's
    // fence (`` ` ``).
    let mut out = String::with_capacity(abs.len());
    let mut i = 0;
    while i < abs.len() {
        let b = abs.as_bytes()[i];
        if b.is_ascii() {
            match b {
                b' ' => out.push_str("%20"),
                b'(' => out.push_str("%28"),
                b')' => out.push_str("%29"),
                b'<' => out.push_str("%3C"),
                b'>' => out.push_str("%3E"),
                b'"' => out.push_str("%22"),
                b'\\' => out.push_str("%5C"),
                b'|' => out.push_str("%7C"),
                b'`' => out.push_str("%60"),
                b if b.is_ascii_control() => {}
                b => out.push(b as char),
            }
            i += 1;
        } else {
            let c = abs[i..].chars().next().expect("a character");
            if !c.is_control() && !bad_char(c) {
                out.push(c);
            }
            i += c.len_utf8();
        }
    }
    Some(out).filter(|o| o.len() <= MAX_URL)
}

/// `x:` before any `/`, `?` or `#`: a scheme of its own.
pub(super) fn has_scheme(h: &str) -> bool {
    let Some(i) = h.find([':', '/', '?', '#']) else { return false };
    let s = &h[..i];
    h[i..].starts_with(':') && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && s.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
}

/// Where the first of `set` is in `s` (each looked for on its own, which is fast on a long address).
pub(super) fn first(s: &str, set: &[char]) -> Option<usize> {
    set.iter().filter_map(|c| s.find(*c)).min()
}

/// A path with its `.` and `..` resolved.
pub(super) fn dots(p: &str) -> String {
    let (path, tail) = first(p, &['?', '#']).map_or((p, ""), |i| (&p[..i], &p[i..]));
    if !path.contains("/.") {
        return p.to_string();
    }
    let mut out: Vec<&str> = vec![];
    for seg in path.split('/').skip(1) {
        match seg {
            "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    let mut s = format!("/{}", out.join("/"));
    if (path.ends_with("/.") || path.ends_with("/..")) && !s.ends_with('/') {
        s.push('/');
    }
    s + tail
}
