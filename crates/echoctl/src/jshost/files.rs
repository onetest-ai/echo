//! Files the user hands a plugin — picked in the system's dialog (`ctx.system.pickFiles`), dropped on the window while
//! one of its views is shown (`ctx.onFileDrop`), or named with `echoctl plugin call … --file` — as **handles**:
//! `{file, name, size}`, `file` an opaque id owned by that one plugin, never the path. Kept in this process's memory
//! for an hour (`KEEP`): a file handed over once isn't the plugin's to read again whenever it likes. With a handle the
//! host half reads the text (`ctx.files.read`) or copies the bytes into a folder it was given (`ctx.fs.copyIn`).

use super::*;
use anyhow::Context as _;
use std::io::{Read, Write};

/// How long a handle lasts: long enough for a sheet the user fills before adding, not a lasting grant.
const KEEP: Duration = Duration::from_secs(3600);
/// Handles kept at once (the oldest go first): picks and drops are the user's acts, never this many.
const MOST: usize = 1000;
/// The largest file `copyIn` copies.
pub(super) const MAX_COPY: u64 = 50 << 20;

struct Handle {
    plugin: String,
    path: PathBuf,
    name: String,
    size: u64,
    made: Instant,
}

static HANDLES: Mutex<BTreeMap<String, Handle>> = Mutex::new(BTreeMap::new());

fn handles() -> std::sync::MutexGuard<'static, BTreeMap<String, Handle>> {
    HANDLES.lock().unwrap_or_else(|e| e.into_inner())
}

/// A handle of plugin `plugin` for the file at `path` (the user handed it over): `{file, name, size}`.
pub fn make(plugin: &str, path: &Path) -> Result<Value> {
    let path = dunce::canonicalize(path).map_err(|_| anyhow!("{} isn't there", path.display()))?;
    let m = std::fs::metadata(&path)?;
    if !m.is_file() {
        bail!("{} isn't a file", path.display());
    }
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|e| anyhow!("{e}"))?;
    let file: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut all = handles();
    all.retain(|_, h| h.made.elapsed() < KEEP);
    while all.len() >= MOST {
        let Some(oldest) = all.iter().min_by_key(|(_, h)| h.made).map(|(k, _)| k.clone()) else { break };
        all.remove(&oldest);
    }
    let v = json!({"file": file, "name": name, "size": m.len()});
    all.insert(file, Handle { plugin: plugin.into(), path, name, size: m.len(), made: Instant::now() });
    Ok(v)
}

/// The file behind handle `file` (an id, or a handle `{file}`), when it is plugin `plugin`'s and not past its hour.
/// Another plugin's is refused as one that isn't there: a plugin learns nothing of another's files.
pub(super) fn own(plugin: &str, file: &Value) -> Result<(PathBuf, Value)> {
    let id = file.as_str().or_else(|| file["file"].as_str()).unwrap_or("");
    match handles().get(id) {
        Some(h) if h.plugin == plugin && h.made.elapsed() < KEEP => Ok((h.path.clone(), json!({"file": id, "name": h.name, "size": h.size}))),
        _ => bail!("“{}” isn't a file handed to {plugin} (or it was over an hour ago)", id.chars().take(40).collect::<String>()),
    }
}

/// Handles a caller passes to plugin `plugin`'s command (its window half's `ctx.call(…, {files})`): each checked as its
/// own; the handles as the command sees them.
pub fn own_list(plugin: &str, files: &[Value]) -> Result<Vec<Value>> {
    files.iter().map(|f| own(plugin, f).map(|(_, v)| v)).collect()
}

/// `ctx.files.read(file, {max})`: its text, `max` bytes (1 MB by default, 4 MB at most), as `ctx.fs.read`.
pub(super) fn read(plugin: &str, file: &Value, max: Option<u64>) -> Result<Value> {
    let (path, _) = own(plugin, file)?;
    let max = max.filter(|m| *m > 0).unwrap_or(1 << 20).min(4 << 20);
    let mut buf = vec![];
    crate::grants::open_regular(&path)?.take(max).read_to_end(&mut buf)?;
    Ok(json!(String::from_utf8_lossy(&buf)))
}

/// `ctx.claude.run({images})`: the pictures behind its handles, each checked as `read` checks one, as Claude's image
/// blocks — 10 at most, each a PNG, JPEG, GIF or WebP of 3.75 MB at most (5 MB base64-encoded).
pub(super) fn images(plugin: &str, list: &Value) -> Result<Vec<Value>> {
    let list = match list {
        Value::Null => return Ok(vec![]),
        Value::Array(l) => l,
        _ => bail!("a run's images are a list of file handles"),
    };
    if list.len() > crate::claudejob::IMAGES_MAX {
        bail!("a run takes {} pictures at most, not {}", crate::claudejob::IMAGES_MAX, list.len());
    }
    list.iter()
        .map(|f| {
            let (path, h) = own(plugin, f)?;
            let name = h["name"].as_str().unwrap_or("").to_string();
            let mut buf = vec![];
            crate::grants::open_regular(&path)?.take(crate::claudejob::IMAGE_BYTES + 1).read_to_end(&mut buf)?;
            if buf.len() as u64 > crate::claudejob::IMAGE_BYTES {
                bail!("“{name}” is over 3.75 MB: a run takes pictures up to 3.75 MB (5 MB base64-encoded, what every platform takes)");
            }
            crate::claudejob::image_block(&buf, &name)
        })
        .collect()
}

/// The largest file `ctx.files.text` reads, how long it works on one, what it unpacks at most from an Office file
/// (its parts, summed: a zip bomb stops here) and how many parts such a file holds.
const TEXT_MAX_FILE: u64 = 50 << 20;
const TEXT_TIME: Duration = Duration::from_secs(60);
const TEXT_MAX_UNPACKED: u64 = 200 << 20;
const TEXT_MAX_PARTS: usize = 5000;
/// The most of the reader's answer the host takes (its text is cut at 4 MB, as JSON a little more).
const TEXT_MAX_ANSWER: u64 = 16 << 20;
/// Extractions at once: one a plugin, two in all.
const TEXT_AT_ONCE: usize = 2;

