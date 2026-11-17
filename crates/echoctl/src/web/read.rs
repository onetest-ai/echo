//! A web page's main text as Markdown: `<article>`, else `<main>`, else `<body>`; scripts, styles, navigation, headers,
//! footers, asides and forms left out; headings, paragraphs, lists, http(s) links, pictures, code and tables kept. Read
//! by a small HTML reader of Echo's own (no crate): what it can't place it keeps as text. What a page that builds itself
//! with JavaScript embeds — JSON-LD, a framework's JSON state, Open Graph, `<noscript>` — is read as data, never run.
//! Pure: no network here, so it reads a page fetched (`jshost::web`) or any HTML a caller has (a rendered DOM too).
//!
//! The page is hostile input: its work is bounded (`MAX_DEPTH`, `MAX_NODES`, JSON caps), its words are never markup in
//! the Markdown (`md_text`), control and direction-changing characters are dropped (`clean`), and a link or picture
//! is kept only as an absolute http(s) address.

use serde_json::{json, Value};

/// Less text than this, on a page with scripts, is a page that needs JavaScript to show itself (`needs_js`).
pub const MIN_TEXT: usize = 200;

/// Deeper elements hold their text but no structure of their own (a page can't overflow the reader).
const MAX_DEPTH: usize = 256;
/// Elements and text pieces read at most: past it the rest of the page is left out (`truncated`).
const MAX_NODES: usize = 250_000;
/// Links and pictures listed at most.
const MAX_LINKS: usize = 2_000;
const MAX_IMAGES: usize = 500;
/// A picture smaller than this on a side (when the page says its size) is a tracking pixel or an icon.
const TINY: u32 = 32;
/// Embedded JSON read at most: one script's text, and the values walked in it.
const MAX_JSON: usize = 2 << 20;
const MAX_JSON_VALUES: usize = 200_000;

// ── reading HTML ──

#[derive(Debug)]
struct El {
    name: String,
    attrs: Vec<(String, String)>,
    kids: Vec<Node>,
}

#[derive(Debug)]
enum Node {
    El(El),
    Text(String),
}

impl El {
    fn new(name: &str, attrs: Vec<(String, String)>) -> El {
        El { name: name.to_string(), attrs, kids: vec![] }
    }

    fn attr(&self, k: &str) -> Option<&str> {
        self.attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
    }

    fn els(&self) -> impl Iterator<Item = &El> {
        self.kids.iter().filter_map(|k| match k {
            Node::El(e) => Some(e),
            Node::Text(_) => None,
        })
    }
}

/// Elements that never have content.
const VOID: [&str; 14] = ["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];
/// Elements whose content is text up to their end tag.
const RAW: [&str; 5] = ["script", "style", "title", "textarea", "noscript"];
/// Left out with all they hold.
const DROP: [&str; 17] = ["script", "style", "nav", "header", "footer", "aside", "form", "noscript", "template", "svg", "head", "title", "iframe", "button", "select", "textarea", "object"];

/// An open `<p>` ends where one of these starts.
fn closes_p(name: &str) -> bool {
    matches!(
        name,
        "address" | "article" | "aside" | "blockquote" | "details" | "div" | "dl" | "fieldset" | "figcaption" | "figure" | "footer" | "form" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            | "header" | "hgroup" | "hr" | "main" | "nav" | "ol" | "p" | "pre" | "section" | "table" | "ul"
    )
}

/// A paragraph of its own.
fn is_block(name: &str) -> bool {
    closes_p(name) || matches!(name, "#doc" | "html" | "body" | "center" | "dd" | "dt" | "dialog" | "summary" | "li" | "tr")
}

struct Tree {
    stack: Vec<El>,
    /// Elements and text pieces made so far (`MAX_NODES`).
    nodes: usize,
}

impl Tree {
    fn push(&mut self, n: Node) {
        self.nodes += 1;
        if let Some(top) = self.stack.last_mut() {
            top.kids.push(n);
        }
    }

    fn text(&mut self, s: &str) {
        if !s.is_empty() {
            self.push(Node::Text(decode(s)));
        }
    }

    fn pop(&mut self) {
        if self.stack.len() > 1 {
            let e = self.stack.pop().expect("an element");
            self.push(Node::El(e));
        }
    }

    fn top(&self) -> &str {
        self.stack.last().map(|e| e.name.as_str()).unwrap_or("")
    }

    /// End an open `name` that sits above the nearest of `scope` (an item ends where the next one begins).
    fn end_open(&mut self, name: &str, scope: &[&str]) {
        for i in (1..self.stack.len()).rev() {
            let n = self.stack[i].name.as_str();
            if n == name {
                while self.stack.len() > i {
                    self.pop();
                }
                return;
            }
            if scope.contains(&n) {
                return;
            }
        }
    }

    fn open(&mut self, name: &str, attrs: Vec<(String, String)>, self_closing: bool) {
        if self.top() == "p" && closes_p(name) {
            self.pop();
        }
        match name {
            "li" => self.end_open("li", &["ul", "ol"]),
            "dt" | "dd" => {
                self.end_open("dt", &["dl"]);
                self.end_open("dd", &["dl"]);
            }
            "tr" => self.end_open("tr", &["table", "thead", "tbody", "tfoot"]),
            "td" | "th" => {
                self.end_open("td", &["tr", "table"]);
                self.end_open("th", &["tr", "table"]);
            }
            "thead" | "tbody" | "tfoot" => {
                for t in ["thead", "tbody", "tfoot"] {
                    self.end_open(t, &["table"]);
                }
            }
            _ => {}
        }
        if VOID.contains(&name) || self_closing {
            self.push(Node::El(El::new(name, attrs)));
        } else if self.stack.len() <= MAX_DEPTH {
            self.stack.push(El::new(name, attrs));
        }
    }

    /// The nearest open element of that name ends (and all opened inside it); an end tag with none open is nothing.
    fn close(&mut self, name: &str) {
        if let Some(i) = self.stack.iter().rposition(|e| e.name == name).filter(|&i| i > 0) {
            while self.stack.len() > i {
                self.pop();
            }
        }
    }
}

/// A tag's name (lowercase) and what follows it.
fn tag_name(s: &str) -> (String, &str) {
    let end = s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == ':' || c == '_')).unwrap_or(s.len());
    (s[..end].to_ascii_lowercase(), &s[end..])
}

/// A tag's attributes, whether it closed itself (`/>`), and what follows the tag.
fn attributes(s: &str) -> (Vec<(String, String)>, bool, &str) {
    let b = s.as_bytes();
    let mut attrs = vec![];
    let mut i = 0;
    loop {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        if i >= b.len() {
            return (attrs, false, "");
        }
        if b[i] == b'>' {
            return (attrs, i > 0 && b[i - 1] == b'/', &s[i + 1..]);
        }
        let start = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && !matches!(b[i], b'=' | b'>' | b'/') {
            i += 1;
        }
        let name = s[start..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                let from = i + 1;
                i = from;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                value = decode(&s[from..i]);
                i = (i + 1).min(b.len());
            } else {
                let from = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                    i += 1;
                }
                value = decode(&s[from..i]);
            }
        } else if name.is_empty() {
            // A stray `=`: step over it.
            i += 1;
        }
        if !name.is_empty() {
            attrs.push((name, value));
        }
    }
}

/// Where the end tag `needle` (`</name`, lowercase) starts in `hay`, in any case.
fn end_tag(hay: &str, needle: &str) -> Option<usize> {
    hay.match_indices("</").map(|(i, _)| i).find(|&i| hay.as_bytes()[i..].get(..needle.len()).is_some_and(|b| b.eq_ignore_ascii_case(needle.as_bytes())))
}

