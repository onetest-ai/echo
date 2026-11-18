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

pub use super::link::web_link;
use super::link::web_scheme;
#[cfg(test)]
use super::link::MAX_URL;

/// Less text than this, on a page with scripts, is a page that needs JavaScript to show itself (`needs_js`).
pub const MIN_TEXT: usize = 200;

/// Deeper elements hold their text but no structure of their own (a page can't overflow the reader).
const MAX_DEPTH: usize = 256;
/// Elements and text pieces read at most: past it the rest of the page is left out (`truncated`).
const MAX_NODES: usize = 250_000;
/// Links and pictures listed at most.
const MAX_LINKS: usize = 2_000;
const MAX_IMAGES: usize = 500;
/// A caption, a picture's words or a link's text in the lists, in characters at most; and a link's address in bytes
/// (a longer one is no link).
const MAX_LINE: usize = 500;
/// A table's columns kept at most.
const MAX_COLUMNS: usize = 64;
/// List items nested this deep at most have a marker and indent what they hold (a deeper item's text sits at the
/// deepest one's indent, with no marker), and the largest number a numbered list starts at.
const MAX_LIST_DEPTH: usize = 8;
const MAX_LIST_START: u64 = 1_000_000;
/// The Markdown written at most: ten times the HTML read, and never past 20 MB (`truncated`).
const MAX_MD: usize = 20 << 20;
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