/// `pages` of `ctx.files.text`: a whole number (`3`, `"3"`) or a range (`"2-5"`, `"7-"`: from page 7 on), 1-based.
/// → (from, to), `to` `usize::MAX` for "to the end"
fn page_range(v: &Value) -> Result<Option<(usize, usize)>> {
    let t = match v {
        Value::Null => return Ok(None),
        Value::Number(n) => n.as_u64().map(|n| n.to_string()).unwrap_or_else(|| n.to_string()),
        Value::String(s) => s.trim().to_string(),
        _ => bail!("pages is a number or a range like \"2-5\""),
    };
    let bad = || anyhow!("pages is a whole number or a range like \"2-5\" (1 is the first page), not “{}”", t.chars().take(40).collect::<String>());
    let num = |x: &str| -> Result<usize> { x.trim().parse::<usize>().ok().filter(|n| *n >= 1).ok_or_else(bad) };
    match t.split_once('-') {
        Some((a, "")) => Ok(Some((num(a)?, usize::MAX))),
        Some((a, b)) => {
            let (a, b) = (num(a)?, num(b)?);
            if a > b {
                return Err(bad());
            }
            Ok(Some((a, b)))
        }
        None => num(&t).map(|n| Some((n, n))),
    }
}

/// The text of the file at `path` as Echo's own readers see it (`intake::files`: PDF through pdfium, Word, PowerPoint,
/// spreadsheets, HTML, plain text); a file of another name is text when it has no NUL in its first 8 KB. No model, no
/// network. → the text and the page count of a paged file. Runs in the child process `echoctl files-text`.
#[cfg(feature = "intake")]
fn text_of(path: &Path, range: Option<(usize, usize)>) -> Result<(String, Option<usize>)> {
    use crate::intake::files as bf;
    let file = crate::grants::open_regular(path)?;
    if file.metadata()?.len() > TEXT_MAX_FILE {
        bail!("the file is over 50 MB: a plugin reads the text of files up to 50 MB");
    }
    let ext = bf::ext(path);
    if !bf::FORMATS.contains(&ext.as_str()) {
        if range.is_some() {
            bail!("this file has no pages: leave pages out");
        }
        let mut head = vec![];
        (&file).take(8192).read_to_end(&mut head)?;
        if head.contains(&0) {
            bail!("not a file with text in it (.{ext})");
        }
        let mut all = vec![];
        crate::grants::open_regular(path)?.take(TEXT_MAX_FILE).read_to_end(&mut all)?;
        return Ok((String::from_utf8_lossy(&all).replace("\r\n", "\n"), None));
    }
    if ext == "xls" {
        bail!("an old .xls file isn't read: save it as .xlsx");
    }
    // An Office file is a zip: what it unpacks to, and its parts, are counted before any reader opens it.
    if ["docx", "pptx", "xlsx", "xlsm"].contains(&ext.as_str()) {
        let z = zip::ZipArchive::new(&file).map_err(|e| anyhow!("not a readable Office file (truncated download?): {e}"))?;
        if z.len() > TEXT_MAX_PARTS {
            bail!("the file holds {} parts: a plugin reads Office files of {TEXT_MAX_PARTS} at most", z.len());
        }
        let mut z = z;
        let mut unpacked = 0u64;
        for i in 0..z.len() {
            unpacked = unpacked.saturating_add(z.by_index_raw(i).map(|f| f.size()).unwrap_or(0));
        }
        if unpacked > TEXT_MAX_UNPACKED {
            bail!("the file unpacks to over 200 MB: a plugin reads Office files that unpack to 200 MB at most");
        }
    }
    drop(file);
    let parsed = bf::read(path)?;
    if let (Some(why), true) = (&parsed.skip, parsed.md.is_none() && parsed.pages.is_empty()) {
        bail!("nothing to read in it: {why}");
    }
    if parsed.pages.is_empty() {
        if range.is_some() {
            bail!("this file has no pages: leave pages out");
        }
        return Ok((parsed.md.unwrap_or_default(), None));
    }
    let total = parsed.pages.len();
    let (a, b) = range.unwrap_or((1, usize::MAX));
    if a > total {
        bail!("the file has {total} page{}: pages starts at {a}", if total == 1 { "" } else { "s" });
    }
    let mut out = vec![];
    for p in parsed.pages.iter().filter(|p| p.n >= a && p.n <= b) {
        let mut m = p.text.clone();
        for t in &p.tables {
            m.push_str(&format!("\n\n{t}"));
        }
        out.push(m.trim().to_string());
    }
    Ok((out.join("\n\n"), Some(total)))
}

#[cfg(not(feature = "intake"))]
fn text_of(_: &Path, _: Option<(usize, usize)>) -> Result<(String, Option<usize>)> {
    bail!("this build of Echo has no document readers")
}

/// Cut `text` at `max` bytes, at a character.
fn cut(text: &mut String, max: usize) -> bool {
    if text.len() <= max {
        return false;
    }
    let mut at = max;
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    text.truncate(at);
    true
}

/// `echoctl files-text <path> [--pages A-B] [--max N]`, the child process the host starts for `ctx.files.text`: the
/// answer as one JSON line on stdout, or why not on stderr with exit 1. A reader that crashes (PDFium on a crafted
/// file) takes only this process with it.
pub fn child_main(path: &Path, pages: Option<&str>, max: usize) -> i32 {
    #[cfg(debug_assertions)]
    {
        // (tests) a reader that hangs or crashes.
        if let Some(s) = std::env::var("ECHOCTL_TEST_TEXT_SLEEP").ok().and_then(|s| s.parse::<u64>().ok()) {
            std::thread::sleep(Duration::from_secs(s));
        }
        if std::env::var_os("ECHOCTL_TEST_TEXT_CRASH").is_some() {
            std::process::abort();
        }
    }
    let r = (|| {
        let range = page_range(&pages.map_or(Value::Null, |p| json!(p)))?;
        text_of(path, range)
    })();
    match r {
        Ok((mut text, pages)) => {
            let truncated = cut(&mut text, max);
            println!("{}", json!({"text": text, "pages": pages, "truncated": truncated}));
            0
        }
        Err(e) => {
            eprintln!("{e:#}");
            1
        }
    }
}

static READING: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// A place among the extractions running: one a plugin, `TEXT_AT_ONCE` in all; given back when it drops.
#[derive(Debug)]
struct Reading(String);

impl Reading {
    fn take(plugin: &str) -> Result<Reading> {
        let mut all = READING.lock().unwrap_or_else(|e| e.into_inner());
        if all.iter().any(|p| p == plugin) {
            bail!("{plugin} is reading a file already: wait for it to end");
        }
        if all.len() >= TEXT_AT_ONCE {
            bail!("Echo is reading {TEXT_AT_ONCE} files for plugins already: try again in a moment");
        }
        all.push(plugin.to_string());
        Ok(Reading(plugin.to_string()))
    }
}

