//! A small element tree over quick-xml, enough for Office parts: local names only (namespaces
//! dropped), attributes by local name, text with entities resolved.

use anyhow::{anyhow, bail, Result};

/// How deep an Office part may nest.
const MAX_DEPTH: usize = 256;
use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, Default, Clone)]
pub struct El {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub kids: Vec<Node>,
}

#[derive(Debug, Clone)]
pub enum Node {
    El(El),
    Text(String),
}

fn local(n: &str) -> &str {
    n.rsplit_once(':').map(|(_, l)| l).unwrap_or(n)
}

fn element(e: &quick_xml::events::BytesStart) -> El {
    let attrs = e
        .attributes()
        .flatten()
        .map(|a| {
            let k = a.key.as_ref().to_string();
            let v = quick_xml::escape::unescape(&a.value).map(|v| v.into_owned()).unwrap_or_else(|_| a.value.to_string());
            (k, v)
        })
        .collect();
    El { name: local(e.name().as_ref()).to_string(), attrs, kids: vec![] }
}

/// The document's root element.
pub fn parse(s: &str) -> Result<El> {
    let mut r = Reader::from_str(s);
    let mut stack: Vec<El> = vec![El { name: "#doc".into(), ..Default::default() }];
    let push_text = |stack: &mut Vec<El>, t: &str| {
        if t.is_empty() {
            return;
        }
        let top = stack.last_mut().expect("root");
        if let Some(Node::Text(prev)) = top.kids.last_mut() {
            prev.push_str(t);
        } else {
            top.kids.push(Node::Text(t.to_string()));
        }
    };
    loop {
        match r.read_event().map_err(|e| anyhow!("XML: {e}"))? {
            Event::Start(e) => {
                // Real Office parts nest a few dozen deep; a crafted one nesting a million deep would
                // overflow the stack in the recursive walks (and in dropping the tree).
                if stack.len() > MAX_DEPTH {
                    bail!("XML: nested deeper than {MAX_DEPTH} levels");
                }
                stack.push(element(&e))
            }
            Event::Empty(e) => {
                let el = element(&e);
                stack.last_mut().expect("root").kids.push(Node::El(el));
            }
            Event::End(_) if stack.len() > 1 => {
                let el = stack.pop().expect("open element");
                stack.last_mut().expect("root").kids.push(Node::El(el));
            }
            Event::Text(t) => push_text(&mut stack, &t),
            Event::CData(t) => push_text(&mut stack, &t),
            Event::GeneralRef(g) => {
                let s: String = match g.resolve_char_ref() {
                    Ok(Some(c)) => c.to_string(),
                    _ => match g.as_ref() {
                        "amp" => "&".into(),
                        "lt" => "<".into(),
                        "gt" => ">".into(),
                        "quot" => "\"".into(),
                        "apos" => "'".into(),
                        _ => String::new(),
                    },
                };
                push_text(&mut stack, &s);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    while stack.len() > 1 {
        let el = stack.pop().expect("open element");
        stack.last_mut().expect("root").kids.push(Node::El(el));
    }
    let doc = stack.pop().expect("root");
    doc.kids.into_iter().find_map(|k| if let Node::El(e) = k { Some(e) } else { None }).ok_or_else(|| anyhow!("XML: no root element"))
}

impl El {
    /// An attribute by its written name (`r:id`), else by its local name (`show` for `p:show`).
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).or_else(|| self.attrs.iter().find(|(k, _)| local(k) == name)).map(|(_, v)| v.as_str())
    }

    pub fn children(&self) -> impl Iterator<Item = &El> {
        self.kids.iter().filter_map(|k| if let Node::El(e) = k { Some(e) } else { None })
    }

    pub fn child(&self, name: &str) -> Option<&El> {
        self.children().find(|e| e.name == name)
    }

    /// Every element named `name` below this one, in document order (not inside a match).
    pub fn find_all<'a>(&'a self, name: &str, out: &mut Vec<&'a El>) {
        for c in self.children() {
            if c.name == name {
                out.push(c);
            } else {
                c.find_all(name, out);
            }
        }
    }

    pub fn all(&self, name: &str) -> Vec<&El> {
        let mut v = vec![];
        self.find_all(name, &mut v);
        v
    }

    /// How many elements named `name` are below this one (matches inside matches too).
    pub fn count(&self, name: &str) -> usize {
        self.children().map(|c| usize::from(c.name == name) + c.count(name)).sum()
    }

    /// All text below, as written.
    pub fn text(&self) -> String {
        let mut s = String::new();
        self.collect_text(&mut s);
        s
    }

    fn collect_text(&self, s: &mut String) {
        for k in &self.kids {
            match k {
                Node::Text(t) => s.push_str(t),
                Node::El(e) => e.collect_text(s),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_nesting_is_refused() {
        let deep = format!("{}{}", "<a>".repeat(10_000), "</a>".repeat(10_000));
        assert!(parse(&deep).is_err());
        let ok = format!("{}x{}", "<a>".repeat(50), "</a>".repeat(50));
        assert_eq!(parse(&ok).unwrap().text(), "x");
    }

    #[test]
    fn parses_names_attrs_and_entities() {
        let r = parse(r#"<?xml version="1.0"?><p:sld xmlns:p="x" show="0" id="256" r:id="rId2"><a:t>R&amp;D &#8212; ok</a:t><a:br/></p:sld>"#).unwrap();
        assert_eq!(r.name, "sld");
        assert_eq!(r.attr("show"), Some("0"));
        assert_eq!((r.attr("id"), r.attr("r:id")), (Some("256"), Some("rId2")));
        assert_eq!(r.child("t").unwrap().text(), "R&D — ok");
        assert_eq!(r.count("br"), 1);
    }
}