/// What follows the next `>` (nothing when there is none).
fn after_gt(s: &str) -> &str {
    s.find('>').map_or("", |e| &s[e + 1..])
}

/// The page as a tree: tags, text, comments left out; end tags HTML leaves out (`<li>`, `<p>`, cells) implied. True
/// with it when the page held more than `MAX_NODES` and the rest was left out.
fn parse(html: &str) -> (El, bool) {
    let mut t = Tree { stack: vec![El::new("#doc", vec![])], nodes: 0 };
    let mut rest = html;
    let mut truncated = false;
    while !rest.is_empty() {
        if t.nodes >= MAX_NODES {
            truncated = true;
            break;
        }
        let Some(lt) = rest.find('<') else {
            t.text(rest);
            break;
        };
        t.text(&rest[..lt]);
        rest = &rest[lt..];
        if let Some(r) = rest.strip_prefix("<!--") {
            rest = r.find("-->").map_or("", |e| &r[e + 3..]);
            continue;
        }
        match rest[1..].chars().next() {
            Some('!' | '?') => rest = after_gt(rest),
            Some('/') => {
                let (name, after) = tag_name(&rest[2..]);
                if name.is_empty() {
                    t.text("</");
                    rest = &rest[2..];
                    continue;
                }
                t.close(&name);
                rest = after_gt(after);
            }
            Some(c) if c.is_ascii_alphabetic() => {
                let (name, after) = tag_name(&rest[1..]);
                let (attrs, self_closing, after) = attributes(after);
                rest = after;
                if RAW.contains(&name.as_str()) && !self_closing {
                    // Its text up to its end tag, as one element (at any depth: a script never becomes the page's text).
                    // A script's, style's or noscript's text is theirs as written (JSON or HTML is read from it); a
                    // title's is HTML text.
                    let end = end_tag(rest, &format!("</{name}"));
                    let raw = &rest[..end.unwrap_or(rest.len())];
                    let inner = if matches!(name.as_str(), "title" | "textarea") { decode(raw) } else { raw.to_string() };
                    t.push(Node::El(El { name, attrs, kids: vec![Node::Text(inner)] }));
                    rest = end.map_or("", |e| after_gt(&rest[e..]));
                } else {
                    t.open(&name, attrs, self_closing);
                }
            }
            _ => {
                t.text("<");
                rest = &rest[1..];
            }
        }
    }
    while t.stack.len() > 1 {
        t.pop();
    }
    (t.stack.pop().expect("the document"), truncated)
}

/// Text as Echo keeps it: no control characters but line breaks and tabs, and none of the invisible ones that change
/// the direction or join of what follows (a link's text can't pass for another).
pub fn clean(s: &str) -> String {
    s.chars().filter(|&c| !bad_char(c)).collect()
}

fn bad_char(c: char) -> bool {
    (c.is_control() && !matches!(c, '\n' | '\t' | '\r'))
        || matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{feff}' | '\u{061c}')
}

/// HTML's character references: the common names and every number.
fn decode(s: &str) -> String {
    if !s.contains('&') {
        return clean(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest[1..].find(|c: char| c == ';' || c == '&' || c == '<' || c.is_whitespace()).map(|e| e + 1);
        if let Some((c, e)) = end.filter(|&e| e <= 32 && rest.as_bytes()[e] == b';').and_then(|e| entity(&rest[1..e]).map(|c| (c, e))) {
            out.push_str(&c);
            rest = &rest[e + 1..];
            continue;
        }
        out.push('&');
        rest = &rest[1..];
    }
    out.push_str(rest);
    clean(&out)
}

fn entity(name: &str) -> Option<std::borrow::Cow<'static, str>> {
    if let Some(n) = name.strip_prefix('#') {
        let code = match n.strip_prefix(['x', 'X']) {
            Some(h) => u32::from_str_radix(h, 16).ok()?,
            None => n.parse().ok()?,
        };
        return char::from_u32(code).filter(|c| *c != '\0').map(|c| c.to_string().into());
    }
    Some(std::borrow::Cow::Borrowed(match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => " ",
        "mdash" => "—",
        "ndash" => "–",
        "hellip" => "…",
        "lsquo" => "‘",
        "rsquo" => "’",
        "ldquo" => "“",
        "rdquo" => "”",
        "laquo" => "«",
        "raquo" => "»",
        "copy" => "©",
        "reg" => "®",
        "trade" => "™",
        "middot" => "·",
        "bull" => "•",
        "euro" => "€",
        "pound" => "£",
        "times" => "×",
        "deg" => "°",
        "shy" => "",
        _ => return None,
    }))
}

// ── the main text ──

fn dropped(e: &El) -> bool {
    DROP.contains(&e.name.as_str())
}

/// The elements named `name` outside left-out parts, in order.
fn find_all<'a>(e: &'a El, name: &str, out: &mut Vec<&'a El>) {
    for k in e.els().filter(|k| !dropped(k)) {
        if k.name == name {
            out.push(k);
        }
        find_all(k, name, out);
    }
}

/// The text a reader sees, whitespace made single spaces.
fn plain_text(e: &El, out: &mut String) {
    for k in &e.kids {
        match k {
            Node::Text(t) => {
                out.push_str(t);
                out.push(' ');
            }
            Node::El(k) if !dropped(k) => {
                out.push(' ');
                plain_text(k, out);
            }
            Node::El(_) => {}
        }
    }
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text_len(e: &El) -> usize {
    let mut s = String::new();
    plain_text(e, &mut s);
    collapse(&s).chars().count()
}

/// The page's main part: its largest `<article>`, else its `<main>`, else its `<body>`.
fn main_of(doc: &El) -> &El {
    for name in ["article", "main", "body"] {
        let mut found = vec![];
        find_all(doc, name, &mut found);
        let pick = if name == "article" { found.into_iter().max_by_key(|a| text_len(a)) } else { found.into_iter().next() };
        if let Some(e) = pick {
            return e;
        }
    }
    doc
}

/// The first `<h1>` of the main part (its own header included), as text.
fn first_h1(e: &El) -> Option<String> {
    for k in e.els().filter(|k| k.name == "header" || !dropped(k)) {
        if k.name == "h1" {
            let mut s = String::new();
            plain_text(k, &mut s);
            return Some(collapse(&s)).filter(|t| !t.is_empty());
        }
        if let Some(t) = first_h1(k) {
            return Some(t);
        }
    }
    None
}

/// The page's `<title>` (not a drawing's).
fn title_of(e: &El) -> Option<String> {
    for k in e.els().filter(|k| k.name != "svg") {
        if k.name == "title" {
            let t = k.kids.iter().filter_map(|n| if let Node::Text(t) = n { Some(t.as_str()) } else { None }).collect::<String>();
            return Some(collapse(&t)).filter(|t| !t.is_empty());
        }
        if let Some(t) = title_of(k) {
            return Some(t);
        }
    }
    None
}

/// `http` or `https` and what follows `://`, for an address that starts so (in any case).
fn web_scheme(u: &str) -> Option<(&'static str, &str)> {
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
        let (host, path) = b.find(['/', '?', '#']).map_or((b, "/"), |i| (&b[..i], &b[i..]));
        let path = if path.starts_with('/') { path.split(['?', '#']).next().unwrap_or("/") } else { "/" };
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
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    // A user part makes a link pass for another host (`https://bank.example@evil.example`).
    if host.is_empty() || host.contains(['@', '\\']) || abs.chars().count() > 4096 {
        return None;
    }
    // Nothing that would end the Markdown link or the line.
    Some(
        abs.chars()
            .filter(|&c| !c.is_control() && !bad_char(c))
            .map(|c| match c {
                ' ' => "%20".to_string(),
                '(' => "%28".to_string(),
                ')' => "%29".to_string(),
                '<' => "%3C".to_string(),
                '>' => "%3E".to_string(),
                '"' => "%22".to_string(),
                '\\' => "%5C".to_string(),
                c => c.to_string(),
            })
            .collect(),
    )
}

/// `x:` before any `/`, `?` or `#`: a scheme of its own.
fn has_scheme(h: &str) -> bool {
    let Some(i) = h.find([':', '/', '?', '#']) else { return false };
    let s = &h[..i];
    h[i..].starts_with(':') && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && s.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
}

/// A path with its `.` and `..` resolved.
fn dots(p: &str) -> String {
    let (path, tail) = p.find(['?', '#']).map_or((p, ""), |i| (&p[..i], &p[i..]));
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

/// Text as Markdown shows it, never as markup: `\`, `<`, `[`, `]` and `!` escaped, so no HTML, link or image can
/// come of a page's (or anyone's) words.
pub fn md_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '<' | '[' | ']' | '!') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A picture of the page: its absolute address, its words (alt), the caption of the figure it is in, and its size
/// when the page gives it.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub src: String,
    pub alt: String,
    pub caption: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// A number the page wrote for a size (`640`, `640px`): its leading digits.
fn size_attr(e: &El, k: &str) -> Option<u32> {
    let v = e.attr(k)?.trim();
    let digits: String = v.chars().take_while(char::is_ascii_digit).take(6).collect();
    digits.parse().ok()
}

/// The largest candidate of a `srcset` (`a.png 480w, b.png 960w` / `a.png 1x, b.png 2x`), with its weight.
fn best_of_srcset(set: &str) -> Option<(u64, &str)> {
    set.split(',')
        .take(50)
        .filter_map(|c| {
            let mut parts = c.split_whitespace();
            let url = parts.next()?;
            let d = parts.next().unwrap_or("1x");
            let n = |s: &str| s.parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0);
            let weight = if let Some(w) = d.strip_suffix(['w', 'W']) { n(w)? } else { n(d.trim_end_matches(['x', 'X']))? * 1000.0 };
            Some((weight as u64, url))
        })
        .max_by_key(|(w, _)| *w)
}

