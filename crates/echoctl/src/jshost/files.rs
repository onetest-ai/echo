//! Files the user hands a plugin — picked in the system's dialog (`ctx.system.pickFiles`), dropped on the window while
//! one of its views is shown (`ctx.onFileDrop`), or named with `echoctl plugin call … --file` — as **handles**:
//! `{file, name, size}`, `file` an opaque id owned by that one plugin, never the path. Kept in this process's memory
//! for an hour (`KEEP`): a file handed over once isn't the plugin's to read again whenever it likes. With a handle the
//! host half reads the text (`ctx.files.read`) or copies the bytes into a folder it was given (`ctx.fs.copyIn`).

use super::*;
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

/// `ctx.fs.copyIn(file, root, rel, {createNew})`: the file's bytes, as they are, into `rel` of a folder the plugin was
/// given (`uses.folders.write`, `grants::writable`: never through a link). Never over a file there unless
/// `createNew: false` asked for it. Written whole into a hidden file of its own beside it first (made new: a link
/// planted there isn't followed), then linked into place — which never replaces a file — or, asked, renamed over it.
pub(super) fn copy_in(plugin: &str, file: &Value, root: &str, rel: &str, create_new: bool) -> Result<Value> {
    uses_flag(plugin, "folders", "write")?;
    let (src, _) = own(plugin, file)?;
    let to = crate::grants::writable(plugin, root, rel)?;
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
    let to = crate::grants::writable(plugin, root, rel)?;
    static N: AtomicU64 = AtomicU64::new(0);
    let name = to.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = to.with_file_name(format!(".{name}.{}.{}.part", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    let copied = std::io::copy(&mut (&mut from).take(MAX_COPY + 1), &mut out).and_then(|n| out.flush().map(|_| n));
    drop(out);
    let placed = match copied {
        Ok(n) if n > MAX_COPY => Err(anyhow!("the file grew past 50 MB while it was copied")),
        Ok(_) if create_new => std::fs::hard_link(&tmp, &to).map_err(|e| match e.kind() {
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
}
