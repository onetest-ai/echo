//! What a plugin offers other plugins and Claude in Echo (`ctx.expose`): found by a dry run of its `apply(ctx)` on a
//! `ctx` whose services do nothing, kept per installed version, read without starting the plugin.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use rquickjs::{CatchResultExt, Context, Function, Module, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::PRELUDE;
use crate::{paths, plugins};

/// How long a dry run's JavaScript may take.
const DRY: Duration = Duration::from_secs(5);
/// A version whose dry run ran over this many times in a row is taken to offer nothing (until it or Echo changes).
const TRIES: u64 = 3;
const MAX_COMMANDS: usize = 20;
const MAX_ABOUT: usize = 200;
const MAX_COMMAND_ABOUT: usize = 300;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Offer {
    pub description: String,
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Command {
    pub name: String,
    pub description: String,
    /// None: its arguments aren't known (any JSON).
    pub args: Option<Vec<Arg>>,
    #[serde(default)]
    pub reads: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Arg {
    pub name: String,
    #[serde(default)]
    pub optional: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
}

/// What a dry run found: the offer (None: no `ctx.expose`) and what was wrong, each a line for its author.
#[derive(Debug, Default)]
pub struct Found {
    pub offer: Option<Offer>,
    pub problems: Vec<String>,
    /// It ran over its time: tried again next time (`TRIES`).
    pub over: bool,
}

/// What the enabled plugin `id` offers (None: it offers nothing, isn't installed and on, or has no host half).
pub fn of(id: &str) -> Option<Offer> {
    let p = plugins::enabled().into_iter().find(|p| p.id == id)?;
    let file = record_file(id);
    // This Echo's version too: a change to the dry run finds again what plugins offer.
    let key = format!("{}:{}:{}", env!("CARGO_PKG_VERSION"), p.commit, p.files);
    let kept = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).filter(|r| r["key"] == key.as_str());
    if let Some(r) = kept.as_ref().filter(|r| r.get("offer").is_some()) {
        return serde_json::from_value(r["offer"].clone()).ok().flatten();
    }
    let found = discover(id, &plugins::plugin_dir(&p));
    for e in &found.problems {
        eprintln!("plugins: {id}: {e}");
    }
    if let Some(d) = file.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let over = kept.and_then(|r| r["over"].as_u64()).unwrap_or(0) + 1;
    let record = if found.over && over < TRIES { json!({"key": key, "over": over}) } else { json!({"key": key, "offer": found.offer}) };
    let _ = paths::write_atomic(&file, record.to_string().as_bytes());
    found.offer
}

/// Whether `id`'s offered command `name` is marked `reads`.
pub fn reads(id: &str, name: &str) -> bool {
    of(id).is_some_and(|o| o.commands.iter().any(|c| c.name == name && c.reads))
}

/// Every enabled plugin that offers itself, by id.
pub fn all() -> Vec<(String, Offer)> {
    plugins::enabled().into_iter().filter_map(|p| of(&p.id).map(|o| (p.id, o))).collect()
}

/// How many calls through offers may be in flight at once: a loop between plugins (A → B → A …) stops here.
const MAX_IN_FLIGHT: usize = 16;
static IN_FLIGHT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn name_of(id: &str) -> String {
    plugins::enabled().into_iter().find(|p| p.id == id).map(|p| p.name).unwrap_or_else(|| id.to_string())
}

/// The plugins that offer themselves: [{id, name, description}].
pub fn listing() -> Value {
    json!(all().into_iter().map(|(id, o)| json!({"id": id, "name": name_of(&id), "description": o.description})).collect::<Vec<_>>())
}

fn offer(id: &str) -> Result<Offer> {
    of(id).ok_or_else(|| anyhow!("no plugin “{id}” offers commands (it isn't installed and on, or offers nothing)"))
}

/// One plugin: {id, name, description, commands: [{name, description, reads}]}.
pub fn detail(id: &str) -> Result<Value> {
    let o = offer(id)?;
    let commands: Vec<Value> = o.commands.iter().map(|c| json!({"name": c.name, "description": c.description, "reads": c.reads})).collect();
    Ok(json!({"id": id, "name": name_of(id), "description": o.description, "commands": commands}))
}

/// One command: {plugin, name, description, reads, args: [{name, optional, default?, about?}] or null (any JSON)}.
pub fn command_detail(id: &str, name: &str) -> Result<Value> {
    let o = offer(id)?;
    let c = o.commands.iter().find(|c| c.name == name).ok_or_else(|| anyhow!("{id} offers no command “{name}”"))?;
    Ok(json!({"plugin": id, "name": c.name, "description": c.description, "reads": c.reads, "args": c.args}))
}

/// Call `id`'s offered command `name` for `from` (a plugin's id or `claude`); `reading`: only a command marked `reads`.
pub fn call(id: &str, name: &str, args: &Value, from: &str, reading: bool) -> Result<Value> {
    let o = offer(id)?;
    let c = o.commands.iter().find(|c| c.name == name).ok_or_else(|| anyhow!("{id} offers no command “{name}”"))?;
    if reading && !c.reads {
        anyhow::bail!("{id}'s “{name}” may change things, and this run only reads: only commands marked reads run here");
    }
    let args = if args.is_null() { json!({}) } else { args.clone() };
    if let Some(list) = &c.args {
        let Some(given) = args.as_object() else { anyhow::bail!("{id}'s “{name}” takes an object of arguments") };
        if let Some(a) = list.iter().find(|a| !a.optional && !given.contains_key(&a.name)) {
            anyhow::bail!("{id}'s “{name}” needs “{}”", a.name);
        }
    }
    if IN_FLIGHT.fetch_add(1, Ordering::SeqCst) >= MAX_IN_FLIGHT {
        IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        anyhow::bail!("more than {MAX_IN_FLIGHT} calls between plugins are waiting at once (do two plugins call each other?): {id}'s “{name}” isn't called");
    }
    let out = super::call_from(id, name, &args, from, super::CALL_TIMEOUT);
    IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
    out
}

fn record_file(id: &str) -> PathBuf {
    paths::config_dir().join("plugins").join("offers").join(format!("{id}.json"))
}

/// Run the package's `apply(ctx)` with services that do nothing (`ctx.terminal` true, so start-up work is skipped) and
/// read what it offered. Anything that fails leaves what was offered before it; a package that can't run offers nothing.
pub fn discover(id: &str, dir: &Path) -> Found {
    let mut found = Found::default();
    // A plugin that offers nothing is told nothing: most never call ctx.expose, and their apply may well fail on a
    // `ctx` whose services do nothing.
    let raw = match dry_run(id, dir) {
        Ok((raw, err)) => {
            if let (Some(e), true) = (err, raw["description"].is_string()) {
                found.problems.push(format!("apply(ctx) failed in Echo's dry run, so only what it offered before that counts (call ctx.expose first in apply): {e}"));
            }
            raw
        }
        Err(e) => {
            if e.to_string().contains("ran over") {
                found.over = true;
                found.problems.push(format!("Echo's dry run of apply(ctx) didn't end: {e:#}"));
            }
            return found;
        }
    };
    let Some(desc) = raw["description"].as_str() else { return found };
    let desc = desc.trim();
    if desc.is_empty() || desc.chars().count() > MAX_ABOUT {
        found.problems.push(format!("ctx.expose needs a description of 1 to {MAX_ABOUT} characters: what the plugin is for"));
        return found;
    }
    let mut commands = vec![];
    for c in raw["commands"].as_array().into_iter().flatten() {
        let name = c["name"].as_str().unwrap_or("");
        match command(c) {
            Ok(_) if commands.len() == MAX_COMMANDS => found.problems.push(format!("“{name}” left out: at most {MAX_COMMANDS} offered commands")),
            Ok(cmd) => commands.push(cmd),
            Err(e) => found.problems.push(format!("“{name}” left out: {e}")),
        }
    }
    found.offer = Some(Offer { description: desc.to_string(), commands });
    found
}

fn command(c: &Value) -> Result<Command> {
    let name = c["name"].as_str().unwrap_or("");
    if name.is_empty() || name.len() > 40 || name.starts_with("__") || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        anyhow::bail!("a name is 1 to 40 letters, digits, - or _, not starting with __");
    }
    let description = c["description"].as_str().unwrap_or("").trim().to_string();
    if description.is_empty() || description.chars().count() > MAX_COMMAND_ABOUT {
        anyhow::bail!("its description (when to call it) is 1 to {MAX_COMMAND_ABOUT} characters");
    }
    let mut args = params(c["source"].as_str().unwrap_or(""));
    if let Some(given) = c["args"].as_object() {
        let list = args.get_or_insert_with(Vec::new);
        for (k, v) in given {
            let about = v.as_str().map(|s| s.chars().take(MAX_ABOUT).collect::<String>());
            match list.iter_mut().find(|a| a.name == *k) {
                Some(a) => a.about = about,
                // Named only here: whether it is needed isn't known, so Echo doesn't ask for it.
                None => list.push(Arg { name: k.clone(), optional: true, default: None, about }),
            }
        }
    }
    Ok(Command { name: name.to_string(), description, args, reads: c["reads"].as_bool() == Some(true) })
}