impl Drop for Reading {
    fn drop(&mut self) {
        let mut all = READING.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = all.iter().position(|p| *p == self.0) {
            all.remove(i);
        }
    }
}

/// The program that reads: Echo itself (in a unit test, the `echoctl` that sits beside the test program).
pub(super) fn reader_exe() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("Echo's own program can't be found")?;
    // (a unit test) the `echoctl` in the folder of the build above the test program, which `cargo test` has built.
    #[cfg(test)]
    let exe = {
        let name = if cfg!(windows) { "echoctl.exe" } else { "echoctl" };
        exe.ancestors().skip(1).map(|d| d.join(name)).find(|p| p.is_file()).ok_or_else(|| anyhow!("no echoctl beside the test program"))?
    };
    Ok(exe)
}

/// `ctx.files.text(file, {pages, max})` → `{text, pages, truncated}`: the text of a handed file (PDF, Word, PowerPoint,
/// spreadsheet, HTML, Markdown, plain text). `pages` in the options picks one page or a range of a PDF or deck (a file
/// without pages, or a range past its last page, is refused); in the answer it is the file's page count (`null` for a
/// file without pages). `truncated` when `max` (1 MB of text by default, 4 MB at most) cut it. The file is read in a
/// child process of Echo that is killed after 60 s, so a crash or a hang in a reader never reaches Echo; a file over
/// 50 MB, or an Office file unpacking to over 200 MB, is refused.
pub(super) fn text(plugin: &str, file: &Value, opts: &Value) -> Result<Value> {
    let (path, _) = own(plugin, file)?;
    let range = page_range(&opts["pages"])?;
    let max = opts["max"].as_u64().filter(|m| *m > 0).unwrap_or(1 << 20).min(4 << 20);
    let size = crate::grants::open_regular(&path)?.metadata()?.len();
    if size > TEXT_MAX_FILE {
        bail!("the file is {} MB: a plugin reads the text of files up to 50 MB", size.div_ceil(1 << 20));
    }
    let _place = Reading::take(plugin)?;
    let mut cmd = std::process::Command::new(reader_exe()?);
    cmd.arg("files-text").arg(&path).arg("--max").arg(max.to_string());
    if let Some((a, b)) = range {
        cmd.arg("--pages").arg(format!("{a}-{}", if b == usize::MAX { String::new() } else { b.to_string() }));
    }
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    let mut cmd = {
        use crate::app::NoWindow;
        cmd.no_window()
    };
    let mut child = cmd.spawn().context("the reader couldn't start")?;
    let drain = |s: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut b = vec![];
            if let Some(s) = s {
                let _ = s.take(TEXT_MAX_ANSWER).read_to_end(&mut b);
            }
            b
        })
    };
    let out = drain(child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
    let err = drain(child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
    #[cfg(debug_assertions)]
    let limit = std::env::var("ECHOCTL_TEST_TEXT_SECS").ok().and_then(|s| s.parse().ok()).map_or(TEXT_TIME, Duration::from_secs);
    #[cfg(not(debug_assertions))]
    let limit = TEXT_TIME;
    let started = Instant::now();
    let status = loop {
        match child.try_wait()? {
            Some(s) => break s,
            None if started.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("reading the file took over 60 seconds");
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    let (out, err) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
    if status.success() {
        return serde_json::from_slice(&out).map_err(|_| anyhow!("the reader failed: its answer wasn't readable"));
    }
    let said = String::from_utf8_lossy(&err);
    match (status.code(), said.lines().next().map(str::trim).filter(|l| !l.is_empty())) {
        (Some(1), Some(why)) => bail!("{why}"),
        _ => bail!("the reader failed on this file ({status})"),
    }
}

/// `ctx.fs.copyIn(file, root, rel, {createNew})`: the file's bytes, as they are, into `rel` of a folder the plugin was
/// given (`uses.folders.write`, `grants::writable`: never through a link). Never over a file there unless
/// `createNew: false` asked for it. Written whole into a hidden file of its own beside it first (made new: a link
/// planted there isn't followed), then put into place only where nothing is (`paths::place_new`) or, asked, renamed
/// over it.
pub(super) fn copy_in(plugin: &str, file: &Value, root: &str, rel: &str, create_new: bool) -> Result<Value> {
    uses_flag(plugin, "folders", "write")?;
    copy_to(plugin, file, rel, create_new, &|| {
        let to = crate::grants::writable(plugin, root, rel)?;
        if !to.is_dir() && crate::grants::in_skills(root, rel, &to) {
            bail!("“{rel}”: nothing is copied into a skill; a skill is written whole (ctx.fs.write)");
        }
        Ok(to)
    })
}

/// `ctx.store.copyIn(file, rel, {createNew})`: the same into the plugin's own store, where `jshost::writable_store_path`
/// allows a write (never its `settings.json` or Echo's files there, never out of the store).
pub(super) fn copy_in_store(plugin: &str, file: &Value, rel: &str, create_new: bool) -> Result<Value> {
    copy_to(plugin, file, rel, create_new, &|| writable_store_path(plugin, rel))
}

/// The copy: `resolve` names the file to write each time it is asked (again after the folders above it are made).
fn copy_to(plugin: &str, file: &Value, rel: &str, create_new: bool, resolve: &dyn Fn() -> Result<PathBuf>) -> Result<Value> {
    let (src, _) = own(plugin, file)?;
    let to = resolve()?;
    if to.is_dir() {
        bail!("“{rel}” is a folder");
    }
    if create_new && std::fs::symlink_metadata(&to).is_ok() {
        bail!("“{rel}” is there already");
    }
    let mut from = crate::grants::open_regular(&src)?;
    let size = from.metadata()?.len();
    if size > MAX_COPY {
        bail!("the file is {} MB: a plugin copies files up to 50 MB", size.div_ceil(1 << 20));
    }
    let dir = to.parent().ok_or_else(|| anyhow!("“{rel}” has no folder"))?;
    std::fs::create_dir_all(dir)?;
    // The folder made may be reached through a link made meanwhile: checked again as it is now.
    let to = resolve()?;
    static N: AtomicU64 = AtomicU64::new(0);
    let name = to.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = to.with_file_name(format!(".{name}.{}.{}.part", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    let copied = std::io::copy(&mut (&mut from).take(MAX_COPY + 1), &mut out).and_then(|n| out.flush().map(|_| n));
    drop(out);
    let placed = match copied {
        Ok(n) if n > MAX_COPY => Err(anyhow!("the file grew past 50 MB while it was copied")),
        Ok(_) if create_new => paths::place_new(&tmp, &to).map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => anyhow!("“{rel}” is there already"),
            _ => anyhow!("“{rel}”: {e}"),
        }),
        Ok(_) => std::fs::rename(&tmp, &to).map_err(Into::into),
        Err(e) => Err(e.into()),
    };
    let _ = std::fs::remove_file(&tmp);
    placed?;
    Ok(json!({"path": rel.trim_start_matches("./"), "size": size}))
}

/// No native dialog inside a run Echo started (a nightly routine, a minutes run): no one is there to answer it.
pub(super) fn no_picker_in_a_run() -> Result<()> {
    if crate::notes::headless() {
        bail!("a picker opens only for you, not inside a run Echo started");
    }
    Ok(())
}

/// Forget plugin `id`'s handles: switched off, updated or removed, it keeps none of the files it was handed.
pub fn forget(id: &str) {
    handles().retain(|_, h| h.plugin != id);
}

/// `ctx.system.pickFiles(prompt, {multiple, ext})`: the system's file picker (`app::choose_files`), each file picked a
/// handle of this plugin; `[]` when the user cancelled.
pub(super) fn pick(plugin: &str, prompt: &str, opts: &Value) -> Result<Value> {
    no_picker_in_a_run()?;
    let ext: Vec<String> = opts["ext"].as_array().into_iter().flatten().filter_map(Value::as_str).map(|e| e.trim_start_matches('.').to_ascii_lowercase()).collect();
    if ext.len() > 20 || ext.iter().any(|e| e.is_empty() || e.len() > 10 || !e.bytes().all(|b| b.is_ascii_alphanumeric())) {
        bail!("ext is a list of up to 20 file extensions, letters and digits only (\"md\", \"pdf\")");
    }
    let name = plugins::enabled().into_iter().find(|p| p.id == plugin).map(|p| p.name).unwrap_or_else(|| plugin.to_string());
    let prompt: String = format!("{name} — {prompt}").chars().take(200).collect();
    let picked = crate::app::choose_files(&prompt, opts["multiple"].as_bool().unwrap_or(false), &ext);
    Ok(json!(picked.iter().map(|p| make(plugin, p)).collect::<Result<Vec<_>>>()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install(root: &Path, id: &str, uses: Value) {
        let mk = root.join("market").join(id);
        let d = mk.join("plugins").join(id);
        std::fs::create_dir_all(&d).unwrap();
        let main = r#"
            export function apply(ctx) {
              ctx.command('read', (a, { files }) => files.map((f) => ctx.files.read(f, { max: a.max })))
              ctx.command('readOne', (a) => ctx.files.read(a.file))
              ctx.command('text', async (a) => await ctx.files.text(a.file, a.opts))
              ctx.command('copy', async (a, { files }) => await ctx.fs.copyIn(files[0], ctx.fs.folders()[0], a.to, a.opts))
              ctx.command('copyId', async (a) => await ctx.fs.copyIn(a.file, ctx.fs.folders()[0], a.to))
              ctx.command('pick', async (a) => await ctx.system.pickFiles('add sources', a))
              ctx.command('seen', (a, { files }) => files)
              ctx.command('folder', async () => await ctx.system.pickFolder('a vault'))
              ctx.command('heard', () => ctx.store.read('heard.txt'))
              // From an event: no one at the screen asked — no picker.
              ctx.on('call_ended', () => ctx.system.pickFiles('x').then(() => ctx.store.write('heard.txt', 'picked'), (e) => ctx.store.write('heard.txt', e.message))).catch(() => {})
              // From its own start-up and from Echo's ask for a widget's data: no one at the screen asked either.
              let atStart = 'waiting', inWidget = 'waiting'
              ctx.system.pickFiles('x').then(() => { atStart = 'picked' }, (e) => { atStart = e.message })
              ctx.widgets.provide('w', () => { ctx.system.pickFiles('x').then(() => { inWidget = 'picked' }, (e) => { inWidget = e.message }); return { value: 1 } })
              ctx.command('unasked', () => ({ atStart, inWidget }))
              ctx.command('talk', async (a, { files }) => {
                const got = []
                const r = await ctx.claude.run({ prompt: a.prompt, tools: 'none', profile: 'work', images: files, resume: a.resume, onText: (t) => got.push(t), dir: 'runs/talk' })
                return { answer: r.answer, got }
              })
              ctx.command('forget', (a) => ctx.claude.forget(a.resume))
            }
        "#;
        std::fs::write(d.join("echo-plugin.json"), json!({"id": id, "name": id, "version": "0.1.0", "main": "main.js", "uses": uses}).to_string()).unwrap();
        std::fs::write(d.join("main.js"), main).unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": format!("m-{id}"), "plugins": [{"id": id, "source": format!("./plugins/{id}")}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some(&format!("m-{id}"))).unwrap();
        plugins::refresh(&name).unwrap();
        let agreed = plugins::review(id, Some(&name)).unwrap().uses;
        plugins::install(id, Some(&name), &agreed).unwrap();
    }

    fn run(id: &str, cmd: &str, a: Value, files: &[Value]) -> std::result::Result<Value, String> {
        for _ in 0..100 {
            match call_with(id, cmd, &a, files, CALL_TIMEOUT) {
                Err(e) if e.to_string().contains("no command") || e.to_string().contains("isn't loaded") => std::thread::sleep(Duration::from_millis(50)),
                r => return r.map_err(|e| format!("{e:#}")),
            }
        }
        Err("never loaded".into())
    }

    #[test]
    #[cfg(unix)]
    fn a_plugins_run_gets_the_pictures_it_holds_streams_its_answer_and_continues_its_conversation() {
        let (_env, root) = paths::testenv::folders("jshost-claude-images");
        let _fake = crate::ai::fake::install(&root);
        let p = crate::ai::fake::profile("work", None, &crate::ai::fake::signed_in("pat@example.com"));
        install(&root, "talker", json!({"claude": {}}));
        install(&root, "other", json!({}));
        sync();
        let session = "5fa9ec63-6e9d-48b0-ba0f-8ddd18c3b0b9";
        let delta = |t: &str| json!({"type": "stream_event", "parent_tool_use_id": null, "session_id": session, "event": {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": t}}});
        let reply = [delta("A red "), delta("square."), json!({"type": "result", "result": "A red square.", "is_error": false, "session_id": session})];
        // Its lines after its message on stdin (no reply of its own to it), or at once without pictures.
        std::fs::write(crate::ai::fake::file(&p, "p.reply"), "").unwrap();
        std::fs::write(crate::ai::fake::file(&p, "p.result"), format!("{}\n", reply.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n"))).unwrap();
        let pic = |name: &str, bytes: &[u8]| {
            std::fs::write(root.join(name), bytes).unwrap();
            make("talker", &root.join(name)).unwrap()
        };
        let png = pic("red.png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR");
        let gif = pic("dot.gif", b"GIF89a\x01\0\x01\0");
        // Its pictures with the prompt, its answer piece by piece and whole, its conversation begun.
        let out = run("talker", "talk", json!({"prompt": "What is it?", "resume": "lesson-1"}), &[png.clone(), gif.clone()]).unwrap();
        assert_eq!(out["answer"], "A red square.");
        let got: Vec<&str> = out["got"].as_array().unwrap().iter().map(|t| t.as_str().unwrap()).collect();
        assert!(!got.is_empty() && got.concat() == "A red square.", "in order, gathered as it came: {got:?}");
        let turn: Value = serde_json::from_str(std::fs::read_to_string(crate::ai::fake::file(&p, "p.turns")).unwrap().lines().last().unwrap()).unwrap();
        let kinds: Vec<&str> = turn["message"]["content"].as_array().unwrap().iter().map(|c| c["source"]["media_type"].as_str().unwrap_or("text")).collect();
        assert_eq!(kinds, ["image/png", "image/gif", "text"]);
        let args = || std::fs::read_to_string(crate::ai::fake::file(&p, "p.args")).unwrap().split('\0').map(str::to_owned).collect::<Vec<String>>();
        assert!(!args().contains(&"--resume".to_string()) && args().contains(&"--include-partial-messages".to_string()));
        // Its next turn continues that session; with no pictures, the prompt after `-p` as before.
        run("talker", "talk", json!({"prompt": "And now?", "resume": "lesson-1"}), &[]).unwrap();
        let a = args();
        assert!(a.windows(2).any(|w| w[0] == "--resume" && w[1] == session) && a[1] != "--input-format", "{a:?}");
        // Forgotten: true once, then false; the next turn begins anew.
        assert_eq!(run("talker", "forget", json!({"resume": "lesson-1"}), &[]).unwrap(), json!(true));
        assert_eq!(run("talker", "forget", json!({"resume": "lesson-1"}), &[]).unwrap(), json!(false));
        run("talker", "talk", json!({"prompt": "Again", "resume": "lesson-1"}), &[]).unwrap();
        assert!(!args().contains(&"--resume".to_string()));
        assert!(run("talker", "talk", json!({"prompt": "x", "resume": "a b"}), &[]).unwrap_err().contains("a conversation (resume) is 1 to 64"));
        // Refused before Claude Code starts: over 10 pictures, one over 3.75 MB, one that isn't a picture, another's handle.
        let runs = || std::fs::read_to_string(crate::ai::fake::file(&p, "p.turns")).unwrap().lines().count();
        let before = runs();
        let eleven = vec![png.clone(); 11];
        assert!(run("talker", "talk", json!({"prompt": "x"}), &eleven).unwrap_err().contains("10 pictures at most, not 11"));
        let mut big = b"\x89PNG\r\n\x1a\n".to_vec();
        big.resize(crate::claudejob::IMAGE_BYTES as usize + 1, 0);
        let big = pic("big.png", &big);
        assert!(run("talker", "talk", json!({"prompt": "x"}), &[big]).unwrap_err().contains("“big.png” is over 3.75 MB"));
        let text = pic("notes.txt", b"plain words");
        assert!(run("talker", "talk", json!({"prompt": "x"}), &[text]).unwrap_err().contains("“notes.txt” isn't a PNG, JPEG, GIF or WebP"));
        let theirs = make("other", &root.join("red.png")).unwrap();
        assert!(service_async("talker", "claude.run", &json!({"prompt": "x", "tools": "none", "images": [theirs]})).unwrap_err().to_string().contains("isn't a file handed to talker"));
        // A stream number only Echo gives, to that plugin, once.
        let refused = |n: u64| service_async("talker", "claude.run", &json!({"prompt": "x", "tools": "none", "stream": n})).unwrap_err().to_string();
        assert!(refused(424242).contains("a run's stream is Echo's to give"));
        assert!(refused(service_async::stream_for("other")).contains("a run's stream is Echo's to give"));
        assert!(service_async("talker", "claude.run", &json!({"prompt": "x", "tools": "none", "stream": "7"})).is_err());
        let n = service_async::stream_for("talker");
        assert!(service_async("talker", "claude.run", &json!({"prompt": "x", "tools": "none", "profile": "work", "stream": n})).is_ok());
        assert!(refused(n).contains("a run's stream is Echo's to give"), "once");
        assert_eq!(runs(), before, "no run started");
        // Forgetting needs `claude`.
        assert!(service_async("other", "claude.forget", &json!("lesson-1")).unwrap_err().to_string().contains("“claude”"));
    }

    #[test]
    fn a_handle_is_its_plugins_alone_and_never_names_the_path() {
        let (_env, root) = paths::testenv::folders("jshost-handles");
        install(&root, "taker", json!({}));
        install(&root, "other", json!({}));
        sync();
        let f = root.join("Brief.md");
        std::fs::write(&f, "# Brief\nAcme's kickoff").unwrap();
        let h = make("taker", &f).unwrap();
        assert_eq!((h["name"].as_str(), h["size"].as_u64()), (Some("Brief.md"), Some(22)));
        assert!(!h.to_string().contains(root.to_str().unwrap()), "no path in a handle: {h}");
        assert_eq!(h["file"].as_str().unwrap().len(), 32, "an opaque id");
        // Its own: read, also capped; its command sees the handles it was passed.
        assert_eq!(run("taker", "read", json!({}), std::slice::from_ref(&h)).unwrap(), json!(["# Brief\nAcme's kickoff"]));
        assert_eq!(run("taker", "read", json!({"max": 7}), std::slice::from_ref(&h)).unwrap(), json!(["# Brief"]));
        assert_eq!(run("taker", "seen", json!({}), std::slice::from_ref(&h)).unwrap(), json!([h]));
        assert_eq!(run("taker", "seen", json!({}), &[]).unwrap(), json!([]), "no files passed: an empty list");
        // Another plugin's handle: refused passed to it, and refused named by id in its own command.
        assert!(run("other", "seen", json!({}), std::slice::from_ref(&h)).unwrap_err().contains("isn't a file handed to other"));
        assert!(run("other", "readOne", json!({"file": h["file"]}), &[]).unwrap_err().contains("isn't a file handed to other"));
        assert!(run("taker", "readOne", json!({"file": "0".repeat(32)}), &[]).unwrap_err().contains("isn't a file handed to taker"));
        assert!(make("taker", &root.join("none.md")).is_err() && make("taker", &root).unwrap_err().to_string().contains("isn't a file"));
        // Past its hour: gone.
        handles().get_mut(h["file"].as_str().unwrap()).unwrap().made -= KEEP;
        assert!(run("taker", "readOne", json!({"file": h["file"]}), &[]).unwrap_err().contains("over an hour ago"));
        // Switched off, updated or removed: it keeps none of the files it was handed.
        let h2 = make("taker", &f).unwrap();
        let o2 = make("other", &f).unwrap();
        plugins::set_enabled("taker", false).unwrap();
        assert!(own_list("taker", std::slice::from_ref(&h2)).is_err() && own_list("other", std::slice::from_ref(&o2)).is_ok(), "only its own go");
        plugins::set_enabled("taker", true).unwrap();
        let h3 = make("taker", &f).unwrap();
        plugins::remove("taker", true).unwrap();
        assert!(own_list("taker", &[h3]).is_err());
        // A file handed over, replaced by a FIFO since: refused, never waited on (opened without blocking on macOS).
        #[cfg(target_os = "macos")]
        {
            let p = root.join("pipe.md");
            std::fs::write(&p, "x").unwrap();
            let ph = make("other", &p).unwrap();
            std::fs::remove_file(&p).unwrap();
            assert!(std::process::Command::new("mkfifo").arg(&p).status().unwrap().success());
            assert!(run("other", "readOne", json!({"file": ph["file"]}), &[]).unwrap_err().contains("isn't a file"));
        }
        plugins::remove("other", true).unwrap();
        sync();
    }

    #[test]
    fn copy_in_puts_the_bytes_in_a_granted_folder_never_over_a_file_or_through_a_link() {
        let (_env, root) = paths::testenv::folders("jshost-copy-in");
        let vault = root.join("vault");
        std::fs::create_dir_all(vault.join("sources")).unwrap();
        std::fs::write(vault.join("sources/Taken.pdf"), "old").unwrap();
        let outside = root.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        install(&root, "ingest", json!({"folders": {"what": "vaults", "write": true}}));
        install(&root, "looker", json!({"folders": {"what": "vaults"}}));
        for id in ["ingest", "looker"] {
            crate::grants::grant(id, &vault).unwrap();
        }
        sync();
        let bytes: Vec<u8> = (0..=255u8).cycle().take(70_000).collect();
        let src = root.join("Deck.pdf");
        std::fs::write(&src, &bytes).unwrap();
        let h = make("ingest", &src).unwrap();
        let one = std::slice::from_ref(&h);
        // Byte for byte, a new folder too.
        assert_eq!(run("ingest", "copy", json!({"to": "sources/2026/Deck.pdf"}), one).unwrap(), json!({"path": "sources/2026/Deck.pdf", "size": 70_000}));
        assert_eq!(std::fs::read(vault.join("sources/2026/Deck.pdf")).unwrap(), bytes);
        // Never over a file — unless asked (createNew: false).
        assert!(run("ingest", "copy", json!({"to": "sources/Taken.pdf"}), one).unwrap_err().contains("there already"));
        assert_eq!(std::fs::read_to_string(vault.join("sources/Taken.pdf")).unwrap(), "old");
        run("ingest", "copy", json!({"to": "sources/Taken.pdf", "opts": {"createNew": false}}), one).unwrap();
        assert_eq!(std::fs::read(vault.join("sources/Taken.pdf")).unwrap(), bytes);
        // Never out of the folder, into a hidden one, through a link, nor by a plugin that only reads.
        for (to, why) in [("../x.pdf", "isn't a path inside"), (".obsidian/x.pdf", "hidden"), ("sources", "a folder")] {
            assert!(run("ingest", "copy", json!({"to": to}), one).unwrap_err().contains(why), "{to}");
        }
        // Nor into a skill of the folder's own Claude Code, even as Markdown: its bytes would skip the skill's check.
        std::fs::write(&src, "```!\nid\n```").unwrap();
        let md = make("ingest", &src).unwrap();
        let e = run("ingest", "copy", json!({"to": ".claude/skills/acme-notes/SKILL.md"}), std::slice::from_ref(&md)).unwrap_err();
        assert!(e.contains("nothing is copied into a skill"), "{e}");
        assert!(!vault.join(".claude/skills/acme-notes").exists());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, vault.join("out")).unwrap();
            std::os::unix::fs::symlink(outside.join("planted.pdf"), vault.join("sources/planted.pdf")).unwrap();
            assert!(run("ingest", "copy", json!({"to": "out/x.pdf"}), one).unwrap_err().contains("leads outside"));
            assert!(run("ingest", "copy", json!({"to": "sources/planted.pdf", "opts": {"createNew": false}}), one).unwrap_err().contains("is a link"));
            assert!(!outside.join("x.pdf").exists() && !outside.join("planted.pdf").exists());
        }
        let lh = make("looker", &src).unwrap();
        assert!(run("ingest", "copyId", json!({"file": lh["file"], "to": "sources/L.pdf"}), &[]).unwrap_err().contains("isn't a file handed to ingest"), "another plugin's handle");
        assert!(run("looker", "copy", json!({"to": "sources/L.pdf"}), &[lh]).unwrap_err().contains("folders.write"));
        // Over 50 MB: refused, nothing left behind (no half-written file, no hidden part).
        let big = root.join("Big.bin");
        std::fs::File::create(&big).unwrap().set_len(MAX_COPY + 1).unwrap();
        let bh = make("ingest", &big).unwrap();
        assert!(run("ingest", "copy", json!({"to": "sources/Big.bin"}), &[bh]).unwrap_err().contains("up to 50 MB"));
        // A handed file replaced by a FIFO since: refused, never waited on (opened without blocking on macOS).
        #[cfg(target_os = "macos")]
        {
            let p = root.join("pipe.pdf");
            std::fs::write(&p, "x").unwrap();
            let ph = make("ingest", &p).unwrap();
            std::fs::remove_file(&p).unwrap();
            assert!(std::process::Command::new("mkfifo").arg(&p).status().unwrap().success());
            assert!(run("ingest", "copy", json!({"to": "sources/pipe.pdf"}), &[ph]).unwrap_err().contains("isn't a file"));
        }
        let left: Vec<_> = std::fs::read_dir(vault.join("sources")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with('.') || n == "Big.bin").collect();
        assert!(left.is_empty(), "{left:?}");
        for id in ["ingest", "looker"] {
            plugins::remove(id, true).unwrap();
        }
        sync();
    }

    // A release build has no ECHOCTL_PICK_FILES: there `pickFiles` would open the real file picker (testing R11).
    #[cfg(debug_assertions)]
    #[test]
    fn pick_files_takes_the_stand_in_in_a_debug_build_and_gives_handles() {
        let (_env, root) = paths::testenv::folders("jshost-pick-files");
        install(&root, "picker", json!({"folders": {"what": "vaults"}, "events": {"on": ["call_ended"]}}));
        sync();
        for n in ["a.md", "b.PDF", "c.txt"] {
            std::fs::write(root.join(n), n).unwrap();
        }
        let list = std::env::join_paths(["a.md", "b.PDF", "c.txt", "none.md"].map(|n| root.join(n))).unwrap();
        std::env::set_var("ECHOCTL_PICK_FILES", &list);
        let names = |v: Value| v.as_array().unwrap().iter().map(|h| h["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        let many = run("picker", "pick", json!({"multiple": true}), &[]);
        let typed = run("picker", "pick", json!({"multiple": true, "ext": ["md", ".pdf"]}), &[]);
        let one = run("picker", "pick", json!({}), &[]);
        let bad = run("picker", "pick", json!({"ext": ["../x"]}), &[]);
        std::env::remove_var("ECHOCTL_PICK_FILES");
        let many = many.unwrap();
        assert_eq!(names(many.clone()), ["a.md", "b.PDF", "c.txt"], "files that aren't there left out");
        assert_eq!(names(typed.unwrap()), ["a.md", "b.PDF"], "only the extensions asked for, in any case");
        assert_eq!(names(one.unwrap()), ["a.md"], "one unless multiple");
        assert!(bad.unwrap_err().contains("file extensions"));
        assert_eq!(run("picker", "read", json!({}), &own_list("picker", &many.as_array().unwrap()[..1]).unwrap()).unwrap(), json!(["a.md"]));
        // Inside a run Echo started (a nightly routine) and from an event handler: no picker, the stand-in set or not.
        std::env::set_var("ECHOCTL_PICK_FILES", root.join("a.md"));
        std::env::set_var("ECHOCTL_PICK_FOLDER", &root);
        std::env::set_var(crate::notes::HEADLESS, "1");
        let (files, folder) = (run("picker", "pick", json!({}), &[]), run("picker", "folder", json!({}), &[]));
        std::env::remove_var(crate::notes::HEADLESS);
        crate::jshost::event("call_ended", &json!({}));
        let heard = (0..100).find_map(|_| run("picker", "heard", json!({}), &[]).ok().filter(|v| !v.is_null()).or_else(|| { std::thread::sleep(Duration::from_millis(50)); None }));
        std::env::remove_var("ECHOCTL_PICK_FILES");
        std::env::remove_var("ECHOCTL_PICK_FOLDER");
        assert!(files.unwrap_err().contains("not inside a run Echo started"));
        assert!(folder.unwrap_err().contains("not inside a run Echo started"));
        assert_eq!(heard, Some(json!("a picker opens only for what you asked, not from a timer or an event")));
        // Its own start-up (`apply`, on every load) and Echo's ask for a widget's data: refused too.
        run("picker", "__widget:w", json!({}), &[]).unwrap();
        let refused = json!("a picker opens only for what you asked, not from a timer or an event");
        let unasked = (0..100).find_map(|_| run("picker", "unasked", json!({}), &[]).ok().filter(|v| v["atStart"] != "waiting" && v["inWidget"] != "waiting").or_else(|| { std::thread::sleep(Duration::from_millis(50)); None }));
        assert_eq!(unasked, Some(json!({"atStart": refused, "inWidget": refused})));
        plugins::remove("picker", true).unwrap();
        sync();
    }

    #[cfg(feature = "intake")]
    #[test]
    fn text_of_a_handed_file_comes_from_echos_readers_capped_and_only_for_its_plugin() {
        use std::io::Write as _;
        let (_env, root) = paths::testenv::folders("jshost-files-text");
        install(&root, "reader", json!({}));
        install(&root, "other", json!({}));
        sync();
        let handle = |plugin: &str, name: &str, bytes: &[u8]| {
            let f = root.join(name);
            std::fs::write(&f, bytes).unwrap();
            make(plugin, &f).unwrap()
        };
        let text = |h: &Value, opts: Value| run("reader", "text", json!({"file": h["file"], "opts": opts}), &[]);
        // Plain text, Markdown and HTML pass through (HTML as the Brain reads it); an unknown name with text is text.
        let md = handle("reader", "Brief.md", "# Acme kickoff\r\nScope: Aurora".as_bytes());
        assert_eq!(text(&md, json!({})).unwrap(), json!({"text": "# Acme kickoff\nScope: Aurora", "pages": null, "truncated": false}));
        let html = handle("reader", "Page.html", format!("<html><body><h1>Globex plan</h1><p>Phase two {}</p><script>evil()</script></body></html>", "of the rollout. ".repeat(20)).as_bytes());
        assert!(text(&handle("reader", "Short.html", b"<p>Too short</p>"), json!({})).unwrap_err().contains("nothing to read in it"));
        let t = text(&html, json!({})).unwrap();
        assert!(t["text"].as_str().unwrap().contains("Globex plan") && t["text"].as_str().unwrap().contains("Phase two") && !t["text"].as_str().unwrap().contains("evil"), "{t}");
        let bare = handle("reader", "fetched-0001", b"Fetched words");
        assert_eq!(text(&bare, json!({})).unwrap()["text"], "Fetched words");
        assert!(text(&handle("reader", "blob.bin", &[1, 0, 2, 0]), json!({})).unwrap_err().contains("not a file with text"));
        // Word: its paragraphs; a made-up file built here.
        let mut docx = std::io::Cursor::new(vec![]);
        {
            let mut z = zip::ZipWriter::new(&mut docx);
            z.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(br#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Umbrella budget</w:t></w:r></w:p><w:p><w:r><w:t>Q3 is approved</w:t></w:r></w:p></w:body></w:document>"#).unwrap();
            z.finish().unwrap();
        }
        let d = text(&handle("reader", "Budget.docx", docx.get_ref()), json!({})).unwrap();
        assert!(d["text"].as_str().unwrap().contains("Umbrella budget") && d["text"].as_str().unwrap().contains("Q3 is approved") && d["pages"].is_null(), "{d}");
        // A file that is not what its name says: an error, never a panic or a hang.
        assert!(text(&handle("reader", "Broken.docx", b"not a zip"), json!({})).is_err());
        assert!(text(&handle("reader", "Broken.pdf", b"not a pdf"), json!({})).is_err());
        // The cap on the text: cut at a character, and said.
        let long = handle("reader", "Long.txt", "é".repeat(100).as_bytes());
        let c = text(&long, json!({"max": 51})).unwrap();
        assert_eq!((c["text"].as_str().unwrap().chars().count(), c["truncated"].clone()), (25, json!(true)));
        // pages: a whole number or a range; anything else refused before the file is read.
        for bad in [json!("0"), json!("5-2"), json!("x"), json!({"a": 1}), json!("1-2-3"), json!("-4"), json!(-1), json!(2.5), json!("2.5")] {
            assert!(text(&md, json!({"pages": bad})).unwrap_err().contains("pages is a"), "{bad}");
        }
        assert_eq!(page_range(&json!("2-5")).unwrap(), Some((2, 5)));
        assert_eq!(page_range(&json!(3)).unwrap(), Some((3, 3)));
        assert_eq!(page_range(&json!("7-")).unwrap(), Some((7, usize::MAX)));
        assert_eq!(page_range(&Value::Null).unwrap(), None);
        // A file without pages is no place for `pages`.
        assert!(text(&md, json!({"pages": 1})).unwrap_err().contains("has no pages"));
        assert!(text(&bare, json!({"pages": "1-2"})).unwrap_err().contains("has no pages"));
        // An old .xls isn't read; an Office file that isn't a zip is an error.
        assert!(text(&handle("reader", "Old.xls", b"x"), json!({})).unwrap_err().contains(".xls"));
        // An Office file that unpacks to over 200 MB, or holds too many parts: refused before a reader opens it.
        let zip_of = |entries: usize, each: u64| {
            let mut out = std::io::Cursor::new(vec![]);
            let mut z = zip::ZipWriter::new(&mut out);
            let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            for i in 0..entries {
                z.start_file(format!("word/p{i}.xml"), o).unwrap();
                let chunk = vec![b'a'; 1 << 20];
                let mut left = each;
                while left > 0 {
                    let n = left.min(chunk.len() as u64) as usize;
                    z.write_all(&chunk[..n]).unwrap();
                    left -= n as u64;
                }
            }
            z.finish().unwrap();
            out.into_inner()
        };
        assert!(text(&handle("reader", "Bomb.docx", &zip_of(1, 201 << 20)), json!({})).unwrap_err().contains("unpacks to over 200 MB"));
        assert!(text(&handle("reader", "Many.docx", &zip_of(TEXT_MAX_PARTS + 1, 0)), json!({})).unwrap_err().contains("parts"));
        // A reader that hangs is killed when its time is up; one that crashes takes only itself (told apart).
        std::env::set_var("ECHOCTL_TEST_TEXT_SECS", "1");
        std::env::set_var("ECHOCTL_TEST_TEXT_SLEEP", "30");
        let t = Instant::now();
        let slow = text(&md, json!({}));
        assert!(slow.unwrap_err().contains("took over 60 seconds") && t.elapsed() < Duration::from_secs(10), "the child is killed, not waited for");
        // One extraction at a time for a plugin: a second while the first runs is refused (and its place given back after).
        std::env::set_var("ECHOCTL_TEST_TEXT_SECS", "3");
        std::env::set_var("ECHOCTL_TEST_TEXT_SLEEP", "2");
        let first = {
            let h = md.clone();
            std::thread::spawn(move || text(&h, json!({})))
        };
        std::thread::sleep(Duration::from_millis(700));
        assert!(text(&md, json!({})).unwrap_err().contains("reading a file already"));
        assert_eq!(first.join().unwrap().unwrap()["text"], "# Acme kickoff\nScope: Aurora");
        std::env::remove_var("ECHOCTL_TEST_TEXT_SECS");
        std::env::remove_var("ECHOCTL_TEST_TEXT_SLEEP");
        std::env::set_var("ECHOCTL_TEST_TEXT_CRASH", "1");
        let crashed = text(&md, json!({})).unwrap_err();
        std::env::remove_var("ECHOCTL_TEST_TEXT_CRASH");
        assert!(crashed.contains("the reader failed") && !crashed.contains("60 seconds"), "{crashed}");
        assert_eq!(text(&md, json!({})).unwrap()["truncated"], false, "the next one runs");
        // Two in all: a third plugin's is refused while two run.
        {
            let _a = Reading::take("p1").unwrap();
            let _b = Reading::take("p2").unwrap();
            assert!(Reading::take("p3").unwrap_err().to_string().contains("2 files"));
        }
        assert!(Reading::take("p3").is_ok());
        // Over 50 MB: refused without reading.
        let big = root.join("Big.txt");
        std::fs::File::create(&big).unwrap().set_len(TEXT_MAX_FILE + 1).unwrap();
        let bh = make("reader", &big).unwrap();
        assert!(text(&bh, json!({})).unwrap_err().contains("up to 50 MB"));
        // Another plugin's handle, and one past its hour: not handles.
        assert!(run("other", "text", json!({"file": md["file"]}), &[]).unwrap_err().contains("isn't a file handed to other"));
        handles().get_mut(md["file"].as_str().unwrap()).unwrap().made -= KEEP;
        assert!(text(&md, json!({})).unwrap_err().contains("over an hour ago"));
        for id in ["reader", "other"] {
            plugins::remove(id, true).unwrap();
        }
        sync();
    }
}
