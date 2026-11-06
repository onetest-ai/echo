//! Frontmatter: the YAML block at the top of a page, read for the subset Obsidian writes — scalars, quoted strings,
//! inline and block lists. Anything else is kept as raw text, never guessed at (no YAML crate; Echo never rewrites a
//! user's frontmatter).

/// A property's value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "t", content = "v", rename_all = "lowercase")]
pub enum Prop {
    Text(String),
    List(Vec<String>),
    /// A value Echo doesn't read (a mapping, a block scalar…), as written.
    Raw(String),
}

/// A page's properties, in their order. A frontmatter Echo can't read at all is one entry with an empty key.
pub type Props = Vec<(String, Prop)>;

/// The frontmatter (without its `---` lines) and the body after it.
pub fn split(text: &str) -> (Option<&str>, &str) {
    let Some(rest) = text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n")) else { return (None, text) };
    let mut at = 0;
    for line in rest.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\r', '\n']);
        if bare == "---" || bare == "..." {
            let fm = rest[..at].trim_end_matches(['\r', '\n']);
            return (Some(fm), &rest[at + line.len()..]);
        }
        at += line.len();
    }
    (None, text)
}

/// A value without its quotes: `"…"` (with `\"` and `\\`), `'…'` (with `''`). `None`: a quote never closed.
fn unquote(v: &str) -> Option<String> {
    if let Some(inner) = v.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => out.push(match chars.next()? {
                    'n' => '\n',
                    't' => '\t',
                    other => other,
                }),
                '"' => {
                    let rest = chars.as_str();
                    return (rest.trim().is_empty() || comment_only(rest)).then_some(out);
                }
                c => out.push(c),
            }
        }
        return None;
    }
    if let Some(inner) = v.strip_prefix('\'') {
        let mut out = String::new();
        let mut rest = inner;
        loop {
            let i = rest.find('\'')?;
            out.push_str(&rest[..i]);
            rest = &rest[i + 1..];
            if let Some(r) = rest.strip_prefix('\'') {
                out.push('\'');
                rest = r;
            } else {
                return (rest.trim().is_empty() || comment_only(rest)).then_some(out);
            }
        }
    }
    Some(strip_comment(v).to_string())
}

fn comment_only(s: &str) -> bool {
    s.trim_start().starts_with('#')
}

/// A plain scalar without a ` # comment` after it.
fn strip_comment(v: &str) -> &str {
    v.find(" #").map_or(v, |i| &v[..i]).trim()
}

/// `[a, "b, c", 'd']` → its items; `None` when it isn't one well-formed inline list.
fn inline_list(v: &str) -> Option<Vec<String>> {
    let inner = v.strip_prefix('[')?;
    let end = inner.rfind(']')?;
    if !(inner[end + 1..].trim().is_empty() || comment_only(&inner[end + 1..])) {
        return None;
    }
    let inner = &inner[..end];
    let mut items = vec![];
    let (mut cur, mut quote) = (String::new(), None::<char>);
    for c in inner.chars() {
        match (quote, c) {
            (None, '"' | '\'') => {
                quote = Some(c);
                cur.push(c);
            }
            (Some(q), c) if c == q => {
                quote = None;
                cur.push(c);
            }
            (None, ',') => {
                items.push(std::mem::take(&mut cur));
            }
            (_, c) => cur.push(c),
        }
    }
    if quote.is_some() {
        return None;
    }
    if !cur.trim().is_empty() || !items.is_empty() {
        items.push(cur);
    }
    items.iter().map(|i| unquote(i.trim())).filter(|i| i.as_ref().is_none_or(|s| !s.is_empty())).collect()
}

/// Read a frontmatter block.
pub fn parse(raw: &str) -> Props {
    let broken = || vec![(String::new(), Prop::Raw(raw.to_string()))];
    let mut out: Props = vec![];
    // The key being read and the indented lines under it.
    let mut open: Option<(String, String, Vec<String>)> = None;
    let close = |open: Option<(String, String, Vec<String>)>, out: &mut Props| -> bool {
        let Some((key, value, under)) = open else { return true };
        let value = value.trim();
        let prop = if !under.is_empty() {
            let items: Option<Vec<String>> = under.iter().map(|l| l.strip_prefix("- ").or_else(|| (l == "-").then_some("")).map(|x| x.trim().to_string())).collect();
            match (value, items) {
                ("", Some(items)) => match items.iter().map(|i| unquote(i)).collect::<Option<Vec<_>>>() {
                    Some(v) => Prop::List(v),
                    None => return false,
                },
                // A block scalar: `|` keeps its lines, `>` folds them into one.
                ("|" | "|-" | "|+", _) => Prop::Text(under.join("\n")),
                (">" | ">-" | ">+", _) => Prop::Text(under.join(" ")),
                // A plain scalar that goes on over indented lines.
                (v, None) if !v.is_empty() && !v.starts_with(['{', '[', '"', '\'', '&', '*', '!']) && !v.ends_with(':') => Prop::Text(format!("{v} {}", under.join(" "))),
                (v, _) => Prop::Raw(if v.is_empty() { under.join("\n") } else { format!("{v}\n{}", under.join("\n")) }),
            }
        } else if value.starts_with('[') {
            match inline_list(value) {
                Some(v) => Prop::List(v),
                None => return false,
            }
        } else if value.starts_with('{') || value.starts_with('|') || value.starts_with('>') {
            Prop::Raw(value.to_string())
        } else {
            match unquote(value) {
                Some(v) => Prop::Text(v),
                None => return false,
            }
        };
        out.push((key, prop));
        true
    };
    for line in raw.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') || line.starts_with("- ") {
            match &mut open {
                Some((_, _, under)) => under.push(line.trim().to_string()),
                None => return broken(),
            }
            continue;
        }
        let Some((key, value)) = line.split_once(':') else { return broken() };
        if key.trim().is_empty() || key.contains(['[', ']', '{', '}', '"', '\'']) || (!value.is_empty() && !value.starts_with(' ')) {
            return broken();
        }
        if !close(open.take(), &mut out) {
            return broken();
        }
        open = Some((key.trim().to_string(), value.to_string(), vec![]));
    }
    if !close(open.take(), &mut out) {
        return broken();
    }
    out
}