/// The text of an element as one plain line (no Markdown), for a caption or a link's text in the list.
fn line_of(e: &El) -> String {
    let mut s = String::new();
    plain_text(e, &mut s);
    collapse(&s)
}

/// The links of the main part, each once: `(text, href)`.
fn links_of(e: &El, base: &str, out: &mut Vec<(String, String)>) {
    for k in e.els().filter(|k| !dropped(k)) {
        if out.len() >= MAX_LINKS {
            return;
        }
        if k.name == "a" {
            if let Some(href) = k.attr("href").and_then(|h| web_link(base, h)) {
                if !out.iter().any(|(_, h)| *h == href) {
                    out.push((line_of(k), href));
                }
            }
        }
        links_of(k, base, out);
    }
}

/// The Markdown being written.
struct Out<'a> {
    base: &'a str,
    md: String,
    /// The inline text being gathered; `\n` is a line break.
    line: String,
    /// Lists open around what is written, and the columns an item's lines start at.
    lists: usize,
    indent: usize,
    /// A list item's marker, for its first line.
    marker: Option<String>,
    /// The last block written was a list item (items are one line apart).
    item: bool,
    /// The title's own heading, written once above the text.
    skip_h1: Option<String>,
    /// The pictures written, in order, each once; and the caption of the figure being written.
    images: Vec<Image>,
    caption: Option<String>,
}