pub(super) fn bad_char(c: char) -> bool {
    (c.is_control() && !matches!(c, '\n' | '\t'))
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

/// The outermost elements named `name` (none inside another): each part of the page in one of them at most.
fn find_outer<'a>(e: &'a El, name: &str, out: &mut Vec<&'a El>) {
    for k in e.els().filter(|k| !dropped(k)) {
        if k.name == name {
            out.push(k);
        } else {
            find_outer(k, name, out);
        }
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

/// The characters of `s` as `collapse` would leave them, counted without making it.
fn text_chars(s: &str) -> usize {
    let (mut n, mut gap) = (0, false);
    for c in s.chars() {
        if c.is_whitespace() {
            gap = n > 0;
        } else {
            n += 1 + usize::from(gap);
            gap = false;
        }
    }
    n
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text_len(e: &El) -> usize {
    let mut s = String::new();
    plain_text(e, &mut s);
    collapse(&s).chars().count()
}

/// The page's main part: its largest `<article>`, else its `<main>`, else its `<body>`. An article holds the articles
/// inside it, so only the outermost are measured (each word once).
fn main_of(doc: &El) -> &El {
    for name in ["article", "main", "body"] {
        let mut found = vec![];
        if name == "article" {
            find_outer(doc, name, &mut found);
        } else {
            find_all(doc, name, &mut found);
        }
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

/// An address as a Markdown link's destination: an `&` that would start an entity (`&#64;`, `&amp;`) escaped, as
/// CommonMark decodes entities there — the link leads where `links` says.
fn dest(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    for (i, c) in url.char_indices() {
        if c == '&' {
            let rest = &url[i + 1..];
            let name = rest.bytes().take_while(u8::is_ascii_alphanumeric).count();
            if rest.starts_with('#') || (name > 0 && rest.as_bytes().get(name) == Some(&b';')) {
                out.push('\\');
            }
        }
        out.push(c);
    }
    out
}

/// The ASCII punctuation that means something to Markdown or GFM wherever it stands (HTML, links, pictures, code,
/// emphasis, strikethrough, tables, headings, quotes): escaped wherever a page's words are written. `-`, `+` and `=`
/// mean something only where a line starts (`line_start`), `&` only where it starts an entity (`md_char`).
const MARKS: &str = "\\`*_~[]()<>|#!{}";

/// Text as Markdown shows it, never as markup: each of `MARKS` escaped, and an `&` that starts something entity-like,
/// so no HTML, link, picture, code span, emphasis, table or block of any kind can come of a page's (or anyone's)
/// words. A line's start is `line_start`'s.
pub fn md_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, c) in s.char_indices() {
        md_char(c, &s[i + c.len_utf8()..], &mut out);
    }
    out
}

/// One character of text as `md_text` writes it, `rest` the text after it.
fn md_char(c: char, rest: &str, out: &mut String) {
    let entity = || {
        let name = rest.bytes().take_while(u8::is_ascii_alphanumeric).count();
        rest.starts_with('#') || (name > 0 && rest[name..].starts_with(';'))
    };
    if MARKS.contains(c) || (c == '&' && entity()) {
        out.push('\\');
    }
    out.push(c);
}

/// A line of text (`md_text`'s) as it starts a line: a `-`, `+` or `=` there can't start a list, a rule or a setext
/// underline (`\-`), nor digits and a `.` a numbered list (`1\.`); every other mark that starts a block is escaped
/// already, and its indent is the writer's.
fn line_start(l: &str) -> String {
    let digits = l.bytes().take_while(u8::is_ascii_digit).count();
    if l.starts_with(['-', '+', '=']) {
        format!("\\{l}")
    } else if digits > 0 && l[digits..].starts_with('.') {
        format!("{}\\{}", &l[..digits], &l[digits..])
    } else {
        l.to_owned()
    }
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

/// The text of an element as one plain line (no Markdown), `MAX_LINE` characters at most, for a caption or a link's
/// text in the list; what is inside a `skip` element isn't read (a figure in a caption).
fn line_of(e: &El, skip: &str) -> String {
    let mut s = String::new();
    words_of(e, skip, &mut s, &mut 0);
    s
}

/// `line_of`'s words, `n` characters so far; true once full.
fn words_of(e: &El, skip: &str, out: &mut String, n: &mut usize) -> bool {
    for k in &e.kids {
        match k {
            Node::Text(t) => {
                for w in t.split_whitespace() {
                    if *n > 0 {
                        if *n + 1 >= MAX_LINE {
                            return true;
                        }
                        out.push(' ');
                        *n += 1;
                    }
                    for c in w.chars() {
                        if *n >= MAX_LINE {
                            return true;
                        }
                        out.push(c);
                        *n += 1;
                    }
                }
            }
            Node::El(k) if !dropped(k) && k.name != skip => {
                if words_of(k, skip, out, n) {
                    return true;
                }
            }
            Node::El(_) => {}
        }
    }
    false
}

/// The `href`s read and the addresses listed, for `links_of`.
type Seen = (std::collections::HashSet<String>, std::collections::HashSet<String>);

/// The links of the main part, each once: `(text, href)`. A link holds no link (a browser never nests them).
fn links_of(e: &El, base: &str, out: &mut Vec<(String, String)>, seen: &mut Seen) {
    for k in e.els().filter(|k| !dropped(k)) {
        if out.len() >= MAX_LINKS {
            return;
        }
        if k.name != "a" {
            links_of(k, base, out, seen);
        } else if let Some(href) = k.attr("href").filter(|h| seen.0.insert(h.to_string())).and_then(|h| web_link(base, h)) {
            if seen.1.insert(href.clone()) {
                out.push((line_of(k, ""), href));
            }
        }
    }
}

/// `s` after `dst`, a space between when a backtick would meet a backtick: a code span's fence never runs into
/// another's, so none ends early and lets its text out.
fn join(dst: &mut String, s: &str) {
    if dst.ends_with('`') && s.starts_with('`') {
        dst.push(' ');
    }
    dst.push_str(s);
}

/// The Markdown being written.
struct Out<'a> {
    base: &'a str,
    md: String,
    /// The inline text being gathered; `\n` is a line break.
    line: String,
    /// List items open around what is written, and the column their lines start at: an item's marker sits at its
    /// list's column and its lines at that column plus the marker's width, so none falls out of it.
    depth: usize,
    indent: usize,
    /// A list item's marker, for its first line, and whether the item follows another of its list.
    marker: Option<String>,
    next: bool,
    /// The last block written was in a list item: the next item of a list follows it one line apart (only a list's
    /// next item with text on its first line: a list's first item, or an empty one, may not interrupt a paragraph).
    item: bool,
    /// The title's own heading, written once above the text.
    skip_h1: Option<String>,
    /// The pictures written, in order, each once; and the caption of the figure being written.
    images: Vec<Image>,
    seen: std::collections::HashSet<String>,
    caption: Option<String>,
    /// The Markdown's bytes at most (`MAX_MD`), and whether the rest was left out for it.
    most: usize,
    full: bool,
    /// Some of what was read was left out (`MAX_MD`, `MAX_COLUMNS`).
    cut: bool,
}

impl<'a> Out<'a> {
    /// Markdown of `most` bytes at most.
    fn new(base: &'a str, skip_h1: Option<String>, most: usize) -> Out<'a> {
        Out { base, md: String::new(), line: String::new(), depth: 0, indent: 0, marker: None, next: false, item: false, skip_h1, images: vec![], seen: Default::default(), caption: None, most, full: false, cut: false }
    }

    /// What the Markdown has room for still, the line being gathered and the blank line and indent before it counted.
    fn room(&self) -> usize {
        self.most.saturating_sub(self.md.len() + self.line.len() + 2 + self.indent)
    }

    /// The Markdown is full: the line gathered so far is written (as it fits), the rest left out.
    fn fill(&mut self) {
        self.flush();
        self.full = true;
        self.cut = true;
    }

    /// Markdown added to the line, while it fits (`join`).
    fn add(&mut self, s: &str) {
        if self.full {
            return;
        }
        if s.len() + 1 > self.room() {
            return self.fill();
        }
        join(&mut self.line, s);
    }

    fn push_text(&mut self, s: &str) {
        for (i, c) in s.char_indices() {
            if self.full {
                return;
            }
            if self.room() < 8 {
                return self.fill();
            }
            if c.is_whitespace() {
                if !self.line.is_empty() && !self.line.ends_with([' ', '\n']) {
                    self.line.push(' ');
                }
            } else {
                md_char(c, &s[i + c.len_utf8()..], &mut self.line);
            }
        }
    }

    /// Lines written as a block, the first after its list item's marker, the rest at the item's indent. What would take
    /// the Markdown past `most` is left out: a fenced block (`whole`) all of it, else from the first line that doesn't
    /// fit (a line of text holds whole inline constructs, a table's line a whole row), so a cut never falls inside a
    /// code span, a link or a fence.
    fn write(&mut self, lines: &[String], whole: bool) {
        if lines.is_empty() || self.full {
            return;
        }
        let marker = self.marker.take();
        let sep = if self.md.is_empty() {
            ""
        } else if marker.is_some() && self.next && self.item && !lines[0].is_empty() {
            "\n"
        } else {
            "\n\n"
        };
        let pad = " ".repeat(self.indent);
        let mut block = String::new();
        let mut kept = 0;
        for (i, l) in lines.iter().enumerate() {
            let mut one = String::new();
            if i == 0 {
                one.push_str(sep);
                match &marker {
                    // The item's lines start at `indent`, its marker's width further than the marker does.
                    Some(m) => {
                        one.push_str(&pad[m.len()..]);
                        one.push_str(m);
                    }
                    None => one.push_str(&pad),
                }
            } else {
                one.push('\n');
                if !l.is_empty() {
                    one.push_str(&pad);
                }
            }
            one.push_str(l);
            if self.md.len() + block.len() + one.len() > self.most {
                break;
            }
            block.push_str(&one);
            kept += 1;
        }
        if kept < lines.len() {
            self.full = true;
            self.cut = true;
            if whole {
                return;
            }
        }
        self.md.push_str(&block);
        self.item = self.depth > 0;
    }

    fn flush(&mut self) {
        let raw = std::mem::take(&mut self.line);
        let lines: Vec<String> = raw.split('\n').map(str::trim).filter(|l| !l.is_empty()).map(line_start).collect();
        self.write(&lines, false);
    }

    /// An element's text on one line (as Markdown text), its links and code kept (headings, cells, link text); no more
    /// is gathered once it is past `most` bytes. In a table's `cell`, code holding a `|` is text (GFM splits a cell at
    /// it even inside a code span).
    fn inline(&self, e: &El, in_link: bool, cell: bool, most: usize) -> String {
        let mut s = String::new();
        for k in &e.kids {
            if s.len() > most {
                break;
            }
            let left = most - s.len();
            match k {
                Node::Text(t) => s.push_str(&md_text(t)),
                Node::El(k) if dropped(k) || k.name == "img" || k.name == "picture" => {}
                Node::El(k) if k.name == "br" => s.push(' '),
                Node::El(k) if k.name == "a" && !in_link => join(&mut s, &self.link(k, cell, left)),
                Node::El(k) if k.name == "code" => join(&mut s, &code_of(k, cell)),
                Node::El(k) => {
                    let block = is_block(&k.name) || matches!(k.name.as_str(), "td" | "th");
                    if block {
                        s.push(' ');
                    }
                    join(&mut s, &self.inline(k, in_link, cell, left));
                    if block {
                        s.push(' ');
                    }
                }
            }
        }
        s
    }

    /// A link as Markdown when it leads to an http(s) page, else its text (`cell`, `most` as for `inline`).
    fn link(&self, a: &El, cell: bool, most: usize) -> String {
        let text = collapse(&self.inline(a, true, cell, most));
        if text.is_empty() {
            return text;
        }
        match a.attr("href").and_then(|h| web_link(self.base, h)) {
            Some(url) => format!("[{text}]({})", dest(&url)),
            None => text,
        }
    }

    /// An `<img>` (or a `<picture>`'s best source with its `<img>`'s words) as a picture: an http(s) address (never
    /// `data:`), not a tracking pixel.
    fn image_of(&self, e: &El) -> Option<Image> {
        let img = if e.name == "img" { Some(e) } else { e.els().find(|k| k.name == "img") };
        let (width, height) = img.map_or((None, None), |i| (size_attr(i, "width"), size_attr(i, "height")));
        if width.is_some_and(|w| w < TINY) || height.is_some_and(|h| h < TINY) {
            return None;
        }
        let mut sets: Vec<(u64, &str)> = vec![];
        if e.name == "picture" {
            sets.extend(e.els().filter(|k| k.name == "source").filter_map(|s| s.attr("srcset").and_then(best_of_srcset)));
        }
        sets.extend(img.and_then(|i| i.attr("srcset")).and_then(best_of_srcset));
        sets.sort_by_key(|(w, _)| std::cmp::Reverse(*w));
        let largest = sets.into_iter().find_map(|(_, u)| web_link(self.base, u));
        // A lazy page keeps the picture in `data-src` and a placeholder (often `data:`) in `src`.
        let src = largest.or_else(|| ["src", "data-src", "data-original", "data-lazy-src"].iter().find_map(|k| img.and_then(|i| i.attr(k)).and_then(|u| web_link(self.base, u))))?;
        let alt = img.and_then(|i| i.attr("alt")).and_then(|a| short(a, MAX_LINE)).unwrap_or_default();
        Some(Image { src, alt, caption: self.caption.clone(), width, height })
    }

    /// A picture on a line of its own: `![alt](src)`, listed once.
    fn picture(&mut self, img: Image) {
        self.flush();
        self.write(&[format!("![{}]({})", md_text(&img.alt), dest(&img.src))], true);
        if self.images.len() < MAX_IMAGES && self.seen.insert(img.src.clone()) {
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
        if self.full {
            return;
        }
        let e = match n {
            Node::Text(t) => return self.push_text(t),
            Node::El(e) => e,
        };
        if dropped(e) {
            return;
        }
        match e.name.as_str() {
            "br" => self.add("\n"),
            "img" | "picture" => match self.image_of(e) {
                Some(i) => self.picture(i),
                // A picture with no picture of its own: whatever it holds is read as text.
                None => e.kids.iter().for_each(|k| self.walk(k)),
            },
            "a" => {
                self.pictures_in(e);
                let l = self.link(e, false, self.room());
                self.add(&l);
            }
            "code" => self.add(&code_of(e, false)),
            "figure" => {
                // The picture first, its caption under it as text.
                self.flush();
                let caption = e.els().find(|k| k.name == "figcaption").map(|c| line_of(c, "figure")).filter(|c| !c.is_empty());
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
                let text = collapse(&self.inline(e, false, false, self.room()));
                if e.name == "h1" && self.skip_h1.as_deref().is_some_and(|t| md_text(t) == text) {
                    self.skip_h1 = None;
                    return;
                }
                if !text.is_empty() {
                    let level = e.name[1..].parse::<usize>().unwrap_or(1);
                    self.write(&[format!("{} {text}", "#".repeat(level))], true);
                }
            }
            "ul" | "ol" => {
                self.flush();
                let mut n: u64 = if e.name == "ol" { e.attr("start").and_then(|s| s.trim().parse::<u64>().ok()).map_or(1, |s| s.min(MAX_LIST_START)) } else { 0 };
                let mut next = false;
                for k in &e.kids {
                    match k {
                        Node::El(li) if li.name == "li" => {
                            let marker = if e.name == "ol" { format!("{n}. ") } else { "- ".to_string() };
                            n = n.saturating_add(1);
                            self.item(li, marker, next);
                            next = true;
                        }
                        other => self.walk(other),
                    }
                }
                self.flush();
            }
            "li" => self.item(e, "- ".into(), false),
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
                    self.write(&lines, true);
                }
            }
            "table" => {
                self.flush();
                let mut rows = vec![];
                let room = self.room();
                self.rows(e, &mut rows, &mut 0, room);
                let n = rows.iter().map(Vec::len).max().unwrap_or(0);
                if n > 0 {
                    // Every row as wide as the widest: the lines are made only while they fit what is left.
                    let line = |r: &[String]| format!("| {} |", (0..n).map(|i| r.get(i).map(String::as_str).unwrap_or("")).collect::<Vec<_>>().join(" | "));
                    let mut lines = vec![line(&rows[0]), format!("| {} |", vec!["---"; n].join(" | "))];
                    let mut size: usize = lines.iter().map(|l| l.len() + 1).sum();
                    for r in &rows[1..] {
                        if size > room {
                            break;
                        }
                        let l = line(r);
                        size += l.len() + 1;
                        lines.push(l);
                    }
                    self.write(&lines, false);
                    if size > room {
                        self.full = true;
                        self.cut = true;
                    }
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

    /// A list item: its marker, and its lines indented by the marker's width. Past `MAX_LIST_DEPTH` items (bare lists
    /// without items don't count) an item has no marker and its lines sit at the deepest item's indent.
    fn item(&mut self, li: &El, marker: String, next: bool) {
        self.flush();
        // An item that holds a list before any text of its own: its marker on a line of its own first.
        if self.marker.is_some() {
            self.write(&[String::new()], true);
        }
        let saved = self.indent;
        if self.depth < MAX_LIST_DEPTH {
            self.indent += marker.len();
            self.marker = Some(marker);
            self.next = next;
        }
        self.depth += 1;
        li.kids.iter().for_each(|k| self.walk(k));
        self.flush();
        self.depth -= 1;
        self.marker = None;
        self.indent = saved;
    }

    /// A table's rows of cells, `MAX_COLUMNS` at most (a table inside a cell is that cell's text), made while their
    /// `used` bytes fit `room`: past it the rest is left out.
    fn rows(&mut self, e: &El, out: &mut Vec<Vec<String>>, used: &mut usize, room: usize) {
        for k in e.els().filter(|k| !dropped(k)) {
            if *used > room {
                self.cut = true;
                return;
            }
            match k.name.as_str() {
                "tr" => {
                    let cells: Vec<&El> = k.els().filter(|c| c.name == "td" || c.name == "th").collect();
                    if cells.len() > MAX_COLUMNS {
                        self.cut = true;
                    }
                    let mut row = vec![];
                    *used += 4;
                    for c in cells.iter().take(MAX_COLUMNS) {
                        let cell = collapse(&self.inline(c, false, true, room.saturating_sub(*used)));
                        *used += cell.len() + 3;
                        row.push(cell);
                    }
                    out.push(row);
                }
                "table" => {}
                _ => self.rows(k, out, used, room),
            }
        }
    }
}

/// Inline code: its text as written, on one line, in backticks; in a table's `cell`, code holding a `|` is text.
fn code_of(e: &El, cell: bool) -> String {
    let mut s = String::new();
    raw_text(e, &mut s);
    let s = collapse(&s);
    if cell && s.contains('|') {
        md_text(&s)
    } else {
        code_span(&s)
    }
}

/// `text` as a code span (CommonMark): fenced by one backtick more than its longest run, so none of its backticks
/// ends it; a space inside the fence when it starts or ends with one.
fn code_span(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let fence = "`".repeat(text.split(|c| c != '`').map(str::len).max().unwrap_or(0) + 1);
    let pad = if text.starts_with('`') || text.ends_with('`') { " " } else { "" };
    format!("{fence}{pad}{text}{pad}{fence}")
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

/// HTML (a piece of a page, a noscript's, a CMS body in JSON) as Markdown of `most` bytes at most; and whether some of
/// it was left out (`MAX_NODES`, `most`).
fn html_md(html: &str, base: &str, most: usize) -> (String, bool) {
    let (doc, truncated) = parse(html);
    let mut out = Out::new(base, None, most);
    doc.kids.iter().for_each(|k| out.walk(k));
    out.flush();
    (out.md.trim().to_string(), truncated || out.cut)
}

/// Plain text (paragraphs on their lines) as Markdown text of `most` bytes at most; and whether some was left out.
fn text_md(text: &str, base: &str, most: usize) -> (String, bool) {
    let mut out = Out::new(base, None, most);
    for para in text.split('\n') {
        out.push_text(para);
        out.flush();
    }
    (out.md.trim().to_string(), out.cut)
}

/// A string that may hold HTML as Markdown of `most` bytes at most; and whether some of it was left out.
fn any_md(s: &str, base: &str, most: usize) -> (String, bool) {
    if s.contains("</") || s.contains("<p>") || s.contains("<br") {
        html_md(s, base, most)
    } else {
        text_md(s, base, most)
    }
}

/// Pieces of Markdown, each made by `md` within what is left of one budget of `most` bytes, joined by blank lines: a
/// piece is whole Markdown (its fences closed), so the joins are the only places it ends. And whether some was left out.
fn joined<T>(items: &[T], most: usize, md: impl Fn(&T, usize) -> (String, bool)) -> (String, bool) {
    let (mut out, mut cut) = (String::new(), false);
    for it in items {
        let sep = if out.is_empty() { 0 } else { 2 };
        let left = most.saturating_sub(out.len() + sep);
        if left == 0 {
            return (out, true);
        }
        let (m, c) = md(it, left);
        cut |= c;
        if !m.is_empty() {
            if sep > 0 {
                out.push_str("\n\n");
            }
            out.push_str(&m);
        }
    }
    (out, cut)
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
    /// The page was too big to read whole (`MAX_NODES`), or its Markdown to write whole (`MAX_MD`, `MAX_COLUMNS`).
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
    read_in(html, base, html.len().saturating_mul(10).min(MAX_MD))
}

/// `read` with the Markdown kept to `most` bytes.
fn read_in(html: &str, base: &str, most: usize) -> Page {
    // As HTML reads its input: a CR, alone or before a LF, is a LF — CommonMark ends a line at a lone CR too, so one
    // kept would start a line Echo didn't indent.
    let html = html.replace("\r\n", "\n").replace('\r', "\n");
    let (doc, truncated) = parse(&html);
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

    let mut out = Out::new(base, Some(title.clone()).filter(|t| !t.is_empty()), most);
    root.kids.iter().for_each(|k| out.walk(k));
    out.flush();
    let mut images = std::mem::take(&mut out.images);
    let html_text = out.md.trim().to_string();

    // What the page embeds, as Markdown within the same budget (all of a page's noscripts, or all of its framework
    // state, within one); the fuller text wins, the page's own HTML on a near tie.
    let len = text_chars;
    let mut best = (len(&html_text), html_text, "html", out.cut);
    let mut offer = |md: String, from: &'static str, cut: bool| {
        let n = len(&md);
        if n > best.0 + best.0 / 4 + 20 {
            best = (n, md, from, cut);
        }
    };
    if let Some(b) = &ld.body {
        let (md, cut) = any_md(b, base, most);
        offer(md, "json-ld", cut);
    }
    if !state.is_empty() {
        let (md, cut) = joined(&state, most, |s, left| any_md(s, base, left));
        offer(md, "state", cut);
    }
    let mut noscripts = vec![];
    every(&doc, "noscript", 50, &mut noscripts);
    if !noscripts.is_empty() {
        let (md, cut) = joined(&noscripts, most, |n, left| html_md(own_text(n), base, left));
        offer(md, "noscript", cut);
    }
    let description = meta(&["og:description", "description", "twitter:description"]).or_else(|| ld.description.as_deref().and_then(|d| short(d, 2000)));
    let (n, markdown, source, cut) = best;
    let (markdown, source) = match (&description, n) {
        (Some(d), 0) => (text_md(d, base, most).0, "meta"),
        _ => (markdown, source),
    };
    let truncated = truncated || cut;

    // Pictures the page names for itself (Open Graph, JSON-LD) after those of its text.
    for src in meta(&["og:image", "og:image:url", "twitter:image"]).into_iter().chain(ld.images.iter().cloned()) {
        if let Some(src) = web_link(base, &src) {
            if images.len() < MAX_IMAGES && !images.iter().any(|i| i.src == src) {
                images.push(Image { src, alt: String::new(), caption: None, width: None, height: None });
            }
        }
    }
    let mut links = vec![];
    links_of(root, base, &mut links, &mut Default::default());
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
             1. Hardware\n\n   - Scanners tested\n   - Receipt printers on the new firmware\n\
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
        assert!(text.contains("Quotes stay as written: “fix forward, no rollback” … © Northwind \\<internal\\>."), "{text}");
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
        assert!(p.markdown.contains("\\[not a link\\]\\(javascript:alert\\(1\\)\\)"), "its words are never markup: {}", p.markdown);
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
        assert!(text.contains(r"Raw \<img src=x onerror=alert\(7\)\> and \[x\]\(javascript:alert\(8\)\) and \!\[i\]\(https://evil.example/t.png\) are text"), "{text}");
        assert!(text.contains(r"Direction txt.exe and zerowidth and a NUL \&\#0; and bell are dropped."), "{text:?}");
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
        let at = cpu_time();
        let p = read(&html, "https://contoso.example/");
        assert!(p.truncated);
        let took = cpu_time() - at;
        assert!(took < std::time::Duration::from_secs(20), "{took:?}");
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
        assert!(text.contains(r"Literal \[x\]\(javascript:alert\(1\)\) and \!\[\]\(http://tracker.example/p.png\) stay text, as does \<img src=http://tracker.example/q.png\>."), "{text}");
        assert!(text.contains("\n\\# not a heading\n") && text.contains("\n\\> not a quote\n"), "{text}");
        assert!(text.contains("Wow\\![pic](https://contoso.example/pic.png) and a back\\\\slash."), "the link stays, never an image: {text}");
        assert!(text.contains(r"## Tags \<i\> and \[brackets\]"), "{text}");
        assert!(text.contains(r"| \<td\> \[c\] | `a<b` |"), "code is code, cells are text: {text}");
        assert!(text.ends_with(r"![\]\(javascript:alert\(1\)\) \<b\>](https://contoso.example/p.png)"), "a picture's words can't end it: {text}");
        assert_eq!(md_text("<b>[x]!\\`"), r"\<b\>\[x\]\!\\\`");
        assert_eq!(md_text("~|*_(){}+-=#. 1"), r"\~\|\*\_\(\)\{\}+-=\#. 1", "every mark with a meaning wherever it stands, and no other");
        assert_eq!(md_text("R&D & co &copy; &#38; &x"), r"R&D & co \&copy; \&\#38; &x", "an `&` only where it starts an entity");
        for (line, md) in [("12. x", r"12\. x"), ("12 x. y", "12 x. y"), ("- x", r"\- x"), ("+ x", r"\+ x"), ("===", r"\==="), ("a - b = c + d", "a - b = c + d")] {
            assert_eq!(line_start(line), md, "a line never starts a list, a rule or an underline");
        }
    }

    #[test]
    fn a_huge_list_start_does_not_overflow() {
        let filler = "Filler text so that the page counts as readable for the extractor. ".repeat(4);
        let html = format!("<main><p>{filler}</p><ol start=\"18446744073709551615\"><li>First</li><li>Second</li></ol></main>");
        let text = read(&html, "https://contoso.example/").markdown;
        assert!(text.contains("1000000. First\n1000001. Second"), "a list's start is capped: {text}");
    }

    /// The processor time this thread has used: what `read`'s own work costs, not how long a loaded machine took to
    /// give it that time (a wall clock fails under a parallel build). Elsewhere than macOS, the wall clock.
    fn cpu_time() -> std::time::Duration {
        #[cfg(target_os = "macos")]
        {
            let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
            // SAFETY: clock_gettime only fills the timespec it is given.
            assert_eq!(unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) }, 0);
            std::time::Duration::new(t.tv_sec as u64, t.tv_nsec as u32)
        }
        #[cfg(not(target_os = "macos"))]
        {
            static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
            START.get_or_init(std::time::Instant::now).elapsed()
        }
    }

    /// `read` of `html` on a worker's stack: the page, the processor time it took, the HTML's size.
    fn timed(html: String) -> (Page, std::time::Duration, usize) {
        let n = html.len();
        std::thread::Builder::new()
            .stack_size(2 << 20)
            .spawn(move || {
                let at = cpu_time();
                let p = read(&html, "https://contoso.example/");
                (p, cpu_time() - at, n)
            })
            .unwrap()
            .join()
            .unwrap()
    }

    #[test]
    fn a_table_of_one_wide_row_and_many_empty_ones_stays_small() {
        // One row of 8,000 cells and 8,000 bare rows: each padded to the widest row, 64 million cells.
        let html = format!("<main><table><tr>{}{}</table></main>", "<td>x</td>".repeat(8_000), "<tr>".repeat(8_000));
        let (p, took, n) = timed(html);
        assert!(p.markdown.len() <= 10 * n, "{} bytes of Markdown from {n} of HTML", p.markdown.len());
        assert!(p.truncated && took < std::time::Duration::from_secs(5), "{took:?}");
        let first = p.markdown.lines().next().unwrap();
        assert_eq!(first.matches(" | ").count() + 1, MAX_COLUMNS, "a table keeps {MAX_COLUMNS} columns: {first}");
    }

    #[test]
    fn deeply_nested_numbered_lists_with_many_lines_stay_small() {
        let html = format!("<main>{}<li>{}end</li></main>", "<ol start=\"18446744073709551615\"><li>".repeat(127), "x<br>".repeat(120_000));
        let (p, took, n) = timed(html);
        assert!(p.markdown.len() <= 10 * n, "{} bytes of Markdown from {n} of HTML", p.markdown.len());
        assert!(took < std::time::Duration::from_secs(5), "{took:?}");
        assert!(p.markdown.lines().all(|l| l.len() < 200), "the indent is capped");
    }

    #[test]
    fn inline_code_never_lets_its_text_out() {
        let filler = "Filler text so that the page counts as readable for the extractor. ".repeat(4);
        let html = format!("<main><p>{filler}</p><p>A <code>a``[x](javascript:alert(1))``b</code> and <code>`tick</code> and ``<a href=\"/ok\">ok</a>`` and `<code>[y](javascript:alert(2))</code>.</p></main>");
        let text = read(&html, "https://contoso.example/").markdown;
        assert!(text.contains(r#"A ```a``[x](javascript:alert(1))``b``` and `` `tick `` and \`\`[ok](https://contoso.example/ok)\`\` and \` `[y](javascript:alert(2))`."#), "{text}");
        // Read as CommonMark reads code spans: outside them, every `](` is Echo's own link to the page's site.
        let mut outside = String::new();
        let mut rest = text.as_str();
        while let Some(i) = rest.find('`') {
            if rest[..i].chars().rev().take_while(|c| *c == '\\').count() % 2 == 1 {
                outside.push_str(&rest[..=i]);
                rest = &rest[i + 1..];
                continue;
            }
            outside.push_str(&rest[..i]);
            let run = rest[i..].chars().take_while(|c| *c == '`').count();
            let after = &rest[i + run..];
            // The span ends at the next run of exactly as many backticks; with none, the run is literal.
            let mut at = 0;
            let end = loop {
                let Some(j) = after[at..].find('`') else { break None };
                let j = at + j;
                let len = after[j..].chars().take_while(|c| *c == '`').count();
                if len == run {
                    break Some(j + run);
                }
                at = j + len;
            };
            rest = match end {
                Some(e) => &after[e..],
                None => {
                    outside.push_str(&"`".repeat(run));
                    after
                }
            };
        }
        outside.push_str(rest);
        for (i, _) in outside.match_indices("](") {
            assert!(outside[..i].ends_with('\\') || outside[i + 2..].starts_with("https://contoso.example/"), "a link out of {:?}", &outside[i.saturating_sub(40)..]);
        }
        // Spans side by side: one's closing fence never runs into the next one's opening fence (in a paragraph, a link,
        // a link that is only text, a table's cell, a heading). Read by a CommonMark reader, the only links are Echo's.
        let two = "x <code>a</code><code>`[z](javascript:alert(1))</code> y";
        let html = format!(
            "<main><p>{filler}</p><p>{two}</p><p><a href=/ok>{two}</a></p><p><a href=/ok2>x <code>a</code><code>`](https://evil.example/swap) </code> y</a></p>\
             <p><code>a</code><a href=\"javascript:x\"><code>`[w](javascript:alert(3))</code></a></p>\
             <table><tr><th>Head</th></tr><tr><td>{two}</td></tr></table><h2>{two}</h2></main>"
        );
        let text = read(&html, "https://contoso.example/").markdown;
        let mut links = vec![];
        for ev in pulldown_cmark::Parser::new_ext(&text, pulldown_cmark::Options::ENABLE_TABLES) {
            if let pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link { dest_url, .. } | pulldown_cmark::Tag::Image { dest_url, .. }) = ev {
                links.push(dest_url.to_string());
            }
        }
        assert_eq!(links, ["https://contoso.example/ok", "https://contoso.example/ok2"], "only Echo's links: {text}");
    }

    #[test]
    fn nested_articles_are_measured_once() {
        let html = format!("<html><body>{}<p>{}</p>{}</body></html>", "<article>".repeat(255), "word ".repeat(200_000), "</article>".repeat(255));
        let (p, took, _) = timed(html);
        assert!(p.markdown.starts_with("word word"), "{}", p.markdown.chars().take(40).collect::<String>());
        assert!(took < std::time::Duration::from_secs(5), "{took:?}");
    }

    /// `read` of `html` at `base` on a worker's stack: the page, the processor time it took, its JSON's size.
    fn timed_at(html: String, base: &str) -> (Page, std::time::Duration, usize) {
        let base = base.to_string();
        std::thread::Builder::new()
            .stack_size(2 << 20)
            .spawn(move || {
                let at = cpu_time();
                let p = read(&html, &base);
                let took = cpu_time() - at;
                let size = serde_json::to_string(&p.to_json()).unwrap().len();
                (p, took, size)
            })
            .unwrap()
            .join()
            .unwrap()
    }

    /// Every field of a page read from `n` bytes of HTML within its bounds, and quickly.
    fn bounded(p: &Page, took: std::time::Duration, size: usize, n: usize) {
        assert!(took < std::time::Duration::from_secs(5), "{took:?}");
        assert!(p.markdown.len() <= (10 * n).min(MAX_MD), "{} bytes of Markdown from {n} of HTML", p.markdown.len());
        assert!(p.links.len() <= MAX_LINKS && p.images.len() <= MAX_IMAGES, "{} links, {} pictures", p.links.len(), p.images.len());
        for (text, href) in &p.links {
            assert!(text.chars().count() <= MAX_LINE && href.len() <= MAX_URL, "a link of {} and {} bytes", text.len(), href.len());
        }
        for i in &p.images {
            assert!(i.src.len() <= MAX_URL && i.alt.chars().count() <= MAX_LINE && i.caption.as_ref().is_none_or(|c| c.chars().count() <= MAX_LINE), "a picture of {} bytes", i.src.len() + i.alt.len());
        }
        assert!(size <= MAX_MD + MAX_LINKS * (4 * MAX_LINE + MAX_URL + 64) + MAX_IMAGES * (8 * MAX_LINE + MAX_URL + 128) + 64 * 1024, "{size} bytes of result");
    }

    #[test]
    fn a_figures_caption_is_kept_short_for_each_of_its_pictures() {
        let html = format!("<main><figure>{}<figcaption>{}</figcaption></figure></main>", (0..500).map(|i| format!("<img src=/p{i}.png>")).collect::<String>(), "caption ".repeat(500_000));
        let n = html.len();
        let (p, took, size) = timed_at(html, "https://contoso.example/");
        assert_eq!(p.images.len(), 500);
        assert!(p.images[0].caption.as_ref().is_some_and(|c| c.chars().count() > MAX_LINE - 10 && c.starts_with("caption caption")), "a caption cut to its first {MAX_LINE} characters");
        bounded(&p, took, size, n);
    }

    #[test]
    fn links_inside_a_link_are_not_listed_again() {
        let html = format!("<main>{}<p>{}</p>{}</main>", (0..250).map(|i| format!("<a href=/l{i}>")).collect::<String>(), "word ".repeat(800_000), "</a>".repeat(250));
        let n = html.len();
        let (p, took, size) = timed_at(html, "https://contoso.example/");
        assert_eq!(p.links.len(), 1, "a browser never nests links");
        assert!(p.links[0].0.chars().count() > MAX_LINE - 10 && p.links[0].0.starts_with("word word"), "a link's text cut to its first {MAX_LINE} characters");
        bounded(&p, took, size, n);
    }

    #[test]
    fn many_links_against_a_long_base_stay_within_the_budget_while_written() {
        // A plugin's base of about 2 KB makes each `<a href=x>` of 15 bytes a link of 2 KB.
        let base = format!("https://contoso.example/{}", "d/".repeat(1_000));
        for html in [
            format!("<main><p>{}</p></main>", "<a href=x>y</a> ".repeat(300_000)),
            format!("<main><h2>{}</h2></main>", "<a href=x>y</a> ".repeat(300_000)),
            format!("<main><table><tr><th>h</th></tr>{}</table></main>", "<tr><td><a href=x>y</a>".repeat(200_000)),
        ] {
            let n = html.len();
            let (p, took, size) = timed_at(html, &base);
            // Written while it fits: a heading too long for what is left is left out whole, never cut inside a link.
            assert!(p.truncated, "{}", &p.markdown[..200.min(p.markdown.len())]);
            bounded(&p, took, size, n);
        }
        // An address longer than 2 KB is no link.
        let long = format!("https://contoso.example/{}", "d/".repeat(1_100));
        assert_eq!(web_link(&long, "x"), None);
        assert!(web_link(&base, "x").is_some());
    }

    #[test]
    fn figures_nested_in_captions_are_read_once() {
        let html = format!("<main>{}<p>{}</p>{}</main>", "<figure><img src=/p.png><figcaption>".repeat(250), "word ".repeat(800_000), "</figcaption></figure>".repeat(250));
        let n = html.len();
        let (p, took, size) = timed_at(html, "https://contoso.example/");
        bounded(&p, took, size, n);
    }

    const SITE: &str = "https://acme.example/notes/page";
    /// Addresses a made-up page links to: those that are links (all on the page's site), and those that are only words.
    const GOOD: [&str; 6] = ["/ok", "/a|b", "/p`q", "https://acme.example/z", "//acme.example/w?x=(1)", "/s p"];
    const BAD: [&str; 5] = ["javascript:alert(9)", " JaVaScRiPt:alert(9)", "data:text/html,<b>x</b>", "#top", "mailto:a@acme.example"];

    /// An address as Echo writes one from a page's `href` or `src`: on the page's site (every address these pages link
    /// to is), and as `web_link` leaves it.
    fn echos_own(dest: &str) -> bool {
        dest.starts_with("https://acme.example/") && web_link(SITE, dest).as_deref() == Some(dest)
    }

    /// The Markdown read by a CommonMark reader with what Echo's renderer adds (GFM's tables, strikethrough and task
    /// lists, wiki links): no HTML in it, and every link and picture Echo's own (never `javascript:` or `data:`).
    fn assert_safe(md: &str, html: &str) {
        use pulldown_cmark::{Event, Options, Parser, Tag};
        let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_WIKILINKS;
        for (ev, at) in Parser::new_ext(md, opts).into_offset_iter() {
            match ev {
                Event::Html(h) | Event::InlineHtml(h) => panic!("HTML {h:?} in\n{md}\n\nfrom\n{html}"),
                Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => assert!(echos_own(&dest_url), "a link to {dest_url:?} in\n{md}\n\nfrom\n{html}"),
                // Echo writes no rule and only `#` headings: one of the page's text is its words made structure.
                Event::Rule => panic!("a rule in\n{md}\n\nfrom\n{html}"),
                Event::Start(Tag::Heading { .. }) => assert!(md[at].starts_with('#'), "a setext heading in\n{md}\n\nfrom\n{html}"),
                _ => {}
            }
        }
    }

    #[test]
    fn a_lone_cr_and_an_entity_in_an_address_mean_what_they_say() {
        use pulldown_cmark::{Event, Options, Parser, Tag};
        let filler = "Filler text so that the page counts as readable for the extractor. ".repeat(4);
        // A lone CR in a list item's pre: a line ending to CommonMark, so a line out of the item.
        let html = format!("<main><p>{filler}</p><ul><li>item<pre>a\r[x](javascript:alert(1))\r&lt;img src=x onerror=alert(1)&gt;</pre></li></ul></main>");
        let md = read(&html, SITE).markdown;
        assert!(!md.contains('\r'), "{md:?}");
        assert_safe(&md, &html);
        // `&amp;#64;` in an href: the address is `…&#64;…`, and the Markdown must not decode it to a user part.
        let html = format!("<main><p>{filler} <a href=\"https://acme.example/a&amp;#64;evil.example/x?b=1&amp;c=2\">go</a> <img src=\"https://acme.example/p&amp;amp;q.png\" alt=p width=400></p></main>");
        let page = read(&html, SITE);
        let mut dests = vec![];
        for ev in Parser::new_ext(&page.markdown, Options::empty()) {
            if let Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) = ev {
                dests.push(dest_url.to_string());
            }
        }
        assert_eq!(dests, vec!["https://acme.example/a&#64;evil.example/x?b=1&c=2".to_string(), "https://acme.example/p&amp;q.png".to_string()], "{}", page.markdown);
        assert!(page.markdown.contains("x?b=1&c=2"), "a plain & stays as it is: {}", page.markdown);
    }

    #[test]
    fn the_third_reviews_shapes_stay_text() {
        let filler = "Filler text so that the page counts as readable for the extractor. ".repeat(4);
        let shapes = [
            // A page's `~~~` opening a fence around Echo's, and a `~~~` line in the pre closing it.
            format!("<main><p>{filler}</p><p>~~~</p><pre>~~~\n[x](javascript:alert(1))\n&lt;img src=x onerror=alert(2)&gt;</pre></main>"),
            // A page's `|` and `---` making a table whose cell splits a code span.
            format!("<main><p>{filler}</p><p>h <code>a|[x](javascript:alert(1))</code> | b<br>---|---|---</p></main>"),
            // Nine bare nested lists: an item deeper than the cap whose lines fell out of it.
            format!("<main><p>{filler}</p>{}<ol start=99999999><li><pre>[x](javascript:alert(1))\n&lt;img src=x onerror=alert(2)&gt;</pre></li></ol>{}</main>", "<ul>".repeat(9), "</ul>".repeat(9)),
        ];
        for html in &shapes {
            assert_safe(&read(html, SITE).markdown, html);
        }
        // Noscripts joined past the budget, cut at every size: never inside a code span or a fence.
        let one = format!("<p>{filler} <code>[x](javascript:alert(1))</code> and</p><pre>~~~\n[y](javascript:alert(2))\n&lt;b&gt;</pre>");
        let html = format!("<html><body><script></script>{}</body></html>", format!("<noscript>{one}</noscript>").repeat(4));
        for most in 1..html.len() * 3 {
            let p = read_in(&html, SITE, most);
            assert!(p.markdown.len() <= most, "{} bytes past {most}", p.markdown.len());
            assert_safe(&p.markdown, &html);
        }
    }

    /// A small seeded generator: the same documents every run.
    struct Rng(u64);

    impl Rng {
        fn below(&mut self, n: usize) -> usize {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 33) as usize % n
        }
        fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
            xs[self.below(xs.len())]
        }
    }

    /// Pieces of a page's text that mean something to Markdown, GFM or HTML.
    const HOSTILE: [&str; 77] = [
        "~~~", "~~~\n", "```", "`", "``", "````\n", "|", " | ", "---", "<br>---|---|---<br>", "<br>---|---<br>", "<br>|-|<br>", "<br>- ", "<br>+ ", "<br>===<br>", "<br>---<br>", "&amp;#35;", "&amp;x;", "===", "\n===\n", "***", "[x](javascript:alert(1))", "![x](javascript:alert(2))", "[r]: javascript:alert(5)\n",
        "<http://evil.example/a>", "&lt;img src=x onerror=alert(3)&gt;", "<img src=x onerror=alert(3)>", "<script>alert(4)</script>", "&lt;script&gt;", "&lt;", "&gt;", "&amp;lt;b&amp;gt;", "\\", "\\\\", "**",
        "__", "*", "_", "~~", "1. ", "\n1. ", "1) ", "- ", "\n- ", "+ ", "> ", "\n> ", "# ", "\n# ", "    ", "\n    ", "\t", "&#96;", "&#124;", "&#126;&#126;&#126;", "{", "}", "(", ")", "[", "]", "!", "=", "-", "&",
        "&amp;copy;", "word", " ", "\n", "<br>", "x", "\n12345. ", "[r]", "&lt;!-- x --&gt;", "\r", "\r\n", "a\r[x](javascript:alert(1))\r",
    ];

    fn hostile_text(r: &mut Rng) -> String {
        (0..1 + r.below(6)).map(|_| r.pick(&HOSTILE)).collect()
    }

    fn href(r: &mut Rng) -> &'static str {
        if r.below(3) == 0 {
            r.pick(&BAD)
        } else {
            r.pick(&GOOD)
        }
    }

    /// Inline content: the page's text, links, code (side by side too), emphasis, pictures, line breaks.
    fn gen_inline(r: &mut Rng, depth: usize) -> String {
        let mut s = String::new();
        for _ in 0..1 + r.below(5) {
            match if depth == 0 { 0 } else { r.below(8) } {
                1 => s += &format!("<a href=\"{}\">{}</a>", href(r), gen_inline(r, depth - 1)),
                2 => s += &format!("<code>{}</code>", hostile_text(r)),
                3 => s += &format!("<code>{}</code><code>{}</code>", hostile_text(r), hostile_text(r)),
                4 => s += &format!("<em>{}</em>", gen_inline(r, depth - 1)),
                5 => s += &format!("<img src=\"{}\" alt=\"{}\">", href(r), hostile_text(r).replace('"', "'")),
                6 => s += "<br>",
                _ => s += &hostile_text(r),
            }
        }
        s
    }

    /// Blocks: paragraphs, pre, headings, lists (bare `<ul>` nesting too), tables, figures, quotes, noscript, JSON-LD
    /// and a framework's state.
    fn gen_blocks(r: &mut Rng, depth: usize) -> String {
        let mut s = String::new();
        for _ in 0..1 + r.below(4) {
            let d = depth.saturating_sub(1);
            match if depth == 0 { r.below(2) } else { r.below(13) } {
                0 => s += &format!("<p>{}</p>", gen_inline(r, 2)),
                1 => s += &format!("<pre>{}</pre>", (0..1 + r.below(5)).map(|_| hostile_text(r) + "\n").collect::<String>()),
                2 => {
                    let n = 1 + r.below(6);
                    s += &format!("<h{n}>{}</h{n}>", gen_inline(r, 2));
                }
                3 | 4 => {
                    let (open, close) = if r.below(2) == 0 { ("<ul>".to_string(), "</ul>") } else { (format!("<ol start={}>", ["1", "7", "123456789", "18446744073"][r.below(4)]), "</ol>") };
                    s += &open;
                    for _ in 0..1 + r.below(3) {
                        s += &format!("<li>{}{}", gen_inline(r, 1), gen_blocks(r, d));
                    }
                    s += close;
                }
                5 => {
                    let k = 1 + r.below(12);
                    s += &format!("{}{}{}", "<ul>".repeat(k), gen_blocks(r, d), "</ul>".repeat(k));
                }
                6 => {
                    s += "<table>";
                    for row in 0..1 + r.below(3) {
                        s += "<tr>";
                        for _ in 0..1 + r.below(3) {
                            let c = if row == 0 { "th" } else { "td" };
                            s += &format!("<{c}>{}</{c}>", gen_inline(r, 2));
                        }
                    }
                    s += "</table>";
                }
                7 => s += &format!("<figure><img src=\"{}\"><figcaption>{}</figcaption></figure>", href(r), gen_inline(r, 2)),
                8 => s += &format!("<blockquote>{}</blockquote>", gen_blocks(r, d)),
                9 => s += &format!("<noscript>{}</noscript>", gen_blocks(r, d)),
                10 => s += &format!("<script type=\"application/ld+json\">{{\"@type\":\"Article\",\"articleBody\":{}}}</script>", serde_json::to_string(&gen_blocks(r, d)).unwrap()),
                11 => {
                    let prose = format!("{} {}", "words that read as prose here ".repeat(4), hostile_text(r));
                    s += &format!("<script id=\"__NEXT_DATA__\" type=\"application/json\">{{\"a\":{}}}</script>", serde_json::to_string(&prose).unwrap());
                }
                _ => s += &format!("<div>{}</div>", gen_inline(r, 2)),
            }
        }
        s
    }

    #[test]
    fn no_page_text_becomes_markup_in_any_place_or_cut() {
        // 5,000 made-up pages of hostile pieces in every place, read whole and with budgets small enough to cut them:
        // read as GFM, the Markdown holds no HTML and no link or picture but Echo's own to the page's addresses.
        let mut r = Rng(0x5eed_0003);
        for _ in 0..5_000 {
            let html = if r.below(4) == 0 {
                let noscripts: String = (0..1 + r.below(4)).map(|_| format!("<noscript>{}</noscript>", gen_blocks(&mut r, 2))).collect();
                format!("<html><body><script></script>{noscripts}</body></html>")
            } else {
                format!("<html><body><main>{}</main></body></html>", gen_blocks(&mut r, 3))
            };
            let most = match r.below(3) {
                0 => html.len().saturating_mul(10).min(MAX_MD),
                1 => 1 + r.below(300),
                _ => 1 + r.below(html.len() * 2),
            };
            let p = read_in(&html, SITE, most);
            assert!(p.markdown.len() <= most, "{} bytes past {most}", p.markdown.len());
            assert_safe(&p.markdown, &html);
            assert!(p.links.iter().all(|(_, h)| echos_own(h)) && p.images.iter().all(|i| echos_own(&i.src)), "{:?} {:?}", p.links, p.images);
        }
    }
}