/// The JavaScript, run apart from the plugins' host: `__offers()` after `apply`, and `apply`'s error if it threw.
fn dry_run(id: &str, dir: &Path) -> Result<(Value, Option<String>)> {
    let m = plugins::read_manifest(dir)?;
    let main = m.main.ok_or_else(|| anyhow!("no host half"))?;
    let file = plugins::inside_plugin(dir, &main)?;
    let source = std::fs::read_to_string(&file).map_err(|e| anyhow!("{main}: {e}"))?;
    let rt = Runtime::new().map_err(|e| anyhow!("{e}"))?;
    rt.set_memory_limit(32 << 20);
    let started = Instant::now();
    let over = Arc::new(AtomicBool::new(false));
    {
        let over = over.clone();
        rt.set_interrupt_handler(Some(Box::new(move || {
            let o = started.elapsed() > DRY;
            if o {
                over.store(true, Ordering::Relaxed);
            }
            o
        })));
    }
    let ctx = Context::full(&rt).map_err(|e| anyhow!("{e}"))?;
    let id = id.to_string();
    let out = ctx.with(|c| -> Result<(Value, Option<String>)> {
        let g = c.globals();
        let host = Function::new(c.clone(), move |op: String, _arg: String| -> String {
            match op.as_str() {
                "id" => json!({"value": id}),
                "terminal" => json!({"value": true}),
                _ => json!({"value": null}),
            }
            .to_string()
        })
        .map_err(|e| anyhow!("{e}"))?;
        g.set("__host", host).map_err(|e| anyhow!("{e}"))?;
        // Nothing it starts ever settles.
        g.set("__hostAsync", Function::new(c.clone(), |_op: String, _arg: String| -> u64 { 0 }).map_err(|e| anyhow!("{e}"))?).map_err(|e| anyhow!("{e}"))?;
        c.eval::<(), _>(PRELUDE).catch(&c).map_err(|e| anyhow!("prelude: {e}"))?;
        let module = Module::declare(c.clone(), main.clone(), source).catch(&c).map_err(|e| anyhow!("{main}: {e}"))?;
        let (module, promise) = module.eval().catch(&c).map_err(|e| anyhow!("{main}: {e}"))?;
        promise.finish::<()>().catch(&c).map_err(|e| anyhow!("{main}: {e}"))?;
        let apply: Function = module.get("apply").map_err(|_| anyhow!("{main} exports no apply(ctx)"))?;
        let pctx: rquickjs::Value = g.get("__ctx").map_err(|e| anyhow!("{e}"))?;
        let err = apply.call::<_, rquickjs::Value>((pctx,)).catch(&c).err().map(|e| e.to_string());
        let read: Function = g.get("__offers").map_err(|e| anyhow!("{e}"))?;
        let text: String = read.call(()).map_err(|e| anyhow!("{e}"))?;
        Ok((serde_json::from_str(&text)?, err))
    });
    if over.load(Ordering::Relaxed) {
        return Err(anyhow!("it ran over {} s", DRY.as_secs()));
    }
    out
}