impl<'a> Out<'a> {
    fn new(base: &'a str, skip_h1: Option<String>) -> Out<'a> {
        Out { base, md: String::new(), line: String::new(), lists: 0, indent: 0, marker: None, item: false, skip_h1, images: vec![], caption: None }
    }

    fn push_text(&mut self, s: &str) {
        for c in s.chars() {
            if c.is_whitespace() {
                if !self.line.is_empty() && !self.line.ends_with([' ', '\n']) {
                    self.line.push(' ');
                }
            } else {
                self.line.push_str(&md_text(c.encode_utf8(&mut [0; 4])));
            }
        }
    }

    fn write(&mut self, lines: &[String]) {
        if lines.is_empty() {
            return;
        }
        let item = self.lists > 0;
        if !self.md.is_empty() {
            self.md.push_str(if item && self.item { "\n" } else { "\n\n" });
        }
        let pad = " ".repeat(self.indent);
        for (i, l) in lines.iter().enumerate() {
            if i == 0 {
                match self.marker.take() {
                    Some(m) => {
                        self.md.push_str(&" ".repeat(self.indent.saturating_sub(m.chars().count())));
                        self.md.push_str(&m);
                    }
                    None => self.md.push_str(&pad),
                }
            } else {
                self.md.push('\n');
                if !l.is_empty() {
                    self.md.push_str(&pad);
                }
            }
            self.md.push_str(l);
        }
        self.item = item;
    }

    fn flush(&mut self) {
        let raw = std::mem::take(&mut self.line);
        // A line of text that starts as a heading or a quote would read as one.
        let lines: Vec<String> = raw.split('\n').map(str::trim).filter(|l| !l.is_empty()).map(|l| if l.starts_with(['#', '>']) { format!("\\{l}") } else { l.to_owned() }).collect();
        self.write(&lines);
    }

    /// An element's text on one line (as Markdown text), its links and code kept (headings, cells, link text).
    fn inline(&self, e: &El, in_link: bool) -> String {
        let mut s = String::new();
        for k in &e.kids {
            match k {
                Node::Text(t) => s.push_str(&md_text(t)),
                Node::El(k) if dropped(k) || k.name == "img" || k.name == "picture" => {}
                Node::El(k) if k.name == "br" => s.push(' '),
                Node::El(k) if k.name == "a" && !in_link => s.push_str(&self.link(k)),
                Node::El(k) if k.name == "code" => s.push_str(&code_of(k)),
                Node::El(k) => {
                    let block = is_block(&k.name) || matches!(k.name.as_str(), "td" | "th");
                    if block {
                        s.push(' ');
                    }
                    s.push_str(&self.inline(k, in_link));
                    if block {
                        s.push(' ');
                    }
                }
            }
        }
        s
    }

    /// A link as Markdown when it leads to an http(s) page, else its text.
    fn link(&self, a: &El) -> String {
        let text = collapse(&self.inline(a, true));
        match a.attr("href").and_then(|h| web_link(self.base, h)) {
            Some(url) if !text.is_empty() => format!("[{text}]({url})"),
            _ => text,
        }
    }

    /// An `<img>` (or a `<picture>`'s best source with its `<img>`'s words) as a picture: an http(s) address (never
    /// `data:`), not a tracking pixel.
    fn image_of(&self, e: &El) -> Option<Image> {
        let img = if e.name == "img" { Some(e) } else { e.els().find(|k| k.name == "img") };
        let mut sets: Vec<(u64, &str)> = vec![];
        if e.name == "picture" {
            sets.extend(e.els().filter(|k| k.name == "source").filter_map(|s| s.attr("srcset").and_then(best_of_srcset)));
        }
        sets.extend(img.and_then(|i| i.attr("srcset")).and_then(best_of_srcset));
        let largest = sets.into_iter().filter_map(|(w, u)| web_link(self.base, u).map(|u| (w, u))).max_by_key(|(w, _)| *w).map(|(_, u)| u);
        // A lazy page keeps the picture in `data-src` and a placeholder (often `data:`) in `src`.
        let src = largest.or_else(|| ["src", "data-src", "data-original", "data-lazy-src"].iter().find_map(|k| img.and_then(|i| i.attr(k)).and_then(|u| web_link(self.base, u))))?;
        let (width, height) = img.map_or((None, None), |i| (size_attr(i, "width"), size_attr(i, "height")));
        if width.is_some_and(|w| w < TINY) || height.is_some_and(|h| h < TINY) {
            return None;
        }
        let alt = img.and_then(|i| i.attr("alt")).map(collapse).unwrap_or_default();
        Some(Image { src, alt, caption: self.caption.clone(), width, height })
    }

    /// A picture on a line of its own: `![alt](src)`, listed once.
    fn picture(&mut self, img: Image) {
        self.flush();
        self.write(&[format!("![{}]({})", md_text(&img.alt), img.src)]);
        if self.images.len() < MAX_IMAGES && !self.images.iter().any(|i| i.src == img.src) {
            self.images.push(img);
        }
    }

    /// The pictures inside an element (a link around a picture).
    fn pictures_in(&mut self, e: &El) {
        for k in e.els().filter(|k| !dropped(k)) {
            if k.name == "img" || k.name == "picture" {
                if let Some(i) = self.image_of(k) {
                    self.picture(i);
                }
            } else {
                self.pictures_in(k);
            }
        }
    }

    fn walk(&mut self, n: &Node) {
        let e = match n {
            Node::Text(t) => return self.push_text(t),
            Node::El(e) => e,
        };
        if dropped(e) {
            return;
        }
        match e.name.as_str() {
            "br" => self.line.push('\n'),
            "img" | "picture" => match self.image_of(e) {
                Some(i) => self.picture(i),
                // A picture with no picture of its own: whatever it holds is read as text.
                None => e.kids.iter().for_each(|k| self.walk(k)),
            },
            "a" => {
                self.pictures_in(e);
                let l = self.link(e);
                self.line.push_str(&l);
            }
            "code" => self.line.push_str(&code_of(e)),
            "figure" => {
                // The picture first, its caption under it as text.
                self.flush();
                let caption = e.els().find(|k| k.name == "figcaption").map(line_of).filter(|c| !c.is_empty());
                let outer = std::mem::replace(&mut self.caption, caption);
                e.kids.iter().filter(|k| !matches!(k, Node::El(k) if k.name == "figcaption")).for_each(|k| self.walk(k));
                self.caption = outer;
                self.flush();
                for c in e.els().filter(|k| k.name == "figcaption") {
                    c.kids.iter().for_each(|k| self.walk(k));
                    self.flush();
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.flush();
                let text = collapse(&self.inline(e, false));
                if e.name == "h1" && self.skip_h1.as_deref().is_some_and(|t| md_text(t) == text) {
                    self.skip_h1 = None;
                    return;
                }
                if !text.is_empty() {
                    let level = e.name[1..].parse::<usize>().unwrap_or(1);
                    let lists = std::mem::take(&mut self.lists);
                    self.write(&[format!("{} {text}", "#".repeat(level))]);
                    self.lists = lists;
                }
            }
            "ul" | "ol" => {
                self.flush();
                self.lists += 1;
                let mut n: usize = if e.name == "ol" { e.attr("start").and_then(|s| s.trim().parse().ok()).unwrap_or(1) } else { 0 };
                for k in &e.kids {
                    match k {
                        Node::El(li) if li.name == "li" => {
                            let marker = if e.name == "ol" { format!("{n}. ") } else { "- ".to_string() };
                            n = n.saturating_add(1);
                            self.item(li, marker);
                        }
                        other => self.walk(other),
                    }
                }
                self.flush();
                self.lists -= 1;
            }
            "li" => {
                self.lists += 1;
                self.item(e, "- ".into());
                self.lists -= 1;
            }
            "pre" => {
                self.flush();
                let mut code = String::new();
                raw_text(e, &mut code);
                let code = code.strip_prefix('\n').unwrap_or(&code).trim_end().to_string();
                if !code.is_empty() {
                    let run = code.split(|c| c != '`').map(str::len).max().unwrap_or(0);
                    let fence = "`".repeat(run.max(2) + 1);
                    let mut lines = vec![fence.clone()];
                    lines.extend(code.lines().map(str::to_owned));
                    lines.push(fence);
                    self.write(&lines);
                }
            }
            "table" => {
                self.flush();
                let mut rows = vec![];
                self.rows(e, &mut rows);
                let n = rows.iter().map(Vec::len).max().unwrap_or(0);
                if n > 0 {
                    let line = |r: &[String]| format!("| {} |", (0..n).map(|i| r.get(i).map(String::as_str).unwrap_or("")).collect::<Vec<_>>().join(" | "));
                    let mut lines = vec![line(&rows[0]), format!("| {} |", vec!["---"; n].join(" | "))];
                    lines.extend(rows[1..].iter().map(|r| line(r)));
                    self.write(&lines);
                }
            }
            name if is_block(name) => {
                self.flush();
                e.kids.iter().for_each(|k| self.walk(k));
                self.flush();
            }
            _ => e.kids.iter().for_each(|k| self.walk(k)),
        }
    }

    fn item(&mut self, li: &El, marker: String) {
        self.flush();
        let saved = self.indent;
        self.indent += marker.chars().count();
        self.marker = Some(marker);
        li.kids.iter().for_each(|k| self.walk(k));
        self.flush();
        self.marker = None;
        self.indent = saved;
    }

    /// A table's rows of cells (a table inside a cell is that cell's text).
    fn rows(&self, e: &El, out: &mut Vec<Vec<String>>) {
        for k in e.els().filter(|k| !dropped(k)) {
            match k.name.as_str() {
                "tr" => out.push(k.els().filter(|c| c.name == "td" || c.name == "th").map(|c| collapse(&self.inline(c, false)).replace('|', "\\|")).collect()),
                "table" => {}
                _ => self.rows(k, out),
            }
        }
    }
}

/// Inline code: its text as written, on one line, in backticks.
fn code_of(e: &El) -> String {
    let mut s = String::new();
    raw_text(e, &mut s);
    code_span(&collapse(&s))
}

fn code_span(text: &str) -> String {
    if text.is_empty() {
        String::new()
    } else if text.contains('`') {
        format!("`` {text} ``")
    } else {
        format!("`{text}`")
    }
}

/// A `<pre>`'s text as written.
fn raw_text(e: &El, out: &mut String) {
    for k in &e.kids {
        match k {
            Node::Text(t) => out.push_str(t),
            Node::El(k) if k.name == "br" => out.push('\n'),
            Node::El(k) if !dropped(k) => raw_text(k, out),
            Node::El(_) => {}
        }
    }
}

// ── what a page embeds: meta, JSON-LD, a framework's state, <noscript> ──

/// Every element named `name`, left-out parts too (`<head>`'s meta, scripts anywhere), in order, up to `most`.
fn every<'a>(e: &'a El, name: &str, most: usize, out: &mut Vec<&'a El>) {
    for k in e.els() {
        if out.len() >= most {
            return;
        }
        if k.name == name {
            out.push(k);
        }
        every(k, name, most, out);
    }
}

/// A raw element's own text (a script's JSON, a noscript's HTML).
fn own_text(e: &El) -> &str {
    e.kids.iter().find_map(|n| if let Node::Text(t) = n { Some(t.as_str()) } else { None }).unwrap_or("")
}

/// What JSON-LD says of the page's article.
#[derive(Default)]
struct Ld {
    title: Option<String>,
    body: Option<String>,
    author: Option<String>,
    published: Option<String>,
    description: Option<String>,
    images: Vec<String>,
}

/// A name or names (`"Ann"`, `{"name": "Ann"}`, a list of them), joined.
fn names(v: &Value) -> Option<String> {
    let one = |v: &Value| v.as_str().map(str::to_owned).or_else(|| v["name"].as_str().map(str::to_owned));
    let all: Vec<String> = match v {
        Value::Array(a) => a.iter().take(20).filter_map(one).collect(),
        v => one(v).into_iter().collect(),
    };
    Some(all.join(", ")).filter(|s| !s.trim().is_empty())
}

/// Picture addresses (`"…"`, `{"url": "…"}`, a list of them).
fn urls(v: &Value) -> Vec<String> {
    let one = |v: &Value| v.as_str().map(str::to_owned).or_else(|| v["url"].as_str().map(str::to_owned));
    match v {
        Value::Array(a) => a.iter().take(20).filter_map(one).collect(),
        v => one(v).into_iter().collect(),
    }
}

/// The article in a JSON-LD value: an object of an article's type, at the top, in a list or in `@graph`.
fn ld_article(v: &Value, ld: &mut Ld, budget: &mut usize) {
    if *budget == 0 {
        return;
    }
    *budget -= 1;
    match v {
        Value::Array(a) => a.iter().take(200).for_each(|x| ld_article(x, ld, budget)),
        Value::Object(o) => {
            if let Some(g) = o.get("@graph") {
                ld_article(g, ld, budget);
            }
            let types: Vec<&str> = match o.get("@type") {
                Some(Value::String(t)) => vec![t.as_str()],
                Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
                _ => vec![],
            };
            let article = types.iter().any(|t| t.ends_with("Article") || matches!(*t, "BlogPosting" | "Report" | "WebPage" | "Blog" | "HowTo" | "Recipe"));
            if !article {
                return;
            }
            let s = |k: &str| o.get(k).and_then(Value::as_str).map(str::to_owned).filter(|x| !x.trim().is_empty());
            ld.title = ld.title.take().or_else(|| s("headline")).or_else(|| s("name"));
            // The fullest body of the page's articles.
            if let Some(b) = s("articleBody").or_else(|| s("text")) {
                if ld.body.as_ref().is_none_or(|old| b.len() > old.len()) {
                    ld.body = Some(b);
                }
            }
            ld.author = ld.author.take().or_else(|| o.get("author").and_then(names));
            ld.published = ld.published.take().or_else(|| s("datePublished"));
            ld.description = ld.description.take().or_else(|| s("description"));
            if let Some(i) = o.get("image") {
                ld.images.extend(urls(i));
            }
            if let Some(m) = o.get("mainEntity") {
                ld_article(m, ld, budget);
            }
        }
        _ => {}
    }
}

/// Text a framework's JSON state carries that reads as prose: long strings of words (HTML in them read as HTML), in
/// order, each once, up to 1 MB.
fn prose(v: &Value, out: &mut Vec<String>, budget: &mut usize, size: &mut usize) {
    if *budget == 0 || *size > 1 << 20 {
        return;
    }
    *budget -= 1;
    match v {
        Value::String(s) => {
            let words = s.split_whitespace().count();
            let looks_like_text = s.len() >= 80 && words >= 10 && !s.trim_start().starts_with(['{', '[']) && web_scheme(s.trim()).is_none();
            if looks_like_text && !out.iter().any(|o| o == s) {
                *size += s.len();
                out.push(s.clone());
            }
        }
        Value::Array(a) => a.iter().for_each(|x| prose(x, out, budget, size)),
        Value::Object(o) => o.values().for_each(|x| prose(x, out, budget, size)),
        _ => {}
    }
}

/// JSON a page embeds, read with caps: too long, too deep (serde's own limit) or not JSON is nothing.
fn json_of(text: &str) -> Option<Value> {
    if text.len() > MAX_JSON {
        return None;
    }
    serde_json::from_str(text.trim()).ok()
}

/// HTML (a piece of a page, a noscript's, a CMS body in JSON) as Markdown, with the pictures it shows.
fn html_md(html: &str, base: &str) -> (String, Vec<Image>) {
    let (doc, _) = parse(html);
    let mut out = Out::new(base, None);
    doc.kids.iter().for_each(|k| out.walk(k));
    out.flush();
    (out.md.trim().to_string(), out.images)
}

/// Plain text (paragraphs on their lines) as Markdown text.
fn text_md(text: &str, base: &str) -> String {
    let mut out = Out::new(base, None);
    for para in text.split('\n') {
        out.push_text(para);
        out.flush();
    }
    out.md.trim().to_string()
}

/// A string that may hold HTML as Markdown.
fn any_md(s: &str, base: &str) -> String {
    if s.contains("</") || s.contains("<p>") || s.contains("<br") {
        html_md(s, base).0
    } else {
        text_md(s, base)
    }
}

// ── the page ──

/// A page read: its main text as Markdown and what it says of itself. Its words are the page's: untrusted.
#[derive(Debug, Default)]
pub struct Page {
    pub title: String,
    pub markdown: String,
    /// The main part's http(s) links, each once: `(text, href)`.
    pub links: Vec<(String, String)>,
    pub images: Vec<Image>,
    pub lang: Option<String>,
    pub byline: Option<String>,
    pub published: Option<String>,
    pub description: Option<String>,
    /// Where the text came from: `html`, `json-ld`, `state` (a framework's JSON), `noscript` or `meta`.
    pub source: &'static str,
    /// Little readable text on a page with scripts: it shows itself only by running them.
    pub needs_js: bool,
    /// The page was too big to read whole (`MAX_NODES`).
    pub truncated: bool,
}

impl Page {
    pub fn to_json(&self) -> Value {
        json!({
            "title": self.title,
            "markdown": self.markdown,
            "links": self.links.iter().map(|(text, href)| json!({"text": text, "href": href})).collect::<Vec<_>>(),
            "images": self.images.iter().map(|i| json!({"src": i.src, "alt": i.alt, "caption": i.caption, "width": i.width, "height": i.height})).collect::<Vec<_>>(),
            "lang": self.lang,
            "byline": self.byline,
            "published": self.published,
            "description": self.description,
            "source": self.source,
            "needs_js": self.needs_js,
            "truncated": self.truncated,
        })
    }
}

/// One line of the page's words, cut to `most` characters (`None` when empty).
fn short(s: &str, most: usize) -> Option<String> {
    Some(collapse(&clean(s)).chars().take(most).collect::<String>()).filter(|s| !s.is_empty())
}

/// A page's HTML read: the main part (`<article>`, else `<main>`, else `<body>`) as Markdown, or — when what the page
/// embeds holds clearly more text (a page that builds itself with JavaScript) — that text; its title (the main part's
/// first heading, else JSON-LD's or Open Graph's, else `<title>`; not repeated in the Markdown), links, pictures and
/// what it says of itself. `base` is the page's own address, for its relative links ("" for none: relative links are
/// then text). Nothing of the page is run or fetched.
pub fn read(html: &str, base: &str) -> Page {
    let (doc, truncated) = parse(html);
    let root = main_of(&doc);

    let mut metas = vec![];
    every(&doc, "meta", 500, &mut metas);
    let meta = |keys: &[&str]| {
        keys.iter().find_map(|k| metas.iter().find(|m| [m.attr("property"), m.attr("name"), m.attr("itemprop")].into_iter().flatten().any(|n| n.eq_ignore_ascii_case(k))).and_then(|m| m.attr("content")).and_then(|c| short(c, 2000)))
    };
    let mut scripts = vec![];
    every(&doc, "script", 2000, &mut scripts);
    let mut ld = Ld::default();
    let mut state = vec![];
    let (mut budget, mut size) = (MAX_JSON_VALUES, 0);
    for s in &scripts {
        let kind = s.attr("type").unwrap_or("").to_ascii_lowercase();
        let id = s.attr("id").unwrap_or("");
        if kind.contains("ld+json") {
            if let Some(v) = json_of(own_text(s)) {
                ld_article(&v, &mut ld, &mut budget);
            }
        } else if kind == "application/json" || matches!(id, "__NEXT_DATA__" | "__NUXT_DATA__" | "__APOLLO_STATE__") {
            if let Some(v) = json_of(own_text(s)) {
                prose(&v, &mut state, &mut budget, &mut size);
            }
        }
    }

    let og_title = meta(&["og:title", "twitter:title"]);
    let title = first_h1(root).or_else(|| ld.title.as_deref().and_then(|t| short(t, 500))).or(og_title).or_else(|| title_of(&doc)).and_then(|t| short(&t, 500)).unwrap_or_default();

    let mut out = Out::new(base, Some(title.clone()).filter(|t| !t.is_empty()));
    root.kids.iter().for_each(|k| out.walk(k));
    out.flush();
    let mut images = std::mem::take(&mut out.images);
    let html_text = out.md.trim().to_string();

    // What the page embeds, as Markdown; the fuller text wins, the page's own HTML on a near tie.
    let len = |md: &str| collapse(md).chars().count();
    let mut best = (len(&html_text), html_text, "html");
    let mut offer = |md: String, from: &'static str| {
        let n = len(&md);
        if n > best.0 + best.0 / 4 + 20 {
            best = (n, md, from);
        }
    };
    if let Some(b) = &ld.body {
        offer(any_md(b, base), "json-ld");
    }
    if !state.is_empty() {
        offer(state.iter().map(|s| any_md(s, base)).filter(|m| !m.is_empty()).collect::<Vec<_>>().join("\n\n"), "state");
    }
    let mut noscripts = vec![];
    every(&doc, "noscript", 50, &mut noscripts);
    let noscript: Vec<String> = noscripts.iter().map(|n| html_md(own_text(n), base).0).filter(|m| !m.is_empty()).collect();
    if !noscript.is_empty() {
        offer(noscript.join("\n\n"), "noscript");
    }
    let description = meta(&["og:description", "description", "twitter:description"]).or_else(|| ld.description.as_deref().and_then(|d| short(d, 2000)));
    let (n, markdown, source) = best;
    let (markdown, source) = match (&description, n) {
        (Some(d), 0) => (text_md(d, base), "meta"),
        _ => (markdown, source),
    };