/// The value of `key` (case-insensitive), if any.
pub fn get<'a>(props: &'a Props, key: &str) -> Option<&'a Prop> {
    props.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)
}

impl Prop {
    /// A text value, or a list's items joined with ", ".
    pub fn text(&self) -> String {
        match self {
            Prop::Text(s) | Prop::Raw(s) => s.clone(),
            Prop::List(v) => v.join(", "),
        }
    }
    /// A list's items, or a text value as one item (`tags: client`), or nothing.
    pub fn items(&self) -> Vec<String> {
        match self {
            Prop::List(v) => v.clone(),
            Prop::Text(s) if !s.is_empty() => vec![s.clone()],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Prop {
        Prop::Text(s.into())
    }
    fn list(v: &[&str]) -> Prop {
        Prop::List(v.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn frontmatter_is_split_from_the_body() {
        let (fm, body) = split("---\ntitle: A\n---\n# A\n");
        assert_eq!(fm, Some("title: A"));
        assert_eq!(body, "# A\n");
        let (fm, body) = split("---\r\ntitle: A\r\n---\r\nbody");
        assert_eq!(fm, Some("title: A"));
        assert_eq!(body, "body");
        assert_eq!(split("# No frontmatter\n---\n"), (None, "# No frontmatter\n---\n"));
        assert_eq!(split("---\nnever closed\n"), (None, "---\nnever closed\n"), "an unclosed block is body");
        assert_eq!(split("---\n---\nbody"), (Some(""), "body"));
    }

    #[test]
    fn frontmatter_subset_parses() {
        let p = parse("title: Northwind Retail\ndate: 2026-06-12\ntags: [client, \"a, b\", 'c']\nsummary: \"Has: a colon # and hash\"\nparticipants:\n  - Dana Reyes\n  - \"Sam Hale\"\nempty:\nquote: 'it''s'");
        assert_eq!(p, vec![
            ("title".into(), text("Northwind Retail")),
            ("date".into(), text("2026-06-12")),
            ("tags".into(), list(&["client", "a, b", "c"])),
            ("summary".into(), text("Has: a colon # and hash")),
            ("participants".into(), list(&["Dana Reyes", "Sam Hale"])),
            ("empty".into(), text("")),
            ("quote".into(), text("it's")),
        ]);
    }

    #[test]
    fn values_echo_does_not_read_stay_raw() {
        let p = parse("title: A\nmeta:\n  owner: x\n  team: y\nnotes: |\n  line one\n  line two");
        assert_eq!(p[0], ("title".into(), text("A")));
        assert_eq!(p[1], ("meta".into(), Prop::Raw("owner: x\nteam: y".into())));
        assert_eq!(p[2], ("notes".into(), text("line one\nline two")), "a literal block is text, its lines kept");
    }

    #[test]
    fn block_and_multi_line_scalars_are_text() {
        let p = parse("a: >\n  Folded into\n  one line.\nb: >-\n  Also\n  folded.\nc: Plain words\n  that go on.\nd: |-\n  kept\n  lines");
        assert_eq!(p[0], ("a".into(), text("Folded into one line.")));
        assert_eq!(p[1], ("b".into(), text("Also folded.")));
        assert_eq!(p[2], ("c".into(), text("Plain words that go on.")));
        assert_eq!(p[3], ("d".into(), text("kept\nlines")));
    }

    #[test]
    fn a_broken_block_is_kept_whole() {
        let raw = "title: A\nthis line has no key\n";
        assert_eq!(parse(raw), vec![(String::new(), Prop::Raw(raw.into()))]);
        let raw = "title: \"never closed";
        assert_eq!(parse(raw), vec![(String::new(), Prop::Raw(raw.into()))]);
    }

    #[test]
    fn inline_comments_and_cyrillic() {
        let p = parse("title: Клиент «Север» # a comment\ntags: [встреча]");
        assert_eq!(p[0], ("title".into(), text("Клиент «Север»")));
        assert_eq!(p[1], ("tags".into(), list(&["встреча"])));
    }
}