/// The names in a function's first parameter when it is destructured (`({query, limit = 10}) => …`): Some([]) when it
/// takes none, None when they can't be told (`(args) => …`).
pub fn params(src: &str) -> Option<Vec<Arg>> {
    // A bound or built-in function shows no source.
    if src.contains("[native code]") {
        return None;
    }
    let s = src.trim_start();
    let s = s.strip_prefix("async").map(str::trim_start).unwrap_or(s);
    let s = match s.strip_prefix("function") {
        Some(rest) => &rest[rest.find('(')?..],
        None => s,
    };
    if !s.starts_with('(') {
        return None;
    }
    let inner = balanced(s)?;
    let first = split_top(inner).into_iter().next().unwrap_or_default();
    let first = first.trim();
    if first.is_empty() {
        return Some(vec![]);
    }
    if !first.starts_with('{') {
        return None;
    }
    // `({a} = {}) => …`: the whole object may be left out, so each name in it may be.
    let inner = balanced(first)?;
    let whole_default = first[inner.len() + 2..].trim_start().starts_with('=');
    let mut out = vec![];
    for part in split_top(inner) {
        let part = part.trim();
        if part.is_empty() || part.starts_with("...") {
            continue;
        }
        let (lhs, default) = match top_eq(part) {
            Some(i) => (&part[..i], Some(part[i + 1..].trim())),
            None => (part, None),
        };
        let name = lhs.split(':').next().unwrap_or("").trim().trim_matches(|c| c == '\'' || c == '"');
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$') {
            continue;
        }
        let default = default.map(|d| d.chars().take(40).collect::<String>());
        out.push(Arg { name: name.to_string(), optional: default.is_some() || whole_default, default, about: None });
    }
    Some(out)
}