    // Pictures the page names for itself (Open Graph, JSON-LD) after those of its text.
    for src in meta(&["og:image", "og:image:url", "twitter:image"]).into_iter().chain(ld.images.iter().cloned()) {
        if let Some(src) = web_link(base, &src) {
            if images.len() < MAX_IMAGES && !images.iter().any(|i| i.src == src) {
                images.push(Image { src, alt: String::new(), caption: None, width: None, height: None });
            }
        }
    }
    let mut links = vec![];
    links_of(root, base, &mut links);
    let html_el = doc.els().find(|e| e.name == "html");
    let lang = html_el.and_then(|h| h.attr("lang")).map(str::trim).filter(|l| !l.is_empty() && l.len() <= 35 && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')).map(str::to_owned);
    let byline = meta(&["author", "article:author"]).filter(|a| web_scheme(a).is_none()).or_else(|| ld.author.as_deref().and_then(|a| short(a, 300)));
    let mut times = vec![];
    every(root, "time", 1, &mut times);
    let published = meta(&["article:published_time", "datePublished"]).or_else(|| ld.published.as_deref().and_then(|p| short(p, 64))).or_else(|| times.first().and_then(|t| t.attr("datetime")).and_then(|d| short(d, 64)));
    let needs_js = len(&markdown) < MIN_TEXT && !scripts.is_empty();
    Page { title, markdown, links, images, lang, byline, published: published.map(|p| p.chars().take(64).collect()), description, source, needs_js, truncated }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pages made up for these tests (no real site's text).
    fn page(name: &str) -> &'static str {
        match name {
            "article" => include_str!("testdata/article.html"),
            "chrome" => include_str!("testdata/chrome.html"),
            "tables" => include_str!("testdata/tables.html"),
            "links" => include_str!("testdata/links.html"),
            "short" => include_str!("testdata/short.html"),
            "code" => include_str!("testdata/code.html"),
            "nested" => include_str!("testdata/nested.html"),
            "unicode" => include_str!("testdata/unicode.html"),
            "two-articles" => include_str!("testdata/two-articles.html"),
            "body-only" => include_str!("testdata/body-only.html"),
            "figures" => include_str!("testdata/figures.html"),
            "jsonld" => include_str!("testdata/jsonld.html"),
            "next" => include_str!("testdata/next.html"),
            "og-only" => include_str!("testdata/og-only.html"),
            "noscript" => include_str!("testdata/noscript.html"),
            "spa" => include_str!("testdata/spa.html"),
            "hostile" => include_str!("testdata/hostile.html"),
            other => panic!("no page {other}"),
        }
    }

    const BASE: &str = "https://contoso.example/notes/page";

    fn md(name: &str) -> (String, String) {
        let p = read(page(name), BASE);
        (p.title, p.markdown)
    }

    #[test]
    fn an_article_keeps_its_main_text_only() {
        let (title, text) = md("article");
        assert_eq!(title, "How Northwind Retail moved its pilot");
        assert_eq!(
            text,
            "Northwind Retail planned to start its checkout pilot in October. After two weeks of load tests the team chose to move the start to 18 November and keep the pricing page as it is.\n\n\
             ## What changed\n\n\
             - The pilot starts in three stores instead of ten.\n\
             - Lena Hartmann owns the data migration.\n\n\
             The full plan is in the [pilot plan](https://contoso.example/plans/pilot-2026); questions go to the pilot desk."
        );
        let p = read(page("article"), BASE);
        assert_eq!(p.links, vec![("pilot plan".to_string(), "https://contoso.example/plans/pilot-2026".to_string())]);
        assert_eq!((p.source, p.needs_js, p.truncated), ("html", false, false));
    }

    #[test]
    fn navigation_scripts_and_forms_are_dropped() {
        let (title, text) = md("chrome");
        assert_eq!(title, "Pricing model notes");
        assert!(text.contains("Per-seat pricing stays for teams under fifty people.") && text.contains("Discounts need a sign-off from Omar Reyes"), "{text}");
        for gone in ["tracking", "Injected", "Home", "Careers", "Subscribe", "Popular", "spreadsheets", "Footer", "Sign up", "Email", "JavaScript", "font-family", "Fabrikam Insights"] {
            assert!(!text.contains(gone), "{gone} in {text}");
        }
    }

    #[test]
    fn tables_become_markdown_tables() {
        let (_, text) = md("tables");
        assert!(text.contains(
            "| Plan | Stores | Price |\n| --- | --- | --- |\n| Starter | 1–5 | €40 |\n| Growth | 6–50 | €32 \\| €28 with a two-year term |\n| Enterprise | 51+ | [Ask sales](https://contoso.example/sales) |"
        ), "{text}");
        assert!(text.contains("## Add-ons\n\n| Analytics | €5 |\n| --- | --- |\n| Priority support | €9 |"), "rows without a head, cells left open: {text}");
    }

    #[test]
    fn only_http_and_https_links_stay_links() {
        let p = read(page("links"), "https://contoso.example/notes/links");
        let text = p.markdown;
        assert!(text.contains("Run the demo is a script link"), "{text}");
        assert!(!text.contains("javascript") && !text.contains("alert"), "a script link is text: {text}");
        assert!(text.contains("[The plain report](http://plain.example/report) uses plain http"), "{text}");
        assert!(text.contains("[The secure report](https://secure.example/report?id=7)"), "{text}");
        assert!(text.contains("[The guide](https://cdn.contoso.example/guide.pdf)"), "{text}");
        assert!(text.contains("[Omar's page](https://contoso.example/team/omar-reyes)"), "{text}");
        assert!(text.contains("back to top stays here") && text.contains("A data link and a mail link are text"), "{text}");
        assert!(!text.contains("mailto") && !text.contains("data:"), "{text}");
        assert!(text.contains("[An odd address](https://evil.example/a%20b%28c%29)"), "{text}");
        assert_eq!(text.matches("](").count(), 5, "five http(s) links: {text}");
        assert_eq!(p.links.len(), 5);
        // A relative link on a page with an http address follows its scheme; on a page with none it is text.
        assert_eq!(web_link("http://lab.local:8080/a/b", "../c?x=1"), Some("http://lab.local:8080/c?x=1".into()));
        assert_eq!(web_link("", "/c"), None);
        assert_eq!(web_link("", "https://acme.example/c"), Some("https://acme.example/c".into()));
    }

    #[test]
    fn a_short_page_gives_its_little_text() {
        let p = read(page("short"), "https://contoso.example/sign-in");
        assert_eq!(p.title, "Sign in");
        assert!(p.markdown.starts_with("You need an account to read this page."), "{}", p.markdown);
        assert!(!p.needs_js, "no scripts: it is just short");
    }

    #[test]
    fn code_keeps_its_lines() {
        let (_, text) = md("code");
        assert!(text.contains("The `--since` flag limits it"), "{text}");
        assert!(text.contains("```\ncontoso export stores --since 2026-11-01 > stores.csv\nif [ -s stores.csv ]; then\n    echo \"done: $(wc -l < stores.csv) stores\"\nfi\n```"), "{text}");
        assert!(text.contains("````\n```yaml\nregion: north\n```\n````"), "a longer fence around backticks: {text}");
    }

    #[test]
    fn unclosed_items_still_make_lists() {
        let (_, text) = md("nested");
        assert_eq!(
            text,
            "Each store goes through the same steps before the pilot starts, in this order, signed off by the store manager.\n\n\
             1. Hardware\n   - Scanners tested\n   - Receipt printers on the new firmware\n\
             2. Staff training\n\
             3. Go-live call with [support](https://northwind.example/support)\n\n\
             Stores that miss a step move to the next wave."
        );
    }

    #[test]
    fn entities_and_other_scripts_read_as_written() {
        let (title, text) = md("unicode");
        assert_eq!(title, "Отчёт о пилоте — Север");
        assert!(text.contains("сократилось на треть."), "{text}");
        assert!(text.contains("Team notes—in English too: the rollout’s second wave starts in December & covers 12 stores 🚀."), "{text}");
        assert!(text.contains("Quotes stay as written: “fix forward, no rollback” … © Northwind \\<internal>."), "{text}");
    }

    #[test]
    fn the_largest_article_is_the_page() {
        let (title, text) = md("two-articles");
        assert_eq!(title, "Weekly digest");
        assert!(text.contains("finished the migration of the order service") && text.contains("plans the December release"), "{text}");
        assert!(!text.contains("Five tips") && !text.contains("teaser"), "{text}");
    }

    #[test]
    fn a_page_without_article_or_main_reads_its_body() {
        let (title, text) = md("body-only");
        assert_eq!(title, "Meeting room guide");
        assert_eq!(
            text,
            "## Booking\n\n\
             Rooms on the third floor are booked through the front desk; the large room needs a day's notice.\n\
             Cancel by noon the day before, or the booking is charged to your team.\n\n\
             ### Equipment\n\n\
             Every room has a screen and a speakerphone. The large room also has a whiteboard wall and a document camera.\n\n\
             Questions: ask Priya at the front desk."
        );
    }

    #[test]
    fn pictures_stay_in_place_with_their_captions_and_tracking_pixels_go() {
        let p = read(page("figures"), "https://acme.example/rollout/diagrams");
        let text = &p.markdown;
        assert_eq!(p.title, "Rollout diagrams");
        assert!(text.contains("![Three waves of stores](https://acme.example/img/waves.png)\n\nFigure 1. Stores by wave"), "the caption under its picture: {text}");
        assert!(text.contains("![Data flow](https://acme.example/img/flow-1440.webp)"), "a picture's largest source: {text}");
        assert!(text.contains("![Store map](https://cdn.acme.example/lazy/map.jpg)"), "a lazy picture's own address: {text}");
        assert!(text.contains("![Rack layout](https://acme.example/img/thumb.png)\n\nClick for the full rack layout."), "a picture in a link: {text}");
        assert!(!text.contains("pixel.acme.example") && !text.contains("data:") && !text.contains("![dot]"), "{text}");
        let srcs: Vec<&str> = p.images.iter().map(|i| i.src.as_str()).collect();
        assert_eq!(
            srcs,
            ["https://acme.example/img/waves.png", "https://acme.example/img/flow-1440.webp", "https://cdn.acme.example/lazy/map.jpg", "https://acme.example/img/thumb.png"],
            "each picture once, in order"
        );
        assert_eq!(p.images[0], Image { src: srcs[0].into(), alt: "Three waves of stores".into(), caption: Some("Figure 1. Stores by wave".into()), width: Some(960), height: Some(540) });
        assert_eq!(p.images[1].caption, None);
        assert_eq!((p.lang.as_deref(), p.byline.as_deref(), p.published.as_deref()), (Some("en"), Some("Lena Hartmann"), Some("2026-11-02T09:00:00Z")));
    }

    #[test]
    fn a_json_ld_article_gives_the_text_a_script_would_show() {
        let p = read(page("jsonld"), "https://acme.example/blog/pilot");
        assert_eq!(p.title, "Why Acme moved its pilot to November");
        assert_eq!(p.source, "json-ld");
        assert!(p.markdown.starts_with("Acme planned to start the checkout pilot in October.\n\nAfter two weeks"), "{}", p.markdown);
        assert!(p.markdown.contains("\\[not a link\\](javascript:alert(1))"), "its words are never markup: {}", p.markdown);
        assert_eq!(p.byline.as_deref(), Some("Omar Reyes, Lena Hartmann"));
        assert_eq!(p.published.as_deref(), Some("2026-10-28"));
        assert_eq!(p.images.iter().map(|i| i.src.as_str()).collect::<Vec<_>>(), ["https://acme.example/img/pilot.jpg"]);
        assert!(!p.needs_js);
    }

    #[test]
    fn a_frameworks_json_state_gives_its_prose() {
        let p = read(page("next"), "https://globex.example/handbook/onboarding");
        assert_eq!(p.source, "state");
        assert_eq!(p.title, "Globex handbook");
        assert!(p.markdown.starts_with("## First week\n\nEvery new starter at Globex spends the first week"), "its HTML read as HTML: {}", p.markdown);
        assert!(p.markdown.contains("write them down in the shared onboarding plan."), "{}", p.markdown);
        assert!(!p.markdown.contains("cdn.globex") && !p.markdown.contains("buildId"), "an address isn't prose: {}", p.markdown);
        assert!(!p.needs_js);
    }

    #[test]
    fn open_graph_alone_gives_a_title_a_description_and_a_picture() {
        let p = read(page("og-only"), "https://status.initech.example/");
        assert_eq!(p.title, "Initech status: all systems normal");
        assert_eq!(p.source, "meta");
        assert_eq!(p.markdown, "All Initech services run normally; the last incident closed on 3 November.");
        assert_eq!(p.description.as_deref(), Some("All Initech services run normally; the last incident closed on 3 November."));
        assert_eq!(p.images[0].src, "https://status.initech.example/og/status.png");
        assert!(p.needs_js, "a description alone: the page shows itself with scripts");
    }

    #[test]
    fn a_noscript_page_is_read() {
        let p = read(page("noscript"), "https://acme.example/releases");
        assert_eq!(p.source, "noscript");
        assert!(p.markdown.contains("Version 4.2 adds store groups") && p.markdown.contains("left out the last day of the week"), "{}", p.markdown);
        assert!(!p.needs_js);
    }

    #[test]
    fn an_empty_app_page_says_it_needs_javascript() {
        let p = read(page("spa"), "https://app.acme.example/");
        assert!(p.needs_js);
        assert_eq!(p.title, "Acme app");
        assert!(p.markdown.chars().count() < MIN_TEXT, "{}", p.markdown);
    }

    #[test]
    fn every_trick_of_a_hostile_page_comes_out_as_plain_words() {
        let p = read(page("hostile"), "https://acme.example/notes");
        let text = &p.markdown;
        assert_eq!(p.title, "Hostile notes");
        for gone in ["JaVaScRiPt", "vbscript", "file:", "data:", "document.", "onerror=\"alert", "alert(11)", "onclick", "onload", "onmouseover", "refresh", "template text", "svg link", "Send", "background", "frame", ".swf", "steal"] {
            assert!(!text.contains(gone), "{gone} in {text}");
        }
        // Every link and picture is Echo's own `](` to an address of the page's site; any other is escaped text.
        for (i, _) in text.match_indices("](") {
            assert!(text[..i].ends_with('\\') || text[i + 2..].starts_with("https://acme.example/"), "a link out of {:?}", &text[i.saturating_sub(40)..]);
        }
        assert!(text.contains("script link, mixed case, tabbed, data link, file link, vb link, a bank, backslashes."), "links that aren't http(s) are their words: {text}");
        assert!(text.contains("Raw \\<img src=x onerror=alert(7)> and \\[x\\](javascript:alert(8)) and \\!\\[i\\](https://evil.example/t.png) are text"), "{text}");
        assert!(text.contains("Direction txt.exe and zerowidth and a NUL &#0; and bell are dropped."), "{text:?}");
        assert!(text.contains("[a fine link](https://acme.example/ok)"), "{text}");
        assert_eq!(p.links, vec![("a fine link".to_string(), "https://acme.example/ok".to_string())]);
        assert!(p.images.iter().all(|i| i.src.starts_with("https://acme.example/")), "{:?}", p.images);
        assert!(!p.title.contains('\u{202e}') && p.images.iter().all(|i| !i.alt.contains('\u{202e}')));
        for c in text.chars() {
            assert!(!bad_char(c), "{c:?} in {text:?}");
        }
    }

    #[test]
    fn deep_nesting_is_read_without_overflow() {
        // 10,000 levels of every kind of element, read on a thread with a worker's usual stack.
        for (open, close) in [("<div><span>", "</span></div>"), ("<ul><li>", "</li></ul>"), ("<table><tr><td>", "</td></tr></table>"), ("<a href=/x>", "</a>"), ("<figure>", "</figure>"), ("<blockquote>", "</blockquote>"), ("<picture>", "</picture>"), ("<article>", "</article>")] {
            let mut html = String::from("<html><body><main><p>");
            html.push_str(&open.repeat(10_000));
            html.push_str(&"Deeply nested text that still counts as the page's words. ".repeat(5));
            html.push_str(&close.repeat(10_000));
            html.push_str("</main></body></html>");
            let text = std::thread::Builder::new().stack_size(2 << 20).spawn(move || read(&html, "https://contoso.example/").markdown).unwrap().join().expect("no overflow");
            assert!(text.contains("Deeply nested text"), "{open}: {text}");
        }
    }

    #[test]
    fn a_huge_page_is_read_in_bounded_work() {
        let html = format!("<html><body><main>{}</main></body></html>", "<i>x</i>".repeat(MAX_NODES));
        let at = std::time::Instant::now();
        let p = read(&html, "https://contoso.example/");
        assert!(p.truncated);
        assert!(at.elapsed() < std::time::Duration::from_secs(20), "{:?}", at.elapsed());
        // JSON nested past serde's limit, or over the size cap, is nothing — never a crash.
        let deep = format!("<script type=\"application/ld+json\">{}{}</script><p>Hi</p>", "[".repeat(100_000), "]".repeat(100_000));
        assert_eq!(read(&deep, "").markdown, "Hi");
    }

    #[test]
    fn malformed_html_never_panics() {
        // Made-up HTML of broken pieces, cut anywhere (a fixed seed: the same every run).
        let bits = ["<", ">", "</", "<p", "<a href=\"", "\"", "'", "=", "<!--", "-->", "<script>", "</script", "<table><tr><td>", "<li>", "<ol start=x>", "<img src=", "srcset=\", 2x,,", "&#x110000;", "&#xD800;", "&#;", "&", "&amp", "<pre>", "`", "\u{202e}", "\u{0}", "é", "😀", " ", "\n", "<figure>", "<figcaption>", "<noscript>", "<svg>", "</svg>", "<meta property=og:title content=", "<script type=application/ld+json>{\"@type\":\"Article\",\"articleBody\":", "<h1>", "<br/>", "<picture><source srcset=\"a 9999999999999999999999w\">"];
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..40 {
            let mut html = String::new();
            for _ in 0..4_000 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                html.push_str(bits[(seed >> 33) as usize % bits.len()]);
            }
            let _ = read(&html, "https://contoso.example/a/b").to_json();
            let _ = read(&html, "").to_json();
        }
    }

