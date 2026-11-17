//! A granted folder's further services — writes only over what the plugin read (`ifSha`) or never over a file
//! (`createNew`), appends, new folders, what a path is (`stat`), text search, scratch copies (`snapshot`) — and the
//! store's guarded writes and hashing. Every folder service checks the manifest (`uses.folders`, writes
//! `folders.write`) and goes through `grants::resolve` (reads) or `grants::writable` (writes). The plugin's writes to a file — its store's and its folders' — take one lock of its
//! own across processes (`.locks/<id>.files.lock`), so the app and `echoctl plugin call` never lose each other's.

use super::*;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

/// The largest file hashed (`stat`, `walk`): a bigger one has no `sha`.
const SHA_MAX: u64 = 64 << 20;
/// The most `append` adds at once.
const APPEND_MAX: usize = 1 << 20;
/// Search: the bytes read of one file, the files looked at, the results by default and at most, a line's text.
const SEARCH_FILE: u64 = 4 << 20;
const SEARCH_FILES: usize = 100_000;
const SEARCH_MAX: u64 = 1000;
const LINE_CHARS: usize = 300;
/// Snapshot: files and bytes at most (a plugin may ask for less); a bigger file is left out and counted.
const SNAP_FILES: u64 = 20_000;
const SNAP_BYTES: u64 = 512 << 20;
/// Snapshots a plugin keeps at once, and their bytes in all.
const SNAP_KEPT: usize = 3;
const SNAP_ALL: u64 = 2 << 30;
/// Paths `exclude` names at most.
const EXCLUDE_MAX: usize = 100;

fn lock_file(id: &str) -> PathBuf {
    paths::state_dir().join("plugins").join(".locks").join(format!("{id}.files.lock"))
}

/// The plugin's lock for one write of a file, across processes.
fn lock(id: &str) -> Result<crate::app::FileLock> {
    let p = lock_file(id);
    std::fs::create_dir_all(p.parent().unwrap_or(&p))?;
    plugins::wait_lock_for(&p, &format!("{id}'s files"), Duration::from_secs(5))
}

fn snapshot_lock_file(id: &str) -> PathBuf {
    lock_file(id).with_file_name(format!("{id}.snapshot.lock"))
}

/// A plugin removed with its data: its locks go too, when nobody holds them.
pub fn forget(id: &str) {
    for p in [lock_file(id), snapshot_lock_file(id)] {
        if let Ok(Some(l)) = crate::app::FileLock::try_take(&p) {
            drop(l);
            let _ = std::fs::remove_file(&p);
        }
    }
}

pub(super) fn sha_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A file opened to read, never through a link at its last name, never a FIFO waited on: a regular file or an error.
fn open_no_link(p: &Path) -> Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    o.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_OPEN_REPARSE_POINT: a link is opened as itself, then refused as no file.
        o.custom_flags(0x0020_0000);
    }
    regular(o.open(p)?, p)
}

/// The file opened, refused unless it is a regular one; reads block again.
fn regular(f: std::fs::File, p: &Path) -> Result<std::fs::File> {
    if !f.metadata()?.is_file() {
        bail!("“{}” isn't a file", p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
    }
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: the open file's own descriptor; F_GETFL / F_SETFL only read and set its status flags.
        unsafe {
            let flags = libc::fcntl(f.as_raw_fd(), libc::F_GETFL);
            if flags >= 0 {
                libc::fcntl(f.as_raw_fd(), libc::F_SETFL, flags & !libc::O_NONBLOCK);
            }
        }
    }
    Ok(f)
}

/// The sha256 of the file at `p`, or None when nothing is there.
fn sha_file(p: &Path) -> Result<Option<String>> {
    if std::fs::symlink_metadata(p).is_err() {
        return Ok(None);
    }
    let mut h = Sha256::new();
    std::io::copy(&mut open_no_link(p)?, &mut h)?;
    Ok(Some(h.finalize().iter().map(|b| format!("{b:02x}")).collect()))
}

/// `text` into `f` (its folder there), as `opts` asks — under the plugin's lock (the caller's):
/// - `ifSha`: only while the file there is still the one with that sha256 (atomically, as any write);
/// - `createNew`: never over a file — a hidden part beside it linked into place, which never replaces one;
/// - neither: atomically, over whatever is there.
pub(super) fn put(f: &Path, name: &str, text: &str, opts: &Value) -> Result<()> {
    // `ifSha: null` (said, not left out) is "nothing there": never an unconditional write.
    let if_sha = opts.get("ifSha");
    let create_new = opts["createNew"] == true || if_sha.is_some_and(Value::is_null);
    if let Some(want) = if_sha.filter(|v| !v.is_null()) {
        let Some(want) = want.as_str().filter(|w| w.len() == 64 && w.bytes().all(|b| b.is_ascii_hexdigit())) else {
            bail!("ifSha is a sha256 in hex (64 characters), or null for a file that isn't there yet")
        };
        if create_new {
            bail!("ifSha and createNew: one or the other");
        }
        // Never a big file hashed while the lock is held, on the plugin's own time.
        if std::fs::symlink_metadata(f).is_ok_and(|m| m.len() > SHA_MAX) {
            bail!("“{name}” is over 64 MB: ifSha compares files up to 64 MB");
        }
        if sha_file(f)?.as_deref() != Some(want.to_ascii_lowercase().as_str()) {
            bail!("“{name}” changed since you read it: read it again");
        }
    } else if create_new {
        static N: AtomicU64 = AtomicU64::new(0);
        let file = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let tmp = f.with_file_name(format!(".{file}.{}.{}.part", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        let placed = out.write_all(text.as_bytes()).and_then(|_| out.flush()).and_then(|_| std::fs::hard_link(&tmp, f));
        drop(out);
        let _ = std::fs::remove_file(&tmp);
        return placed.map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => anyhow!("“{name}” is there already"),
            _ => anyhow!("“{name}”: {e}"),
        });
    }
    paths::write_atomic(f, text.as_bytes())?;
    Ok(())
}

/// `ctx.fs.write(root, rel, text, {ifSha, createNew})`, with `folders.write`.
pub(super) fn write(id: &str, root: &str, rel: &str, text: &str, opts: &Value) -> Result<()> {
    uses_flag(id, "folders", "write")?;
    let f = crate::grants::writable(id, root, rel)?;
    if f.is_dir() {
        bail!("“{rel}” is a folder");
    }
    std::fs::create_dir_all(f.parent().unwrap_or(&f))?;
    let _l = lock(id)?;
    put(&f, rel, text, opts)
}

/// `ctx.store.write(rel, text, {ifSha, createNew})`: a path the plugin may write in its store, under its lock.
pub(super) fn store_write(id: &str, rel: &str, text: &str, opts: &Value) -> Result<()> {
    let p = writable_store_path(id, rel)?;
    std::fs::create_dir_all(p.parent().unwrap_or(&p))?;
    let _l = lock(id)?;
    put(&p, rel, text, opts)
}

