//! A web page as a source (spec K14-06, D36): Echo fetches an https page with `curl`, as `graph.rs` uses it — GET, no
//! cookies, at most 5 redirects that stay https, 5 MB, 20 s, HTML or plain text or Markdown — and keeps its main text
//! as Markdown: `<article>`, else `<main>`, else `<body>`; scripts, styles, navigation, headers, footers, asides and
//! forms left out; headings, paragraphs, lists, https links, code and tables kept. Read by a small HTML reader of
//! Echo's own (no crate): what it can't place it keeps as text.

use anyhow::{anyhow, bail, Context as _, Result};
use std::io::Write as _;
use std::process::{Command, Stdio};

use crate::app::NoWindow as _;

/// Less text than this is not a page worth keeping.
pub const MIN_TEXT: usize = 200;

/// Why a page with too little text is refused.
pub const NOTHING: &str = "Nothing readable here — save it as a PDF and add the file.";

/// The largest page Echo fetches.
const MAX_BYTES: u64 = 5 << 20;

/// Deeper elements hold their text but no structure of their own (a page can't overflow the reader).
const MAX_DEPTH: usize = 400;

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
}

impl Tree {
    fn push(&mut self, n: Node) {
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

/// The page as a tree: tags, text, comments left out; end tags HTML leaves out (`<li>`, `<p>`, cells) implied.
fn parse(html: &str) -> El {
    let mut t = Tree { stack: vec![El::new("#doc", vec![])] };
    let mut rest = html;
    while !rest.is_empty() {
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
                    let end = end_tag(rest, &format!("</{name}"));
                    let inner = decode(&rest[..end.unwrap_or(rest.len())]);
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
    t.stack.pop().expect("the document")
}

/// HTML's character references: the common names and every number.
fn decode(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
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
    out
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

/// `href` as an https address, if it is one — a relative one against the page's own (https) address. Anything else
/// (`javascript:`, `mailto:`, `data:`, http, this page's own anchors) is text.
fn https_link(base: &str, href: &str) -> Option<String> {
    let h: String = href.chars().filter(|c| !matches!(c, '\t' | '\n' | '\r')).collect();
    let h = h.trim();
    if h.is_empty() || h.starts_with('#') {
        return None;
    }
    let abs = if h.get(..8).is_some_and(|p| p.eq_ignore_ascii_case("https://")) {
        format!("https://{}", &h[8..])
    } else if let Some(rest) = h.strip_prefix("//") {
        format!("https://{rest}")
    } else if has_scheme(h) {
        return None;
    } else {
        let b = base.get(..8).filter(|p| p.eq_ignore_ascii_case("https://")).map(|_| &base[8..])?;
        let (host, path) = b.find(['/', '?', '#']).map_or((b, "/"), |i| (&b[..i], &b[i..]));
        let path = if path.starts_with('/') { path.split(['?', '#']).next().unwrap_or("/") } else { "/" };
        let joined = if h.starts_with('/') {
            h.to_string()
        } else if h.starts_with('?') {
            format!("{path}{h}")
        } else {
            format!("{}{h}", &path[..path.rfind('/').map_or(0, |i| i + 1)])
        };
        format!("https://{host}{}", dots(&joined))
    };
    let host = abs[8..].split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() {
        return None;
    }
    // Nothing that would end the Markdown link or the line.
    Some(abs.chars().filter(|c| !c.is_control()).map(|c| match c {
        ' ' => "%20".to_string(),
        '(' => "%28".to_string(),
        ')' => "%29".to_string(),
        '<' => "%3C".to_string(),
        '>' => "%3E".to_string(),
        '"' => "%22".to_string(),
        '\\' => "%5C".to_string(),
        c => c.to_string(),
    }).collect())
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
}

impl Out<'_> {
    fn push_text(&mut self, s: &str) {
        for c in s.chars() {
            if c.is_whitespace() {
                if !self.line.is_empty() && !self.line.ends_with([' ', '\n']) {
                    self.line.push(' ');
                }
            } else {
                self.line.push(c);
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
        let lines: Vec<String> = raw.split('\n').map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect();
        self.write(&lines);
    }

    /// An element's text on one line, its https links and code kept (headings, cells, link text).
    fn inline(&self, e: &El, in_link: bool) -> String {
        let mut s = String::new();
        for k in &e.kids {
            match k {
                Node::Text(t) => s.push_str(t),
                Node::El(k) if dropped(k) || k.name == "img" => {}
                Node::El(k) if k.name == "br" => s.push(' '),
                Node::El(k) if k.name == "a" && !in_link => s.push_str(&self.link(k)),
                Node::El(k) if k.name == "code" => s.push_str(&code_span(&collapse(&self.inline(k, true)))),
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

    /// A link as Markdown when it leads to an https page, else its text.
    fn link(&self, a: &El) -> String {
        let text = collapse(&self.inline(a, true));
        match a.attr("href").and_then(|h| https_link(self.base, h)) {
            Some(url) if !text.is_empty() => format!("[{}]({url})", text.replace('[', "\\[").replace(']', "\\]")),
            _ => text,
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
            "img" => {}
            "a" => {
                let l = self.link(e);
                self.line.push_str(&l);
            }
            "code" => {
                let c = code_span(&collapse(&self.inline(e, true)));
                self.line.push_str(&c);
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.flush();
                let text = collapse(&self.inline(e, false));
                if e.name == "h1" && self.skip_h1.as_deref().is_some_and(|t| t == text) {
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
                            n += 1;
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

/// The main text of a page as `(title, markdown)`: `<article>`, else `<main>`, else `<body>`. `base` is the page's
/// own URL, for its relative links. The title is the main part's first heading, else the page's `<title>`; it isn't
/// repeated in the Markdown. Under [`MIN_TEXT`] characters of text: refused ([`NOTHING`]).
pub fn main_text(html: &str, base: &str) -> Result<(String, String)> {
    let doc = parse(html);
    let root = main_of(&doc);
    if text_len(root) < MIN_TEXT {
        bail!(NOTHING);
    }
    let title = first_h1(root).or_else(|| title_of(&doc)).unwrap_or_default();
    let mut out = Out { base, md: String::new(), line: String::new(), lists: 0, indent: 0, marker: None, item: false, skip_h1: Some(title.clone()).filter(|t| !t.is_empty()) };
    root.kids.iter().for_each(|k| out.walk(k));
    out.flush();
    Ok((title, out.md.trim().to_string()))
}

// ── the fetch ──

/// What a fetch may do.
struct Rules {
    /// The first request's scheme (always https but in tests of the fetch itself).
    scheme: &'static str,
    /// The whole fetch, in seconds.
    secs: u64,
    /// The test's own server: no proxy.
    local: bool,
}

const WEB: Rules = Rules { scheme: "https", secs: 20, local: false };

/// Fetch an https page and keep its main text: `(title, markdown)`.
pub fn fetch(url: &str) -> Result<(String, String)> {
    fetch_with(url, &WEB)
}

/// The address the user typed, if it is one Echo fetches.
fn checked_url(url: &str, scheme: &str) -> Result<String> {
    let u = url.trim();
    if u.is_empty() || u.len() > 4096 || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        bail!("that isn't a web address");
    }
    let prefix = format!("{scheme}://");
    if !u.get(..prefix.len()).is_some_and(|p| p.eq_ignore_ascii_case(&prefix)) {
        bail!("Echo fetches only https pages — the address must start with https://");
    }
    let host = u[prefix.len()..].split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() {
        bail!("that isn't a web address");
    }
    Ok(u.to_string())
}

/// A value in curl's config: quoted, so a newline can't start another option.
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r"))
}

/// The page's bytes, its type (`text/html`…) and the address it came from after redirects.
fn get(url: &str, r: &Rules) -> Result<(Vec<u8>, String, String)> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!("echo-fetch-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&dir)?;
    let got = curl(url, &dir.join("page"), r);
    let _ = std::fs::remove_dir_all(&dir);
    got
}

fn curl(url: &str, body: &std::path::Path, r: &Rules) -> Result<(Vec<u8>, String, String)> {
    let mut config = format!(
        "url = {}\nlocation\nmax-redirs = 5\nproto = \"={}\"\nproto-redir = \"=https\"\nmax-filesize = {MAX_BYTES}\nmax-time = {}\noutput = {}\nwrite-out = \"%{{http_code}}\\n%{{content_type}}\\n%{{url_effective}}\"\n",
        quote(url),
        r.scheme,
        r.secs,
        quote(&body.display().to_string()),
    );
    if r.local {
        config.push_str("noproxy = \"127.0.0.1\"\n");
    }
    // `-q` first: the user's own `.curlrc` can't add cookies, credentials or anything else to Echo's fetch.
    let mut child = Command::new("curl")
        .no_window()
        .args(["-q", "-sS", "-K", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Echo couldn't run curl")?;
    child.stdin.take().ok_or_else(|| anyhow!("curl has no input"))?.write_all(config.as_bytes())?;
    let out = child.wait_with_output()?;
    let host = url.split("://").nth(1).and_then(|h| h.split(['/', '?', '#']).next()).unwrap_or(url);
    match out.status.code() {
        Some(0) => {}
        Some(1) => bail!("the page sent Echo on to an address that isn't https"),
        Some(6) => bail!("Echo couldn't find {host}"),
        Some(7) => bail!("Echo couldn't connect to {host}"),
        Some(28) => bail!("the page took longer than {} s", r.secs),
        Some(47) => bail!("the page redirected more than 5 times"),
        Some(63) => bail!("the page is over 5 MB"),
        Some(35 | 51 | 53 | 54 | 58 | 59 | 60 | 66 | 77 | 80 | 82 | 83 | 90 | 91) => bail!("{host}'s secure connection couldn't be checked"),
        _ => bail!("Echo couldn't fetch the page: {}", String::from_utf8_lossy(&out.stderr).trim()),
    }
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut lines = said.lines();
    let code: u16 = lines.next().and_then(|c| c.trim().parse().ok()).unwrap_or(0);
    let mime = lines.next().unwrap_or("").split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    let at = lines.next().map(str::trim).filter(|u| !u.is_empty()).unwrap_or(url).to_string();
    if !(200..300).contains(&code) {
        bail!("the site answered {code}");
    }
    if !["text/html", "text/plain", "text/markdown"].contains(&mime.as_str()) {
        if mime.is_empty() {
            bail!("the site doesn't say what this is — Echo saves web pages (HTML or text)");
        }
        bail!("this is {mime}, not a web page — download it and add the file");
    }
    let bytes = match std::fs::read(body) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(e.into()),
    };
    if bytes.len() as u64 > MAX_BYTES {
        bail!("the page is over 5 MB");
    }
    Ok((bytes, mime, at))
}

/// A text or Markdown file: its own first `# ` heading is its title.
fn plain(text: &str) -> Result<(String, String)> {
    let t = text.replace("\r\n", "\n");
    let t = t.trim();
    if collapse(t).chars().count() < MIN_TEXT {
        bail!(NOTHING);
    }
    match t.lines().next().and_then(|l| l.strip_prefix("# ")) {
        Some(h) => Ok((h.trim().to_string(), t[t.find('\n').map_or(t.len(), |i| i + 1)..].trim().to_string())),
        None => Ok((String::new(), t.to_string())),
    }
}

fn fetch_with(url: &str, r: &Rules) -> Result<(String, String)> {
    let url = checked_url(url, r.scheme)?;
    let (bytes, mime, at) = get(&url, r)?;
    let text = String::from_utf8_lossy(&bytes);
    let (title, md) = if mime == "text/html" { main_text(&text, &at)? } else { plain(&text)? };
    // No title of its own: its address, without the scheme.
    let title = if title.is_empty() { at.split_once("://").map_or(at.as_str(), |(_, rest)| rest).trim_end_matches('/').to_string() } else { title };
    Ok((title, md))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ten pages made up for these tests (no real site's text).
    fn page(name: &str) -> &'static str {
        match name {
            "article" => include_str!("testdata/web/article.html"),
            "chrome" => include_str!("testdata/web/chrome.html"),
            "tables" => include_str!("testdata/web/tables.html"),
            "links" => include_str!("testdata/web/links.html"),
            "short" => include_str!("testdata/web/short.html"),
            "code" => include_str!("testdata/web/code.html"),
            "nested" => include_str!("testdata/web/nested.html"),
            "unicode" => include_str!("testdata/web/unicode.html"),
            "two-articles" => include_str!("testdata/web/two-articles.html"),
            "body-only" => include_str!("testdata/web/body-only.html"),
            other => panic!("no page {other}"),
        }
    }

    fn md(name: &str) -> (String, String) {
        main_text(page(name), "https://contoso.example/notes/page").unwrap_or_else(|e| panic!("{name}: {e:#}"))
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
    fn only_https_links_stay_links() {
        let (_, text) = main_text(page("links"), "https://contoso.example/notes/links").unwrap();
        assert!(text.contains("Run the demo is a script link"), "{text}");
        assert!(!text.contains("javascript") && !text.contains("alert"), "a script link is text: {text}");
        assert!(text.contains("The plain report uses http") && !text.contains("http://"), "{text}");
        assert!(text.contains("[The secure report](https://secure.example/report?id=7)"), "{text}");
        assert!(text.contains("[The guide](https://cdn.contoso.example/guide.pdf)"), "{text}");
        assert!(text.contains("[Omar's page](https://contoso.example/team/omar-reyes)"), "{text}");
        assert!(text.contains("back to top stays here") && text.contains("A data link and a mail link are text"), "{text}");
        assert!(!text.contains("mailto") && !text.contains("data:"), "{text}");
        assert!(text.contains("[An odd address](https://evil.example/a%20b%28c%29)"), "{text}");
        assert_eq!(text.matches("](").count(), 4, "four https links: {text}");
    }

    #[test]
    fn a_short_page_is_refused() {
        let e = main_text(page("short"), "https://contoso.example/sign-in").unwrap_err().to_string();
        assert_eq!(e, NOTHING);
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
        assert!(text.contains("Quotes stay as written: “fix forward, no rollback” … © Northwind <internal>."), "{text}");
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
    fn deep_nesting_is_read_without_overflow() {
        let mut html = String::from("<html><body><main><p>");
        html.push_str(&"<div><span>".repeat(20_000));
        html.push_str(&"Deeply nested text that still counts as the page's words. ".repeat(5));
        html.push_str("</main></body></html>");
        let (_, text) = main_text(&html, "https://contoso.example/").unwrap();
        assert!(text.contains("Deeply nested text"), "{text}");
    }

    // ── the fetch (K14-T5): a local server, plain http for the first request only ──

    use std::io::Read as _;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A made-up site on 127.0.0.1: its address and how many requests it has had.
    fn serve() -> (String, Arc<AtomicUsize>) {
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("a local port");
        let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        let hits = Arc::new(AtomicUsize::new(0));
        let (b, h) = (base.clone(), hits.clone());
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                h.fetch_add(1, Ordering::SeqCst);
                let b = b.clone();
                std::thread::spawn(move || answer(s, &b));
            }
        });
        (base, hits)
    }

    fn answer(mut s: std::net::TcpStream, base: &str) {
        let mut req = vec![];
        let mut buf = [0u8; 1024];
        while !req.windows(4).any(|w| w == b"\r\n\r\n") {
            match s.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => req.extend_from_slice(&buf[..n]),
            }
        }
        let req = String::from_utf8_lossy(&req);
        let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
        let reply = |s: &mut std::net::TcpStream, status: &str, kind: &str, body: &[u8]| {
            let head = format!("HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(body);
        };
        match path.as_str() {
            "/page" => reply(&mut s, "200 OK", "text/html; charset=utf-8", page("article").as_bytes()),
            "/notes.md" => reply(&mut s, "200 OK", "text/markdown", "# Store opening notes\n\nThe new store in the harbour district opens on 4 December. Staff training runs the week before, and the till hardware arrives on 28 November with two spare scanners. Omar Reyes signs off the opening checklist.\n".as_bytes()),
            "/to-http" => {
                let head = format!("HTTP/1.1 302 Found\r\nLocation: {base}/page\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                let _ = s.write_all(head.as_bytes());
            }
            "/big" => reply(&mut s, "200 OK", "text/html", &vec![b'a'; 6 << 20]),
            "/slow" => {
                std::thread::sleep(std::time::Duration::from_secs(3));
                reply(&mut s, "200 OK", "text/html", page("article").as_bytes());
            }
            "/pdf" => reply(&mut s, "200 OK", "application/pdf", b"%PDF-1.4 made up"),
            _ => reply(&mut s, "404 Not Found", "text/html", b"<p>no</p>"),
        }
    }

    /// The fetch's own rules, but a first request over plain http to the test's server (redirects stay https-only).
    const LOCAL: Rules = Rules { scheme: "http", secs: 5, local: true };

    #[test]
    fn fetch_rules_refuse_with_their_reasons() {
        let (base, hits) = serve();
        // A good page and a Markdown file come through.
        let (title, text) = fetch_with(&format!("{base}/page"), &LOCAL).unwrap_or_else(|e| panic!("{e:#}"));
        assert_eq!(title, "How Northwind Retail moved its pilot");
        assert!(text.contains("The full plan is in the pilot plan"), "a link relative to an http page is text: {text}");
        let (title, text) = fetch_with(&format!("{base}/notes.md"), &LOCAL).unwrap_or_else(|e| panic!("{e:#}"));
        assert_eq!(title, "Store opening notes");
        assert!(text.starts_with("The new store in the harbour district"), "{text}");
        let before = hits.load(Ordering::SeqCst);
        // An http address: refused before anything is asked.
        let e = fetch(&format!("{base}/page")).unwrap_err().to_string();
        assert!(e.contains("only https"), "{e}");
        assert_eq!(hits.load(Ordering::SeqCst), before, "nothing was asked of the site");
        for bad in ["https://", "ftp://contoso.example/x", "https://contoso.example/a b", "https://contoso.example/\nx", "file:///etc/hosts"] {
            assert!(fetch(bad).is_err(), "{bad:?}");
        }
        // A redirect off https.
        let e = fetch_with(&format!("{base}/to-http"), &LOCAL).unwrap_err().to_string();
        assert!(e.contains("isn't https"), "{e}");
        // Over 5 MB.
        let e = fetch_with(&format!("{base}/big"), &LOCAL).unwrap_err().to_string();
        assert!(e.contains("over 5 MB"), "{e}");
        // Too slow (the rule's 20 s made 1 s here).
        let e = fetch_with(&format!("{base}/slow"), &Rules { secs: 1, ..LOCAL }).unwrap_err().to_string();
        assert!(e.contains("longer than 1 s"), "{e}");
        // Not a web page.
        let e = fetch_with(&format!("{base}/pdf"), &LOCAL).unwrap_err().to_string();
        assert!(e.contains("application/pdf"), "{e}");
        let e = fetch_with(&format!("{base}/missing"), &LOCAL).unwrap_err().to_string();
        assert!(e.contains("404"), "{e}");
    }
}