    #[test]
    fn page_text_is_never_markup() {
        // A page's words can't become HTML, a link or an image in the Markdown.
        let filler = "Filler text so that the page counts as readable for the extractor. ".repeat(4);
        let html = format!(
            "<html><body><article><h1>Release notes</h1><p>{filler}</p>\
             <p>Literal [x](javascript:alert(1)) and ![](http://tracker.example/p.png) stay text, as does &lt;img src=http://tracker.example/q.png&gt;.</p>\
             <p># not a heading</p><p>&gt; not a quote</p><p>Wow!<a href=\"https://contoso.example/pic.png\">pic</a> and a back\\slash.</p>\
             <h2>Tags &lt;i&gt; and [brackets]</h2><table><tr><td>&lt;td&gt; [c]</td><td><code>a&lt;b</code></td></tr></table>\
             <img src=\"/p.png\" alt=\"](javascript:alert(1)) <b>\"></article></body></html>"
        );
        let text = read(&html, "https://contoso.example/").markdown;
        assert!(text.contains("Literal \\[x\\](javascript:alert(1)) and \\!\\[\\](http://tracker.example/p.png) stay text, as does \\<img src=http://tracker.example/q.png>."), "{text}");
        assert!(text.contains("\n\\# not a heading\n") && text.contains("\n\\> not a quote\n"), "{text}");
        assert!(text.contains("Wow\\![pic](https://contoso.example/pic.png) and a back\\\\slash."), "the link stays, never an image: {text}");
        assert!(text.contains("## Tags \\<i> and \\[brackets\\]"), "{text}");
        assert!(text.contains("| \\<td> \\[c\\] | `a<b` |"), "code is code, cells are text: {text}");
        assert!(text.ends_with("![\\](javascript:alert(1)) \\<b>](https://contoso.example/p.png)"), "a picture's words can't end it: {text}");
        assert_eq!(md_text("<b>[x]!\\"), "\\<b>\\[x\\]\\!\\\\");
    }

    #[test]
    fn a_huge_list_start_does_not_overflow() {
        let filler = "Filler text so that the page counts as readable for the extractor. ".repeat(4);
        let html = format!("<main><p>{filler}</p><ol start=\"18446744073709551615\"><li>First</li><li>Second</li></ol></main>");
        let text = read(&html, "https://contoso.example/").markdown;
        assert!(text.contains("18446744073709551615. First\n18446744073709551615. Second"), "{text}");
    }
}