/// `ctx.fs.append(root, rel, text)`: `text` added at the end of the file (made when it isn't there; a newline added
/// when it doesn't end with one), in one write under the plugin's lock — never through a link, never into a FIFO.
pub(super) fn append(id: &str, root: &str, rel: &str, text: &str) -> Result<()> {
    uses_flag(id, "folders", "write")?;
    if text.len() > APPEND_MAX {
        bail!("ctx.fs.append adds 1 MB at most at once");
    }
    let f = crate::grants::writable(id, root, rel)?;
    if f.is_dir() {
        bail!("“{rel}” is a folder");
    }
    std::fs::create_dir_all(f.parent().unwrap_or(&f))?;
    let mut o = std::fs::OpenOptions::new();
    o.append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        o.custom_flags(0x0020_0000);
    }
    let _l = lock(id)?;
    let mut out = regular(o.open(&f).map_err(|e| anyhow!("“{rel}”: {e}"))?, &f)?;
    let mut block = text.to_string();
    if !block.ends_with('\n') {
        block.push('\n');
    }
    out.write_all(block.as_bytes())?;
    Ok(())
}

/// `ctx.fs.mkdir(root, rel)`: the folder and those above it, inside a folder it may change.
pub(super) fn mkdir(id: &str, root: &str, rel: &str) -> Result<()> {
    uses_flag(id, "folders", "write")?;
    let d = crate::grants::writable(id, root, rel)?;
    if std::fs::symlink_metadata(&d).is_ok_and(|m| !m.is_dir()) {
        bail!("“{rel}” is a file");
    }
    std::fs::create_dir_all(&d)?;
    // A folder of it swapped for a link meanwhile: checked again as it is now.
    crate::grants::writable(id, root, rel)?;
    Ok(())
}

/// `p` as a path under `root` with `/` between names.
fn rel_of(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

fn secs(m: &std::fs::Metadata) -> u64 {
    m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

/// `ctx.fs.stat(root, rel, {sha})`: `{kind: file | dir | link | other, size, mtime, real, sha?}`, or null when nothing
/// is there. The last name is never followed: a link is a link, its `real` the place it leads to — relative to the
/// root, and only when that is inside it. Nothing outside the root is ever named.
pub(super) fn stat(id: &str, root: &str, rel: &str, opts: &Value) -> Result<Value> {
    uses(id, "folders")?;
    let root = crate::grants::resolve(id, root, "")?;
    let p = crate::grants::plain(&root, rel.trim_end_matches('/'))?;
    let full = match (p.parent(), p.file_name()) {
        // Its folder by its real names (a link inside the root followed, never one out of it).
        (Some(up), Some(name)) => crate::grants::inside(&root, &up.to_string_lossy())?.join(name),
        _ => root.clone(),
    };
    let Ok(m) = std::fs::symlink_metadata(&full) else { return Ok(Value::Null) };
    let real = dunce::canonicalize(&full).ok().filter(|r| r.starts_with(&root)).map(|r| rel_of(&root, &r));
    let t = m.file_type();
    let kind = if t.is_symlink() {
        "link"
    } else if t.is_dir() {
        "dir"
    } else if t.is_file() {
        "file"
    } else {
        "other"
    };
    let mut out = json!({"kind": kind, "size": if t.is_file() { m.len() } else { 0 }, "mtime": secs(&m), "real": real});
    if opts["sha"] == true && t.is_file() && m.len() <= SHA_MAX {
        out["sha"] = json!(sha_file(&full)?);
    }
    Ok(out)
}

/// Every file under `base` (inside the canonical `root`), never through a link; dot names left out unless `hidden`,
/// and whatever `skip` says (by its path under the root) — a folder skipped isn't entered. `f` gets each file's path
/// under the root, its full path and metadata, and says whether to go on.
fn walk_files(root: &Path, base: &Path, hidden: bool, skip: &dyn Fn(&str) -> bool, f: &mut dyn FnMut(&str, &Path, &std::fs::Metadata) -> bool) {
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        // A folder swapped for a link since it was listed: not read.
        if !dunce::canonicalize(&dir).is_ok_and(|real| real.starts_with(root)) {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let Ok(t) = e.file_type() else { continue };
            // Links aren't followed: one could lead out of the folder.
            if t.is_symlink() || (!hidden && name.starts_with('.')) {
                continue;
            }
            let p = e.path();
            let rel = rel_of(root, &p);
            if skip(&rel) {
                continue;
            }
            if t.is_dir() {
                stack.push(p);
            } else if t.is_file() {
                let Ok(m) = e.metadata() else { continue };
                if !f(&rel, &p, &m) {
                    return;
                }
            }
        }
    }
}

/// The root (canonical, granted) and the folder of it a walk starts from (`path`).
fn walk_base(id: &str, root: &str, o: &Value) -> Result<(PathBuf, PathBuf)> {
    uses(id, "folders")?;
    let root = crate::grants::resolve(id, root, "")?;
    let base = match o["path"].as_str().filter(|p| !p.is_empty()) {
        Some(rel) => crate::grants::resolve(id, &root.display().to_string(), rel)?,
        None => root.clone(),
    };
    Ok((root, base))
}

fn exts(o: &Value) -> Vec<String> {
    o["ext"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(|e| e.trim_start_matches('.').to_lowercase())).collect()).unwrap_or_default()
}

fn has_ext(p: &Path, exts: &[String]) -> bool {
    exts.is_empty() || p.extension().is_some_and(|x| exts.contains(&x.to_string_lossy().to_lowercase()))
}

/// `ctx.fs.walk(root, {path, ext, hidden, max, sha})`: [{path, size, mtime, sha?}].
pub(super) fn walk(id: &str, root: &str, o: &Value) -> Result<Value> {
    let (root, base) = walk_base(id, root, o)?;
    let max = o["max"].as_u64().filter(|m| *m > 0).unwrap_or(20_000).min(100_000) as usize;
    let (exts, sha) = (exts(o), o["sha"] == true);
    let mut out = vec![];
    walk_files(&root, &base, o["hidden"] == true, &|_| false, &mut |rel, p, m| {
        if has_ext(p, &exts) {
            let mut row = json!({"path": rel, "size": m.len(), "mtime": secs(m)});
            if sha && m.len() <= SHA_MAX {
                row["sha"] = json!(sha_file(p).ok().flatten());
            }
            out.push(row);
        }
        out.len() < max
    });
    Ok(json!(out))
}

/// `ctx.fs.search(root, query, {path, ext, hidden, max, caseSensitive})`: the lines holding `query` as it is typed
/// (no pattern), [{path, line, text}] — 200 by default, 1,000 at most; 4 MB read of a file, its line's text cut at 300
/// characters.
pub(super) fn search(id: &str, root: &str, query: &str, o: &Value) -> Result<Value> {
    let (root, base) = walk_base(id, root, o)?;
    if query.is_empty() || query.chars().count() > 1000 {
        bail!("ctx.fs.search looks for 1 to 1,000 characters");
    }
    let case = o["caseSensitive"] == true;
    let q = if case { query.to_string() } else { query.to_lowercase() };
    let max = o["max"].as_u64().filter(|m| *m > 0).unwrap_or(200).min(SEARCH_MAX) as usize;
    let exts = exts(o);
    let (mut out, mut seen) = (vec![], 0usize);
    walk_files(&root, &base, o["hidden"] == true, &|_| false, &mut |rel, p, _| {
        if !has_ext(p, &exts) {
            return true;
        }
        seen += 1;
        let mut buf = vec![];
        if open_no_link(p).and_then(|f| Ok(f.take(SEARCH_FILE).read_to_end(&mut buf)?)).is_err() {
            return seen < SEARCH_FILES;
        }
        for (i, line) in String::from_utf8_lossy(&buf).lines().enumerate() {
            let hit = if case { line.contains(&q) } else { line.to_lowercase().contains(&q) };
            if hit {
                out.push(json!({"path": rel, "line": i + 1, "text": line.chars().take(LINE_CHARS).collect::<String>()}));
                if out.len() >= max {
                    return false;
                }
            }
        }
        seen < SEARCH_FILES
    });
    Ok(json!(out))
}