/// The text inside the bracket `s` opens with, up to its match (strings skipped).
fn balanced(s: &str) -> Option<&str> {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, ch) in s.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[1..i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// `s` split at its top-level commas.
fn split_top(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth, mut quote, mut escaped) = (vec![], String::new(), 0i32, None::<char>, false);
    for ch in s.chars() {
        if let Some(q) = quote {
            cur.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    out.push(cur);
    out
}

/// Where a top-level `=` (a default, not `==` or `=>`) is in `s`.
fn top_eq(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut depth = 0i32;
    for (i, &c) in b.iter().enumerate() {
        match c {
            b'(' | b'{' | b'[' => depth += 1,
            b')' | b'}' | b']' => depth -= 1,
            b'=' if depth == 0 && b.get(i + 1) != Some(&b'=') && b.get(i + 1) != Some(&b'>') && (i == 0 || !b"=!<>".contains(&b[i - 1])) => return Some(i),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn names(src: &str) -> Option<Vec<(String, bool, Option<String>)>> {
        params(src).map(|a| a.into_iter().map(|a| (a.name, a.optional, a.default)).collect())
    }

    /// A made-up marketplace with the plugins `(id, main.js)`, each installed and on.
    pub(crate) fn install(root: &Path, list: &[(&str, &str)]) {
        install_with(root, &list.iter().map(|(id, m)| (*id, json!({}), *m)).collect::<Vec<_>>())
    }

    /// The same, each with its `uses`.
    fn install_with(root: &Path, list: &[(&str, Value, &str)]) {
        let mk = root.join("market");
        for (id, uses, main) in list {
            let d = mk.join("plugins").join(id);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("echo-plugin.json"), json!({"id": id, "name": id, "version": "0.1.0", "main": "main.js", "uses": uses}).to_string()).unwrap();
            std::fs::write(d.join("main.js"), main).unwrap();
        }
        let ps: Vec<Value> = list.iter().map(|(id, _, _)| json!({"id": id, "source": format!("./plugins/{id}")})).collect();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": ps}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        for (id, _, _) in list {
            let agreed = plugins::review(id, Some(&name)).unwrap().uses;
            plugins::install(id, Some(&name), &agreed).unwrap();
        }
    }

    #[test]
    fn a_plugin_offers_only_what_it_exposes_found_without_starting_it() {
        let (_env, root) = paths::testenv::folders("offers");
        install(&root, &[
            ("vaults", r#"
              export function apply(ctx) {
                if (!ctx.terminal) ctx.store.write('started.txt', 'x')
                ctx.store.write('always.txt', 'x')
                ctx.command('refresh', () => 1)
                const api = ctx.expose('Search and links across the vaults')
                api.command('search', async ({query, limit = 10}) => [query, limit], {description: 'Find notes by words', reads: true, args: {query: 'what to find'}})
                api.command('backlinks', (a) => a, {description: 'Who links to a note'})
                api.command('nameless', () => 1, {})
                api.command('__hidden', () => 1, {description: 'x'})
                try { api.command('refresh', () => 2, {description: 'again'}) } catch (e) { ctx.log(e.message) }
                ctx.settings.get('x').y
                api.command('late', () => 1, {description: 'after the throw'})
              }"#),
            ("quiet", "export function apply(ctx) { ctx.command('ping', () => 'pong') }"),
            ("broken", "export function apply(ctx) { const api = ctx.expose('x'); api.command('spin', () => 1, {description: 'd'}); while (true) {} }"),
        ]);
        let found = discover("vaults", &plugins::plugin_dir(&plugins::enabled().into_iter().find(|p| p.id == "vaults").unwrap()));
        let o = found.offer.clone().unwrap();
        assert_eq!(o.description, "Search and links across the vaults");
        assert_eq!(o.commands.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["search", "backlinks"]);
        let search = &o.commands[0];
        assert!(search.reads);
        assert_eq!(search.args.clone().unwrap(), vec![
            Arg { name: "query".into(), optional: false, default: None, about: Some("what to find".into()) },
            Arg { name: "limit".into(), optional: true, default: Some("10".into()), about: None },
        ]);
        assert_eq!((o.commands[1].args.clone(), o.commands[1].reads), (None, false));
        // Each problem said for the author: no description, Echo's own name, apply's throw.
        assert_eq!(found.problems.len(), 3, "{:?}", found.problems);
        assert!(found.problems.iter().any(|p| p.starts_with("“nameless” left out")) && found.problems.iter().any(|p| p.starts_with("“__hidden” left out")));
        assert!(found.problems.iter().any(|p| p.contains("only what it offered before that counts")));
        // The dry run started nothing of it.
        assert!(!plugins::data_dir("vaults").join("started.txt").exists());
        assert!(!plugins::data_dir("vaults").join("always.txt").exists(), "its services did nothing");
        // Kept per installed version, read again without a run.
        assert_eq!(of("vaults"), Some(o));
        assert!(record_file("vaults").exists());
        assert_eq!(of("quiet"), None);
        // A host half that never ends its apply offers nothing, and says why; tried again, and after TRIES runs over in
        // a row it is left alone (kept as offering nothing) until it or Echo changes.
        let b = discover("broken", &plugins::plugin_dir(&plugins::enabled().into_iter().find(|p| p.id == "broken").unwrap()));
        assert!(b.offer.is_none() && b.over && b.problems[0].contains("ran over"), "{:?}", b.problems);
        let kept = || serde_json::from_slice::<Value>(&std::fs::read(record_file("broken")).unwrap()).unwrap();
        for n in 1..TRIES {
            assert_eq!(of("broken"), None);
            assert_eq!((kept()["over"].as_u64(), kept().get("offer").is_some()), (Some(n), false), "not kept yet: tried again");
        }
        assert_eq!(of("broken"), None);
        assert!(kept().get("offer").is_some(), "{}", kept());
        assert_eq!(all().into_iter().map(|(id, _)| id).collect::<Vec<_>>(), ["vaults"]);
        // Switched off: offers nothing.
        plugins::set_enabled("vaults", false).unwrap();
        assert_eq!(of("vaults"), None);
    }

    #[test]
    fn a_plugin_that_asked_calls_what_another_offers_and_is_told_who_called() {
        let (_env, root) = paths::testenv::folders("offers-call");
        install_with(&root, &[
            ("vaults", json!({}), r#"
              export function apply(ctx) {
                const api = ctx.expose('Search across the vaults')
                api.command('search', ({query, limit = 2}, how) => ({query, limit, from: how.from || null}), {description: 'Find notes by words', reads: true})
                api.command('echo', (a) => a, {description: 'Answers what it was given'})
                api.command('pick', async () => await ctx.system.pickFolder('a vault'), {description: 'Asks the user for a folder'})
                ctx.command('secret', () => 'mine')
              }"#),
            ("ping", json!({"plugins": true}), r#"
              export function apply(ctx) {
                const api = ctx.expose('Ping pong')
                api.command('ping', async () => await ctx.plugins.call('pong', 'pong', {}), {description: 'Calls pong'})
                ctx.command('list', () => ctx.plugins.list())
                ctx.command('get', (a) => ctx.plugins.get(a.id))
                ctx.command('call', (a) => ctx.plugins.call(a.id, a.name, a.args))
              }"#),
            ("pong", json!({"plugins": true}), r#"
              export function apply(ctx) {
                const api = ctx.expose('Pong ping')
                api.command('pong', async () => await ctx.plugins.call('ping', 'ping', {}), {description: 'Calls ping'})
              }"#),
            ("nosy", json!({}), "export function apply(ctx) { ctx.command('call', () => ctx.plugins.call('vaults', 'search', {query: 'x'})) }"),
        ]);
        crate::jshost::sync();
        let call = |p: &str, c: &str, a: Value| (0..50).find_map(|_| match crate::jshost::call(p, c, &a) {
            Err(e) if e.to_string().starts_with("no command") => { std::thread::sleep(Duration::from_millis(50)); None }
            r => Some(r.map_err(|e| e.to_string())),
        }).unwrap();
        let listed = call("ping", "list", json!({})).unwrap();
        assert_eq!(listed.as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap()).collect::<Vec<_>>(), ["ping", "pong", "vaults"]);
        assert_eq!(call("ping", "get", json!({"id": "vaults"})).unwrap()["commands"][0], json!({"name": "search", "description": "Find notes by words", "reads": true}));
        // The call: its default, and the caller's id as `from`.
        assert_eq!(call("ping", "call", json!({"id": "vaults", "name": "search", "args": {"query": "acme"}})).unwrap(), json!({"query": "acme", "limit": 2, "from": "ping"}));
        // An offered command is its plugin's own command too: called directly, no `from`.
        assert_eq!(call("vaults", "search", json!({"query": "q"})).unwrap()["from"], Value::Null);
        let refused = |p: &str, a: Value, why: &str| {
            let e = call(p, "call", a).unwrap_err();
            assert!(e.contains(why), "{e}");
        };
        refused("ping", json!({"id": "vaults", "name": "search", "args": {}}), "needs “query”");
        refused("ping", json!({"id": "vaults", "name": "secret", "args": {}}), "offers no command “secret”");
        refused("ping", json!({"id": "quiet", "name": "x", "args": {}}), "no plugin “quiet”");
        refused("ping", json!({"id": "ping", "name": "ping", "args": {}}), "calls its own commands itself");
        refused("nosy", json!({}), "didn't ask for “plugins”");
        // No one at the screen asked for a call from another plugin: no picker opens in it.
        refused("ping", json!({"id": "vaults", "name": "pick", "args": {}}), "not from a timer, an event or another plugin's or Claude's call");
        // Two plugins calling each other stop at the limit, said.
        let e = call("ping", "ping", json!({})).unwrap_err();
        assert!(e.contains("calls between plugins are waiting at once"), "{e}");
        assert_eq!(IN_FLIGHT.load(Ordering::SeqCst), 0);
        crate::jshost::unload_all();
    }

    #[test]
    fn argument_names_come_from_the_destructured_first_parameter() {
        let s = |n: &str, o: bool, d: Option<&str>| (n.to_string(), o, d.map(str::to_string));
        assert_eq!(names("async ({query, limit = 10}) => 1"), Some(vec![s("query", false, None), s("limit", true, Some("10"))]));
        assert_eq!(names("({ path }) => x"), Some(vec![s("path", false, None)]));
        assert_eq!(names("function ({a, b: c = 'x,y', ...rest}, files) { }"), Some(vec![s("a", false, None), s("b", true, Some("'x,y'"))]));
        assert_eq!(names("async function search({q = {a: 1}}) {}"), Some(vec![s("q", true, Some("{a: 1}"))]));
        assert_eq!(names("({opts} = {}) => 1"), Some(vec![s("opts", true, None)]));
        assert_eq!(names("function bound search() {\n    [native code]\n}"), None);
        assert_eq!(names("() => 1"), Some(vec![]));
        assert_eq!(names("(args) => args.q"), None);
        assert_eq!(names("a => a"), None);
        assert_eq!(names("async (a, {b}) => 1"), None);
    }
}