/// Other programs' code, history and tokens, never copied, by name at any depth: a `.git` folder, `.obsidian/plugins`.
fn tool_folder(rel: &str) -> bool {
    let names: Vec<String> = rel.split('/').map(str::to_lowercase).collect();
    names.iter().any(|n| n == ".git") || names.windows(2).any(|w| w[0] == ".obsidian" && w[1] == "plugins")
}

/// Room for one more snapshot of plugin `id`: none being made (its lock, held while it is), fewer than 3 kept, and
/// what they hold under 2 GB → the lock and the bytes left.
fn snapshot_room(id: &str) -> Result<(crate::app::FileLock, u64)> {
    let p = snapshot_lock_file(id);
    std::fs::create_dir_all(p.parent().unwrap_or(&p))?;
    let Some(l) = crate::app::FileLock::try_take(&p)? else { bail!("a snapshot of this plugin is being made already: wait for it") };
    let kept: Vec<PathBuf> = std::fs::read_dir(plugins::data_dir(id).join("cache/snapshots")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    if kept.len() >= SNAP_KEPT {
        bail!("this plugin keeps {SNAP_KEPT} snapshots already: remove one (ctx.store.remove) first");
    }
    let mut used = 0u64;
    for k in &kept {
        let k = dunce::canonicalize(k)?;
        walk_files(&k, &k, true, &|_| false, &mut |_, _, m| {
            used += m.len();
            true
        });
    }
    if used >= SNAP_ALL {
        bail!("this plugin's snapshots hold 2 GB already: remove one (ctx.store.remove) first");
    }
    Ok((l, SNAP_ALL - used))
}

/// What became of one file a snapshot copies.
#[derive(Debug, PartialEq)]
enum Copied {
    Whole(u64),
    /// Over 50 MB — grown past it while it was copied too.
    Large,
    /// Gone since it was listed, not allowed, locked.
    Unreadable,
}

/// One file copied whole into `to` (made new), or nothing of it left there (`Large`, `Unreadable`); an error when the
/// copy can't be written.
fn copy_file(from: &Path, to: &Path) -> Result<Copied> {
    let Ok(src) = open_no_link(from) else { return Ok(Copied::Unreadable) };
    std::fs::create_dir_all(to.parent().unwrap_or(to))?;
    let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(to)?;
    let (mut src, mut buf, mut n) = (src.take(files::MAX_COPY + 1), vec![0u8; 64 << 10], 0u64);
    let left_out = loop {
        match src.read(&mut buf) {
            Ok(0) if n > files::MAX_COPY => break Copied::Large,
            Ok(0) => return Ok(Copied::Whole(n)),
            Ok(k) => {
                out.write_all(&buf[..k])?;
                n += k as u64;
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break Copied::Unreadable,
        }
    };
    drop(out);
    let _ = std::fs::remove_file(to);
    Ok(left_out)
}

/// `ctx.fs.snapshot(root, {exclude, hidden, maxFiles, maxBytes})`: the folder's files copied into the plugin's store,
/// `cache/snapshots/<id>/` (Echo sweeps `cache/`), for it to work on: never through a link, never a `.git` folder or
/// `.obsidian/plugins` at any depth, nor what `exclude` names (up to 100 paths under the root); dot names only with
/// `hidden`. 20,000 files and 512 MB at most — more is refused and nothing is left; a file over 50 MB, or one that
/// can't be read, is left out and counted. One made at a time, 3 kept and 2 GB in all a plugin.
/// → {path (in the store), files, bytes, skipped, why: {large, unreadable}}. `ctx.store.remove(path)` removes it.
pub(super) fn snapshot(id: &str, root: &str, o: &Value) -> Result<Value> {
    let (root, base) = walk_base(id, root, &json!({}))?;
    let asked = o["exclude"].as_array().map(Vec::as_slice).unwrap_or_default();
    if asked.len() > EXCLUDE_MAX {
        bail!("exclude names {EXCLUDE_MAX} paths at most");
    }
    let mut exclude = vec![];
    for e in asked {
        let e = e.as_str().ok_or_else(|| anyhow!("exclude is a list of paths in the folder"))?;
        // Compared in lower case: `.GIT` is `.git` on a disk that folds case.
        exclude.push(crate::grants::plain(&root, e.trim_end_matches('/'))?.to_string_lossy().replace('\\', "/").to_lowercase());
    }
    let skip = |rel: &str| {
        let rel = rel.to_lowercase();
        tool_folder(&rel) || exclude.iter().any(|e| rel == *e || rel.strip_prefix(e.as_str()).is_some_and(|r| r.starts_with('/')))
    };
    let (_making, room) = snapshot_room(id)?;
    let most_files = o["maxFiles"].as_u64().filter(|m| *m > 0).unwrap_or(SNAP_FILES).min(SNAP_FILES);
    let most_bytes = o["maxBytes"].as_u64().filter(|m| *m > 0).unwrap_or(SNAP_BYTES).min(SNAP_BYTES).min(room);
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).map_err(|e| anyhow!("{e}"))?;
    let at = format!("cache/snapshots/{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>());
    let dest = writable_store_path(id, &at)?;
    std::fs::create_dir_all(&dest)?;
    let (mut files, mut bytes, mut large, mut unreadable) = (0u64, 0u64, 0u64, 0u64);
    let mut failed: Option<anyhow::Error> = None;
    walk_files(&root, &base, o["hidden"] == true, &skip, &mut |rel, p, m| {
        if m.len() > files::MAX_COPY {
            large += 1;
            return true;
        }
        if files + 1 > most_files || bytes + m.len() > most_bytes {
            failed = Some(anyhow!("the folder holds more than {most_files} files or {} MB: a snapshot takes up to that", most_bytes >> 20));
            return false;
        }
        match copy_file(p, &dest.join(rel)) {
            Ok(Copied::Whole(n)) => {
                files += 1;
                bytes += n;
                true
            }
            Ok(Copied::Large) => {
                large += 1;
                true
            }
            Ok(Copied::Unreadable) => {
                unreadable += 1;
                true
            }
            Err(e) => {
                failed = Some(anyhow!("“{rel}”: {e:#}"));
                false
            }
        }
    });
    if let Some(e) = failed {
        let _ = std::fs::remove_dir_all(&dest);
        return Err(e);
    }
    Ok(json!({"path": at, "files": files, "bytes": bytes, "skipped": large + unreadable, "why": {"large": large, "unreadable": unreadable}}))
}

/// `ctx.claude.run({folder, path, mode})`: the run goes into a folder given to the plugin (`uses.folders`; `edit` also
/// `folders.write`), at `path` inside it — or, for `copy`, into a snapshot of it (`snapshot`, dot names too, so the
/// folder's `.claude` comes along), whose answer is the run's `snapshot` for the plugin to compare and apply. No
/// `folder`: the run as before (`mode` and `path` then refused).
pub(super) fn claude_folder(id: &str, arg: &Value, spec: &mut crate::claudejob::Spec) -> Result<Option<Value>> {
    use crate::claudejob::{FolderMode, InFolder};
    let (root, path) = (arg["folder"].as_str().unwrap_or(""), arg["path"].as_str().unwrap_or(""));
    if root.is_empty() {
        if !arg["mode"].is_null() || !path.is_empty() {
            bail!("a run's mode and path go with a folder it was given (folder)");
        }
        return Ok(None);
    }
    let mode = FolderMode::parse(arg["mode"].as_str().unwrap_or(""))?;
    let cwd = granted_cwd(id, root, path, mode)?;
    let (cwd, snap) = match mode {
        // The path in the snapshot: a subfolder it left out (`.git`, a cap) refuses the run, and the snapshot goes.
        FolderMode::Copy => {
            let s = snapshot(id, root, &json!({"hidden": true}))?;
            let at = store_path(id, s["path"].as_str().unwrap_or(""))?;
            let inner = dunce::canonicalize(&at).map_err(anyhow::Error::from).and_then(|a| crate::grants::inside(&a, path)).and_then(|c| Ok(dunce::canonicalize(c)?));
            match inner {
                Ok(c) if c.is_dir() => (c, Some(s)),
                _ => {
                    let _ = std::fs::remove_dir_all(&at);
                    bail!("“{path}” isn't in the copy of {root} (a copy leaves out .git and what is over its caps)");
                }
            }
        }
        _ => (dunce::canonicalize(&cwd)?, None),
    };
    spec.folder = Some(InFolder { cwd, mode });
    spec.session = false;
    Ok(snap)
}

/// Where Claude Code goes in a folder given to plugin `id` (`uses.folders`; `edit` also `folders.write`): `path` inside
/// `root`, a folder.
fn granted_cwd(id: &str, root: &str, path: &str, mode: crate::claudejob::FolderMode) -> Result<std::path::PathBuf> {
    uses(id, "folders")?;
    if mode == crate::claudejob::FolderMode::Edit {
        uses_flag(id, "folders", "write")?;
    }
    let cwd = crate::grants::resolve(id, root, path)?;
    if !cwd.is_dir() {
        bail!("“{path}” isn't a folder in {root}");
    }
    Ok(cwd)
}

/// `ctx.claude.session({folder, path, mode})` (the client half, on the user's click): the folder a plugin's Claude
/// session in Echo's terminal opens in — the plugin installed and on, with `claude` and `folders` on its sheet; `read`
/// (the default) or `edit` (`folders.write`), never `copy` (a session has no snapshot to hand back).
#[cfg(feature = "tray")]
pub fn session_folder(id: &str, arg: &Value) -> Result<crate::claudejob::InFolder> {
    use crate::claudejob::{FolderMode, InFolder};
    uses(id, "claude")?;
    let root = arg["folder"].as_str().unwrap_or("");
    if root.is_empty() {
        bail!("a Claude session opens in a folder you gave the plugin (folder)");
    }
    let mode = match FolderMode::parse(arg["mode"].as_str().unwrap_or(""))? {
        FolderMode::Copy => bail!("mode “copy”: a session reads (read) or edits (edit) its folder"),
        m => m,
    };
    let cwd = granted_cwd(id, root, arg["path"].as_str().unwrap_or(""), mode)?;
    Ok(InFolder { cwd: dunce::canonicalize(&cwd)?, mode })
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
              const r = () => ctx.fs.folders()[0]
              ctx.command('write', (a) => ctx.fs.write(r(), a.path, a.text, a.opts))
              ctx.command('append', (a) => ctx.fs.append(r(), a.path, a.text))
              ctx.command('mkdir', (a) => ctx.fs.mkdir(r(), a.path))
              ctx.command('stat', (a) => ctx.fs.stat(r(), a.path, a.opts))
              ctx.command('sha', (a) => ctx.crypto.sha256(a.text))
              ctx.command('walk', async (a) => await ctx.fs.walk(r(), a))
              ctx.command('search', async (a) => await ctx.fs.search(r(), a.q, a.opts))
              ctx.command('snapshot', async (a) => await ctx.fs.snapshot(r(), a))
              ctx.command('storeRead', (a) => ctx.store.read(a.path))
              ctx.command('storeWrite', (a) => ctx.store.write(a.path, a.text, a.opts))
              ctx.command('storeRemove', (a) => ctx.store.remove(a.path))
              ctx.command('bump', (a) => ctx.store.update(a.path, (t) => String(Number(t || 0) + 1)))
              ctx.command('same', (a) => ctx.store.update(a.path, (t) => t))
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

    fn run(id: &str, cmd: &str, a: Value) -> std::result::Result<Value, String> {
        for _ in 0..100 {
            match call_with(id, cmd, &a, &[], CALL_TIMEOUT) {
                Err(e) if e.to_string().contains("no command") || e.to_string().contains("isn't loaded") => std::thread::sleep(Duration::from_millis(50)),
                r => return r.map_err(|e| format!("{e:#}")),
            }
        }
        Err("never loaded".into())
    }

    /// A vault given to a plugin that may change it: (its path, the folder outside it).
    fn vault(root: &Path, id: &str) -> (PathBuf, PathBuf) {
        let vault = root.join("vault");
        let outside = root.join("outside");
        std::fs::create_dir_all(vault.join("Notes")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        install(root, id, json!({"folders": {"what": "vaults", "write": true}}));
        crate::grants::grant(id, &vault).unwrap();
        sync();
        (dunce::canonicalize(&vault).unwrap(), outside)
    }

    #[test]
    fn a_write_over_a_changed_file_is_refused_and_create_new_never_replaces_one() {
        let (_env, root) = paths::testenv::folders("jshost-folders-write");
        let (vault, _) = vault(&root, "keeper");
        let note = vault.join("Notes/Acme.md");
        std::fs::write(&note, "# Acme\n").unwrap();
        let read = sha_hex(b"# Acme\n");
        assert_eq!(run("keeper", "sha", json!({"text": "# Acme\n"})).unwrap(), json!(read), "ctx.crypto.sha256 hashes in Rust");
        // Over what it read: written. Over a file changed since: refused, untouched.
        run("keeper", "write", json!({"path": "Notes/Acme.md", "text": "# Acme\nv2\n", "opts": {"ifSha": read.to_uppercase()}})).unwrap();
        assert_eq!(std::fs::read_to_string(&note).unwrap(), "# Acme\nv2\n");
        let stale = run("keeper", "write", json!({"path": "Notes/Acme.md", "text": "lost", "opts": {"ifSha": read}})).unwrap_err();
        assert!(stale.contains("“Notes/Acme.md” changed since you read it"), "{stale}");
        assert_eq!(std::fs::read_to_string(&note).unwrap(), "# Acme\nv2\n");
        assert!(run("keeper", "write", json!({"path": "Notes/Gone.md", "text": "x", "opts": {"ifSha": read}})).unwrap_err().contains("changed since"), "a file gone since it was read");
        // Never over a file; a new one made, its folder too.
        assert!(run("keeper", "write", json!({"path": "Notes/Acme.md", "text": "x", "opts": {"createNew": true}})).unwrap_err().contains("is there already"));
        run("keeper", "write", json!({"path": "Inbox/2026/New.md", "text": "new", "opts": {"createNew": true}})).unwrap();
        // ifSha: null (said) is "nothing there yet", never an unconditional write; a sha not in hex, or both asked: refused.
        assert!(run("keeper", "write", json!({"path": "Notes/Acme.md", "text": "x", "opts": {"ifSha": null}})).unwrap_err().contains("is there already"));
        run("keeper", "write", json!({"path": "Notes/Null.md", "text": "first", "opts": {"ifSha": null}})).unwrap();
        assert_eq!(std::fs::read_to_string(vault.join("Notes/Null.md")).unwrap(), "first");
        for bad in [json!(12), json!(""), json!("not-a-sha"), json!("z".repeat(64)), json!(read[..63].to_string())] {
            assert!(run("keeper", "write", json!({"path": "Notes/Null.md", "text": "x", "opts": {"ifSha": bad}})).unwrap_err().contains("64 characters"), "{bad}");
        }
        assert_eq!(std::fs::read_to_string(vault.join("Notes/Null.md")).unwrap(), "first");
        assert!(run("keeper", "write", json!({"path": "Notes/Null.md", "text": "x", "opts": {"ifSha": sha_hex(b"first"), "createNew": true}})).unwrap_err().contains("one or the other"));
        // Over 64 MB: refused before it is hashed.
        std::fs::File::create(vault.join("Notes/Huge.bin")).unwrap().set_len(SHA_MAX + 1).unwrap();
        assert!(run("keeper", "write", json!({"path": "Notes/Huge.bin", "text": "x", "opts": {"ifSha": read}})).unwrap_err().contains("over 64 MB"));
        assert_eq!(std::fs::metadata(vault.join("Notes/Huge.bin")).unwrap().len(), SHA_MAX + 1);
        assert_eq!(std::fs::read_to_string(vault.join("Inbox/2026/New.md")).unwrap(), "new");
        // Racing writers asking for a new file: exactly one makes it, the others are told it is there; no part left.
        let won: Vec<bool> = (0..8)
            .map(|i| {
                let v = vault.display().to_string();
                std::thread::spawn(move || service("keeper", "fs.write", &json!([v, "Notes/Race.md", format!("w{i}"), {"createNew": true}])).is_ok())
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect();
        assert_eq!(won.iter().filter(|w| **w).count(), 1, "{won:?}");
        assert!(std::fs::read_to_string(vault.join("Notes/Race.md")).unwrap().starts_with('w'));
        let parts: Vec<_> = std::fs::read_dir(vault.join("Notes")).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with('.')).collect();
        assert!(parts.is_empty(), "no hidden part left: {parts:?}");
        plugins::remove("keeper", true).unwrap();
        sync();
    }

    #[test]
    fn append_adds_blocks_and_mkdir_makes_folders_never_through_a_link() {
        let (_env, root) = paths::testenv::folders("jshost-folders-append");
        let (vault, outside) = vault(&root, "logger");
        run("logger", "append", json!({"path": "log.md", "text": "- made"})).unwrap();
        run("logger", "append", json!({"path": "log.md", "text": "- added · Notes/A.md\n"})).unwrap();
        assert_eq!(std::fs::read_to_string(vault.join("log.md")).unwrap(), "- made\n- added · Notes/A.md\n");
        // Racing appends from many threads: every block whole, none lost.
        let v = vault.display().to_string();
        let ts: Vec<_> = (0..16).map(|i| {
            let v = v.clone();
            std::thread::spawn(move || service("logger", "fs.append", &json!([v, "race.log", format!("line {i} {}", "x".repeat(2000))])).unwrap())
        }).collect();
        ts.into_iter().for_each(|t| { t.join().unwrap(); });
        let text = std::fs::read_to_string(vault.join("race.log")).unwrap();
        assert_eq!(text.lines().count(), 16);
        assert!(text.lines().all(|l| l.starts_with("line ") && l.ends_with(&"x".repeat(2000))));
        assert!(run("logger", "append", json!({"path": "big.md", "text": "x".repeat(APPEND_MAX + 1)})).unwrap_err().contains("1 MB"));
        // Folders: made, nested; never hidden, never the root, never out, never over a file.
        run("logger", "mkdir", json!({"path": "Projects/Acme/2026"})).unwrap();
        assert!(vault.join("Projects/Acme/2026").is_dir());
        run("logger", "mkdir", json!({"path": "Projects/Acme"})).unwrap();
        for (p, why) in [(".obsidian/x", "hidden"), ("../x", "isn't a path inside"), ("", "not the folder itself"), ("log.md", "is a file")] {
            assert!(run("logger", "mkdir", json!({"path": p})).unwrap_err().contains(why), "{p}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, vault.join("out")).unwrap();
            std::os::unix::fs::symlink(outside.join("planted.log"), vault.join("planted.log")).unwrap();
            std::fs::write(outside.join("planted.log"), "theirs\n").unwrap();
            assert!(run("logger", "mkdir", json!({"path": "out/new"})).unwrap_err().contains("leads outside"));
            assert!(run("logger", "append", json!({"path": "out/x.log", "text": "x"})).unwrap_err().contains("leads outside"));
            let planted = run("logger", "append", json!({"path": "planted.log", "text": "x"})).unwrap_err();
            assert!(planted.contains("leads outside") || planted.contains("is a link"), "{planted}");
            assert_eq!(std::fs::read_to_string(outside.join("planted.log")).unwrap(), "theirs\n");
            assert!(!outside.join("new").exists() && !outside.join("x.log").exists());
        }
        #[cfg(target_os = "macos")]
        {
            assert!(std::process::Command::new("mkfifo").arg(vault.join("pipe.log")).status().unwrap().success());
            assert!(run("logger", "append", json!({"path": "pipe.log", "text": "x"})).is_err(), "a FIFO is never waited on");
        }
        plugins::remove("logger", true).unwrap();
        sync();
    }

    #[test]
    fn stat_reports_links_as_links_and_never_an_absolute_path() {
        let (_env, root) = paths::testenv::folders("jshost-folders-stat");
        let (vault, outside) = vault(&root, "looker");
        std::fs::write(vault.join("Notes/A.md"), "# A\n").unwrap();
        let a = run("looker", "stat", json!({"path": "Notes/A.md", "opts": {"sha": true}})).unwrap();
        assert_eq!((a["kind"].as_str(), a["size"].as_u64(), a["real"].as_str()), (Some("file"), Some(4), Some("Notes/A.md")));
        assert_eq!(a["sha"], json!(sha_hex(b"# A\n")));
        assert!(a["mtime"].as_u64().unwrap() > 1_700_000_000);
        assert!(run("looker", "stat", json!({"path": "Notes/A.md"})).unwrap().get("sha").is_none(), "a hash only asked");
        assert_eq!(run("looker", "stat", json!({"path": "Notes"})).unwrap()["kind"], "dir");
        assert_eq!(run("looker", "stat", json!({"path": ""})).unwrap()["real"], "", "the root itself");
        assert_eq!(run("looker", "stat", json!({"path": "Notes/None.md"})).unwrap(), Value::Null);
        assert_eq!(run("looker", "stat", json!({"path": "None/x.md"})).unwrap(), Value::Null);
        assert!(run("looker", "stat", json!({"path": "../vault/Notes/A.md"})).unwrap_err().contains("isn't a path inside"));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(vault.join("Notes"), vault.join("Linked")).unwrap();
            std::os::unix::fs::symlink(&outside, vault.join("Out")).unwrap();
            std::os::unix::fs::symlink(outside.join("secret.md"), vault.join("Notes/secret.md")).unwrap();
            std::fs::write(outside.join("secret.md"), "theirs").unwrap();
            // A link inside: a link, its real place in the vault; a file through it: its real place.
            let l = run("looker", "stat", json!({"path": "Linked"})).unwrap();
            assert_eq!((l["kind"].as_str(), l["real"].as_str()), (Some("link"), Some("Notes")));
            assert_eq!(run("looker", "stat", json!({"path": "Linked/A.md"})).unwrap()["real"], "Notes/A.md");
            // A link out: a link, no real place, nothing read through it, nothing outside named.
            for p in ["Out", "Notes/secret.md"] {
                let s = run("looker", "stat", json!({"path": p, "opts": {"sha": true}})).unwrap();
                assert_eq!((s["kind"].as_str(), &s["real"], s.get("sha")), (Some("link"), &Value::Null, None), "{p}");
                assert!(!s.to_string().contains(outside.to_str().unwrap()) && !s.to_string().contains(root.to_str().unwrap()));
            }
            assert!(run("looker", "stat", json!({"path": "Out/secret.md"})).unwrap_err().contains("leads outside"));
        }
        plugins::remove("looker", true).unwrap();
        sync();
    }

    #[test]
    fn search_finds_literal_text_capped_and_never_through_a_link() {
        let (_env, root) = paths::testenv::folders("jshost-folders-search");
        let (vault, outside) = vault(&root, "finder");
        std::fs::write(vault.join("Notes/Acme.md"), "# Acme\nKickoff with ACME on SSO.\nNo match here\n").unwrap();
        std::fs::write(vault.join("Notes/Globex.txt"), "acme (a.*b) pattern\n").unwrap();
        std::fs::create_dir_all(vault.join(".obsidian")).unwrap();
        std::fs::write(vault.join(".obsidian/app.json"), "acme").unwrap();
        std::fs::write(vault.join("Notes/Many.md"), "acme\n".repeat(50)).unwrap();
        std::fs::write(outside.join("leak.md"), "acme outside").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, vault.join("Out")).unwrap();
        let hits = |q: &str, o: Value| run("finder", "search", json!({"q": q, "opts": o})).unwrap().as_array().unwrap().clone();
        let all = hits("acme", json!({}));
        assert_eq!(all.len(), 2 + 1 + 50, "every line, any case; not hidden, not through a link: {all:?}");
        assert!(all.iter().all(|h| !h["path"].as_str().unwrap().starts_with(".") && !h["path"].as_str().unwrap().starts_with("Out")));
        let sso = hits("on sso", json!({}));
        assert_eq!(sso, vec![json!({"path": "Notes/Acme.md", "line": 2, "text": "Kickoff with ACME on SSO."})]);
        assert_eq!(hits("ACME", json!({"caseSensitive": true})).len(), 1, "the kickoff line alone");
        assert_eq!(hits("(a.*b)", json!({})).len(), 1, "literal, no pattern");
        assert_eq!(hits("acme", json!({"ext": ["txt"]})).len(), 1);
        assert_eq!(hits("acme", json!({"max": 5})).len(), 5);
        assert_eq!(hits("acme", json!({"max": 5000})).len(), 53, "capped at 1,000, all here");
        assert_eq!(hits("acme", json!({"hidden": true})).len(), 54);
        assert!(run("finder", "search", json!({"q": ""})).unwrap_err().contains("1 to 1,000"));
        assert!(run("finder", "search", json!({"q": "x", "opts": {"path": "../outside"}})).is_err());
        // A long line: its text cut.
        std::fs::write(vault.join("Notes/Long.md"), format!("zeta {}", "y".repeat(1000))).unwrap();
        assert_eq!(hits("zeta", json!({}))[0]["text"].as_str().unwrap().chars().count(), LINE_CHARS);
        plugins::remove("finder", true).unwrap();
        sync();
    }

    #[test]
    fn a_snapshot_copies_the_folder_into_the_cache_never_a_link_or_a_tool_folder() {
        let (_env, root) = paths::testenv::folders("jshost-folders-snapshot");
        let (vault, outside) = vault(&root, "copier");
        for (p, t) in [("Notes/A.md", "# A"), ("Notes/sub/B.md", "# B"), (".git/config", "token"), (".obsidian/plugins/x/data.json", "token"), (".obsidian/app.json", "{}"), ("Archive/old.md", "old"), (".env", "secret"), ("Notes/sub/.git/config", "token"), ("Archive/.obsidian/plugins/y/data.json", "token")] {
            std::fs::create_dir_all(vault.join(p).parent().unwrap()).unwrap();
            std::fs::write(vault.join(p), t).unwrap();
        }
        std::fs::write(outside.join("leak.md"), "theirs").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, vault.join("Out")).unwrap();
            std::os::unix::fs::symlink(outside.join("leak.md"), vault.join("Notes/leak.md")).unwrap();
        }
        let s = run("copier", "snapshot", json!({})).unwrap();
        let at = s["path"].as_str().unwrap().to_string();
        assert!(at.starts_with("cache/snapshots/") && !s.to_string().contains(root.to_str().unwrap()), "{s}");
        assert_eq!((s["files"].as_u64(), s["bytes"].as_u64(), s["skipped"].as_u64()), (Some(3), Some(9), Some(0)));
        let copy = dunce::canonicalize(plugins::data_dir("copier").join(&at)).unwrap();
        let mut got: Vec<String> = vec![];
        walk_files(&copy, &copy, true, &|_| false, &mut |rel, _, _| { got.push(rel.to_string()); true });
        got.sort();
        assert_eq!(got, ["Archive/old.md", "Notes/A.md", "Notes/sub/B.md"], "no dot names, no links");
        assert_eq!(run("copier", "storeRead", json!({"path": format!("{at}/Notes/A.md")})).unwrap(), "# A", "read with ctx.store");
        // With hidden: dot names too — but never .git or .obsidian/plugins; what `exclude` names left out.
        let h = run("copier", "snapshot", json!({"hidden": true, "exclude": ["Archive", "notes/sub/"]})).unwrap();
        let hc = dunce::canonicalize(plugins::data_dir("copier").join(h["path"].as_str().unwrap())).unwrap();
        let mut got: Vec<String> = vec![];
        walk_files(&hc, &hc, true, &|_| false, &mut |rel, _, _| { got.push(rel.to_string()); true });
        got.sort();
        assert_eq!(got, [".env", ".obsidian/app.json", "Notes/A.md"]);
        let nested = run("copier", "snapshot", json!({"hidden": true})).unwrap();
        let nc = dunce::canonicalize(plugins::data_dir("copier").join(nested["path"].as_str().unwrap())).unwrap();
        let mut got: Vec<String> = vec![];
        walk_files(&nc, &nc, true, &|_| false, &mut |rel, _, _| { got.push(rel.to_string()); true });
        assert!(!got.iter().any(|g| g.contains(".git") || g.contains("plugins")) && got.contains(&"Notes/sub/B.md".to_string()), "tool folders by name at any depth: {got:?}");
        run("copier", "storeRemove", json!({"path": nested["path"]})).unwrap();
        assert!(run("copier", "snapshot", json!({"exclude": ["../x"]})).unwrap_err().contains("isn't a path inside"));
        let many: Vec<String> = (0..=EXCLUDE_MAX).map(|i| format!("x{i}")).collect();
        assert!(run("copier", "snapshot", json!({"exclude": many})).unwrap_err().contains("100 paths at most"), "never dropped silently");
        // One made at a time.
        let making = crate::app::FileLock::try_take(&snapshot_lock_file("copier")).unwrap().unwrap();
        assert!(run("copier", "snapshot", json!({})).unwrap_err().contains("being made already"));
        drop(making);
        // Over its caps: refused, nothing left.
        let before = std::fs::read_dir(plugins::data_dir("copier").join("cache/snapshots")).unwrap().count();
        assert!(run("copier", "snapshot", json!({"maxFiles": 2})).unwrap_err().contains("more than 2 files"));
        assert!(run("copier", "snapshot", json!({"maxBytes": 5})).unwrap_err().contains("more than"));
        assert_eq!(std::fs::read_dir(plugins::data_dir("copier").join("cache/snapshots")).unwrap().count(), before);
        // A file over 50 MB: left out and counted.
        std::fs::File::create(vault.join("Notes/video.mp4")).unwrap().set_len(files::MAX_COPY + 1).unwrap();
        // A file it can't read: left out and counted, the rest copied.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(vault.join("Notes/locked.md"), "x").unwrap();
            std::fs::set_permissions(vault.join("Notes/locked.md"), std::fs::Permissions::from_mode(0o000)).unwrap();
        }
        let big = run("copier", "snapshot", json!({})).unwrap();
        #[cfg(unix)]
        assert_eq!((big["files"].as_u64(), big["skipped"].as_u64(), &big["why"]), (Some(3), Some(2), &json!({"large": 1, "unreadable": 1})));
        // A file grown past 50 MB while it was copied (here: given to the copy as it is now): left out, nothing kept of it.
        let grown = root.join("grown.bin");
        std::fs::File::create(&grown).unwrap().set_len(files::MAX_COPY + 1).unwrap();
        let to = root.join("copy/grown.bin");
        assert_eq!(copy_file(&grown, &to).unwrap(), Copied::Large);
        assert!(!to.exists());
        std::fs::write(&grown, "small").unwrap();
        assert_eq!(copy_file(&grown, &to).unwrap(), Copied::Whole(5));
        // Three kept: a fourth refused until one is removed.
        assert!(run("copier", "snapshot", json!({})).unwrap_err().contains("keeps 3 snapshots already"));
        // Removed with ctx.store.remove.
        run("copier", "storeRemove", json!({"path": at})).unwrap();
        assert!(!copy.exists());
        // Echo's sweep: old files of cache/ and the folders they leave empty go.
        let old = plugins::data_dir("copier").join(big["path"].as_str().unwrap());
        for f in ["Notes/A.md", "Notes/sub/B.md", "Archive/old.md"] {
            std::fs::File::options().write(true).open(old.join(f)).unwrap().set_modified(std::time::SystemTime::now() - Duration::from_secs(8 * 86_400)).unwrap();
        }
        sweep_store("copier");
        assert!(!old.exists(), "a snapshot swept whole");
        assert!(plugins::data_dir("copier").join("cache").is_dir(), "cache/ itself stays");
        plugins::remove("copier", true).unwrap();
        sync();
    }

    #[test]
    fn folder_services_ask_for_what_the_sheet_said() {
        let (_env, root) = paths::testenv::folders("jshost-folders-uses");
        let vault = root.join("vault");
        std::fs::create_dir_all(&vault).unwrap();
        std::fs::write(vault.join("A.md"), "acme").unwrap();
        install(&root, "reader", json!({"folders": {"what": "vaults"}}));
        install(&root, "none", json!({}));
        crate::grants::grant("reader", &vault).unwrap();
        crate::grants::grant("none", &vault).unwrap();
        sync();
        // Read-only: reads, never writes.
        assert_eq!(run("reader", "stat", json!({"path": "A.md"})).unwrap()["kind"], "file");
        assert_eq!(run("reader", "search", json!({"q": "acme"})).unwrap().as_array().unwrap().len(), 1);
        assert!(run("reader", "snapshot", json!({})).is_ok(), "a copy into its own store");
        for (cmd, a) in [("write", json!({"path": "B.md", "text": "x", "opts": {"createNew": true}})), ("append", json!({"path": "log.md", "text": "x"})), ("mkdir", json!({"path": "X"}))] {
            assert!(run("reader", cmd, a).unwrap_err().contains("folders.write"), "{cmd}");
        }
        // Without uses.folders: nothing.
        for (cmd, a) in [("stat", json!({"path": "A.md"})), ("search", json!({"q": "acme"})), ("snapshot", json!({})), ("walk", json!({}))] {
            assert!(run("none", cmd, a).unwrap_err().contains("“folders”"), "{cmd}");
        }
        for id in ["reader", "none"] {
            plugins::remove(id, true).unwrap();
        }
        sync();
    }

    #[test]
    fn store_update_keeps_both_processes_writes() {
        let (_env, root) = paths::testenv::folders("jshost-folders-store");
        install(&root, "counter", json!({}));
        sync();
        // ctx.store.update: read, change, written only over what it read.
        assert_eq!(run("counter", "bump", json!({"path": "n.txt"})).unwrap(), "1");
        assert_eq!(run("counter", "bump", json!({"path": "n.txt"})).unwrap(), "2");
        assert_eq!(run("counter", "same", json!({"path": "n.txt"})).unwrap(), "2", "nothing to change: nothing written");
        // Another process's writer, as `echoctl plugin call` is beside the app: every read-change-write over the
        // file's sha, read again when refused — none lost, the plugin's own updates racing it.
        let other = std::thread::spawn(|| {
            for _ in 0..30 {
                loop {
                    let cur = service("counter", "store.read", &json!("n.txt")).unwrap();
                    let n: u64 = cur.as_str().unwrap().parse().unwrap();
                    match service("counter", "store.write", &json!(["n.txt", (n + 1).to_string(), {"ifSha": sha_hex(cur.as_str().unwrap().as_bytes())}])) {
                        Ok(_) => break,
                        Err(e) => assert!(e.to_string().contains("changed since"), "{e}"),
                    }
                }
            }
        });
        for _ in 0..30 {
            run("counter", "bump", json!({"path": "n.txt"})).unwrap();
        }
        other.join().unwrap();
        assert_eq!(run("counter", "storeRead", json!({"path": "n.txt"})).unwrap(), "62");
        // createNew and ifSha in the store; Echo's own files still refused.
        assert!(run("counter", "storeWrite", json!({"path": "n.txt", "text": "0", "opts": {"createNew": true}})).unwrap_err().contains("there already"));
        assert!(run("counter", "storeWrite", json!({"path": "n.txt", "text": "0", "opts": {"ifSha": sha_hex(b"61")}})).unwrap_err().contains("changed since"));
        assert!(run("counter", "storeWrite", json!({"path": "settings.json", "text": "{}"})).unwrap_err().contains("isn't its to write"));
        assert!(run("counter", "bump", json!({"path": "work.json"})).unwrap_err().contains("isn't its to write"));
        assert!(lock_file("counter").exists(), "written under the plugin's lock");
        plugins::remove("counter", false).unwrap();
        assert!(!lock_file("counter").exists(), "its lock goes with its data");
        sync();
    }

    #[test]
    #[cfg(unix)]
    fn a_claude_run_goes_into_a_folder_given_to_the_plugin_edits_only_with_write_and_copy_answers_its_snapshot() {
        let (_env, root) = paths::testenv::folders("jshost-folders-claude");
        let _fake = crate::ai::fake::install(&root);
        let p = crate::ai::fake::profile("work", None, &crate::ai::fake::signed_in("pat@example.com"));
        crate::ai::set_default("work").unwrap();
        let cwd = || std::fs::read_to_string(crate::ai::fake::file(&p, "p.cwd")).unwrap_or_default().trim().to_string();
        let started = || std::fs::read_to_string(crate::ai::fake::file(&p, "p.args")).is_ok();
        let forget = || {
            let _ = std::fs::remove_file(crate::ai::fake::file(&p, "p.args"));
        };
        let vault = root.join("acme-vault");
        std::fs::create_dir_all(vault.join(".claude/skills/tidy")).unwrap();
        std::fs::create_dir_all(vault.join("Notes")).unwrap();
        std::fs::write(vault.join("CLAUDE.md"), "# Acme vault\n").unwrap();
        std::fs::write(vault.join(".claude/skills/tidy/SKILL.md"), "---\nname: tidy\ndescription: Tidy.\n---\n").unwrap();
        std::fs::write(vault.join("Notes/Acme.md"), "# Acme\n").unwrap();
        std::fs::create_dir_all(vault.join(".git")).unwrap();
        let other = root.join("other");
        std::fs::create_dir_all(&other).unwrap();
        install(&root, "reader", json!({"claude": {}, "folders": {"what": "vaults"}}));
        install(&root, "writer", json!({"claude": {}, "folders": {"what": "vaults", "write": true}}));
        install(&root, "asker", json!({"claude": {}}));
        for id in ["reader", "writer"] {
            crate::grants::grant(id, &vault).unwrap();
        }
        sync();
        let vault = dunce::canonicalize(&vault).unwrap();
        let v = vault.display().to_string();
        let claude = |id: &str, mut a: Value| {
            a["prompt"] = json!("Tidy the notes");
            a["dir"] = json!("runs/x");
            service_async(id, "claude.run", &a)
        };
        // Read: in the folder, or a subfolder of it.
        let _ = claude("reader", json!({"folder": v, "mode": "read"}));
        assert_eq!(cwd(), v);
        let _ = claude("reader", json!({"folder": v, "path": "Notes"}));
        assert_eq!(cwd(), vault.join("Notes").display().to_string(), "read is the default mode");
        // Refused before anything starts: no folders on its sheet, a folder not given, a path out of it, a mode Echo
        // doesn't know, a path or mode with no folder, and editing without folders.write.
        forget();
        let refused = [
            claude("asker", json!({"folder": v})),
            claude("reader", json!({"folder": other.display().to_string()})),
            claude("reader", json!({"folder": v, "path": "../other"})),
            claude("reader", json!({"folder": v, "path": "Notes/Acme.md"})),
            claude("reader", json!({"folder": v, "mode": "bypassPermissions"})),
            claude("reader", json!({"mode": "edit"})),
            claude("reader", json!({"folder": v, "mode": "edit"})),
        ];
        let why: Vec<String> = refused.into_iter().map(|r| r.unwrap_err().to_string()).collect();
        assert!(why[0].contains("“folders”") && why[1].contains("isn't one of the folders you gave") && why[3].contains("isn't a folder"), "{why:?}");
        assert!(why[4].contains("read, edit or copy") && why[5].contains("go with a folder") && why[6].contains("“folders.write”"), "{why:?}");
        assert!(!started(), "nothing started");
        // Edit, with folders.write: in the folder itself.
        let _ = claude("writer", json!({"folder": v, "mode": "edit"}));
        assert_eq!(cwd(), v);
        // Copy (no write needed): in a snapshot of it in the store, its `.claude` too, the folder left as it was; the
        // answer names the snapshot.
        std::fs::write(crate::ai::fake::file(&p, "p.result"), json!({"type": "result", "subtype": "success", "result": "Tidied", "usage": {}}).to_string() + "\n").unwrap();
        let out = claude("reader", json!({"folder": v, "mode": "copy"})).unwrap();
        assert_eq!(out["answer"], "Tidied");
        let at = out["snapshot"]["path"].as_str().unwrap().to_string();
        assert!(at.starts_with("cache/snapshots/"), "{out}");
        let snap = dunce::canonicalize(plugins::data_dir("reader").join(&at)).unwrap();
        assert_eq!(cwd(), snap.display().to_string());
        assert!(snap.join("CLAUDE.md").is_file() && snap.join(".claude/skills/tidy/SKILL.md").is_file() && snap.join("Notes/Acme.md").is_file());
        // A copy run that fails leaves no snapshot behind (the plugin never got its name).
        std::fs::remove_file(crate::ai::fake::file(&p, "p.result")).unwrap();
        assert!(claude("reader", json!({"folder": v, "mode": "copy"})).is_err());
        let kept: Vec<_> = std::fs::read_dir(plugins::data_dir("reader").join("cache/snapshots")).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(kept.len(), 1, "only the answered one: {kept:?}");
        // A copy run into what a copy leaves out (`.git`): refused before Claude Code starts, its snapshot gone too.
        forget();
        let e = claude("reader", json!({"folder": v, "path": ".git", "mode": "copy"})).unwrap_err();
        assert!(e.to_string().contains("isn't in the copy of") && !started(), "{e}");
        let kept: Vec<_> = std::fs::read_dir(plugins::data_dir("reader").join("cache/snapshots")).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(kept.len(), 1, "no snapshot left by the refused run: {kept:?}");
        for id in ["reader", "writer", "asker"] {
            plugins::remove(id, true).unwrap();
        }
        sync();
    }
}
