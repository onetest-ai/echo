//! `ctx.git.*`: a granted folder's git history, changes and commits, for a plugin that may not start programs itself
//! (echo-plugins R17). Git runs here only with the verbs of `VERBS`, never through a shell, each path literal after
//! `--` (`GIT_LITERAL_PATHSPECS`), within `TIMEOUT` and the output caps, in an environment of a few named variables
//! (none of Echo's secrets, no `CLAUDE*` / `ANTHROPIC*`, no `GIT_*` of the user's), and with every setting that names a
//! program switched off on its command line (`overrides`): a vault's own `.git/config` runs nothing. No network verb.

use super::*;
use crate::app::NoWindow as _;
use std::io::Read;
use std::process::{Command, Stdio};

/// One git run at most.
const TIMEOUT: Duration = Duration::from_secs(20);
/// What is kept of git's output and of its errors; the rest is read and dropped.
const OUT_MAX: usize = 16 << 20;
const ERR_MAX: usize = 64 << 10;
/// `show`: the text given by default and at most.
const SHOW_MAX: u64 = 1 << 20;
const SHOW_MAX_ALL: u64 = 4 << 20;
/// `log`: commits by default and at most.
const LOG_MAX: u64 = 50;
const LOG_MAX_ALL: u64 = 1000;
/// Paths one call names at most; a commit message's characters at most.
const PATHS_MAX: usize = 1000;
const MESSAGE_MAX: usize = 5000;
/// The oldest git whose `GIT_CONFIG_COUNT` (how the overrides reach it) and `--path-format` Echo relies on.
const GIT_MIN: (u32, u32) = (2, 31);
/// The committer's address of a plugin's commits: `<id>@plugins.echo.invalid`.
const ECHO_MAIL: &str = "@plugins.echo.invalid";

/// The only verbs Echo runs.
const VERBS: &[&str] = &["version", "config", "rev-parse", "symbolic-ref", "status", "log", "cat-file", "ls-files", "init", "add", "commit", "rm"];

/// Settings whose value names a program (or a folder of programs) — by the last part of their name, in any section:
/// cleared on the command line wherever the repository, the user or the system set one.
const PROGRAM_KEYS: &[&str] = &[
    "clean", "smudge", "process", "command", "textconv", "driver", "helper", "cmd", "program", "sshcommand", "askpass", "editor", "pager", "hookspath",
    "fsmonitor", "virtualfilesystem", "defaultkeycommand", "proxy", "gitproxy", "uploadpack", "receivepack", "vcs", "external", "alternaterefscommand", "packobjectshook",
];

/// The git program: an absolute `git` (Windows: `git.exe`, never a `.cmd`, which runs through cmd.exe).
fn program() -> Result<PathBuf> {
    #[cfg(test)]
    if std::env::var_os("ECHOCTL_TEST_NO_GIT").is_some() {
        bail!("git isn't installed on this computer");
    }
    let name = if cfg!(windows) { "git.exe" } else { "git" };
    crate::app::program_candidates("git")
        .into_iter()
        .find(|p| p.is_absolute() && p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name)) && p.is_file())
        .ok_or_else(|| anyhow!("git isn't installed on this computer"))
}

/// Git's answer: whether it succeeded, its output (cut at `OUT_MAX`, `over` when there was more) and its errors.
struct Out {
    ok: bool,
    stdout: Vec<u8>,
    over: bool,
    stderr: Vec<u8>,
}

impl Out {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).trim_end().to_string()
    }

    /// What went wrong, in git's first line of it.
    fn why(&self) -> String {
        let all = String::from_utf8_lossy(&self.stderr);
        let line = all.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("git refused it");
        let line = line.strip_prefix("fatal: ").or_else(|| line.strip_prefix("error: ")).unwrap_or(line);
        line.chars().take(300).collect()
    }
}

/// The variables git gets from the user's environment; nothing else of it (no `GIT_*`, none of Echo's or Claude's).
const ENV_KEPT: &[&str] = &["PATH", "HOME", "USERPROFILE", "HOMEDRIVE", "HOMEPATH", "SystemRoot", "SYSTEMROOT", "TEMP", "TMP", "TMPDIR", "XDG_CONFIG_HOME"];

/// An empty folder git is told holds the hooks: none ever runs.
fn no_hooks() -> Result<PathBuf> {
    let d = paths::state_dir().join("plugins").join(".locks").join("git-no-hooks");
    std::fs::create_dir_all(&d)?;
    if std::fs::read_dir(&d)?.next().is_some() {
        bail!("{} isn't empty: Echo runs no git hooks", d.display());
    }
    Ok(d)
}

/// `git version` once: refused below `GIT_MIN`, whose overrides through the environment it would ignore.
fn checked_version(git: &Path) -> Result<()> {
    static SEEN: OnceLock<Mutex<Vec<PathBuf>>> = OnceLock::new();
    let seen = SEEN.get_or_init(Default::default);
    if seen.lock().unwrap_or_else(|e| e.into_inner()).contains(&git.to_path_buf()) {
        return Ok(());
    }
    let o = spawn(git, Path::new("."), &["version"], &[])?;
    let text = o.text();
    let v: Vec<u32> = text.trim_start_matches("git version ").split(|c: char| !c.is_ascii_digit()).take(2).filter_map(|n| n.parse().ok()).collect();
    if v.len() < 2 || (v[0], v[1]) < GIT_MIN {
        bail!("Echo needs git {}.{} or newer (this is “{}”)", GIT_MIN.0, GIT_MIN.1, text.chars().take(60).collect::<String>());
    }
    seen.lock().unwrap_or_else(|e| e.into_inner()).push(git.to_path_buf());
    Ok(())
}

/// Every setting that could start a program, as the command line's own (`GIT_CONFIG_COUNT`), which outranks the
/// system's, the user's and the repository's: those of `PROGRAM_KEYS` found in any of them (filters, diff and merge
/// drivers, credential helpers, signing programs, `includeIf` files' too) cleared, a filter not required, every
/// command's pager off — then the fixed ones, last so they win.
fn overrides(git: &Path, dir: &Path, hooks: &Path) -> Result<Vec<(String, String)>> {
    let fixed: Vec<(String, String)> = [
        ("core.hooksPath", hooks.to_string_lossy().as_ref()),
        ("core.fsmonitor", "false"),
        ("core.useBuiltinFSMonitor", "false"),
        ("core.pager", ""),
        ("core.editor", ":"),
        ("sequence.editor", ":"),
        ("credential.helper", ""),
        ("diff.external", ""),
        ("commit.gpgSign", "false"),
        ("tag.gpgSign", "false"),
        ("log.showSignature", "false"),
        ("maintenance.auto", "false"),
        ("gc.auto", "0"),
        ("submodule.recurse", "false"),
        ("status.submoduleSummary", "false"),
        ("diff.ignoreSubmodules", "all"),
        ("protocol.allow", "never"),
        ("core.quotePath", "false"),
        ("color.ui", "never"),
        ("i18n.logOutputEncoding", "UTF-8"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    // Read with the fixed ones in force (a config read runs nothing).
    let listed = spawn(git, dir, &["config", "--list", "-z"], &fixed)?;
    let mut out = vec![];
    for entry in listed.stdout.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let entry = String::from_utf8_lossy(entry);
        let key = entry.split('\n').next().unwrap_or("");
        let (section, last) = (key.split('.').next().unwrap_or("").to_ascii_lowercase(), key.rsplit('.').next().unwrap_or("").to_ascii_lowercase());
        if section == "pager" || (section == "filter" && last == "required") {
            out.push((key.to_string(), "false".to_string()));
        } else if PROGRAM_KEYS.contains(&last.as_str()) && key.contains('.') {
            out.push((key.to_string(), String::new()));
        }
    }
    out.extend(fixed);
    Ok(out)
}

/// `git <args>` in `dir` with only `config` set on its command line: no shell, no stdin, its own process group, killed
/// at `TIMEOUT`, both streams read as they come (a full pipe would stall it) and capped.
fn spawn(git: &Path, dir: &Path, args: &[&str], config: &[(String, String)]) -> Result<Out> {
    spawn_within(git, dir, args, config, TIMEOUT)
}

fn spawn_within(git: &Path, dir: &Path, args: &[&str], config: &[(String, String)], timeout: Duration) -> Result<Out> {
    if !args.first().is_some_and(|v| VERBS.contains(v)) {
        bail!("Echo doesn't run “git {}”", args.first().unwrap_or(&""));
    }
    let mut c = Command::new(git).no_window();
    c.arg("--no-pager").args(args).current_dir(dir).env_clear();
    for k in ENV_KEPT {
        if let Some(v) = std::env::var_os(k) {
            c.env(k, v);
        }
    }
    c.env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .env("GIT_EDITOR", ":")
        .env("GIT_PAGER", "")
        .env("GIT_CONFIG_COUNT", config.len().to_string());
    for (n, (k, v)) in config.iter().enumerate() {
        c.env(format!("GIT_CONFIG_KEY_{n}"), k).env(format!("GIT_CONFIG_VALUE_{n}"), v);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    let mut child = c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| anyhow!("git couldn't start: {e}"))?;
    fn drain<R: Read + Send + 'static>(r: Option<R>, cap: usize) -> std::thread::JoinHandle<(Vec<u8>, bool)> {
        std::thread::spawn(move || {
            let (mut kept, mut over, mut buf) = (Vec::new(), false, [0u8; 65536]);
            if let Some(mut r) = r {
                while let Ok(n) = r.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    let room = cap.saturating_sub(kept.len());
                    kept.extend_from_slice(&buf[..n.min(room)]);
                    over |= n > room;
                }
            }
            (kept, over)
        })
    }
    let (out, err) = (drain(child.stdout.take(), OUT_MAX), drain(child.stderr.take(), ERR_MAX));
    let t = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let ((stdout, over), (stderr, _)) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
                return Ok(Out { ok: status.success(), stdout, over, stderr });
            }
            Ok(None) if t.elapsed() < timeout => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                stop(&mut child);
                bail!("git took longer than {} s and was stopped", timeout.as_secs());
            }
        }
    }
}

/// SIGTERM to its group first (git removes its lock files), SIGKILL a second later.
fn stop(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let pg = child.id() as i32;
        // SAFETY: signalling the process group of our own child, which it leads (`process_group(0)`).
        unsafe {
            libc::kill(-pg, libc::SIGTERM);
        }
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(1) {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        // SAFETY: as above.
        unsafe {
            libc::kill(-pg, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Git, ready to run in one granted folder: the program, the folder (canonical) and the overrides for it.
struct Git {
    git: PathBuf,
    root: PathBuf,
    config: Vec<(String, String)>,
}

/// Where the root sits in its repository.
struct Repo {
    /// The root's path in the repository (`Notes/` when it is a subfolder; empty at its top).
    prefix: String,
    git_dir: PathBuf,
}

impl Git {
    fn new(root: PathBuf) -> Result<Git> {
        let git = program()?;
        checked_version(&git)?;
        let config = overrides(&git, &root, &no_hooks()?)?;
        Ok(Git { git, root, config })
    }

    fn run(&self, args: &[&str]) -> Result<Out> {
        spawn(&self.git, &self.root, args, &self.config)
    }

    /// `args` that must succeed: their output.
    fn ok(&self, args: &[&str]) -> Result<Out> {
        let o = self.run(args)?;
        if !o.ok {
            bail!("{}", o.why());
        }
        if o.over {
            bail!("git's answer was over {} MB", OUT_MAX >> 20);
        }
        Ok(o)
    }

    /// The repository the root is in, or None. Only one whose files are all in it: its `.git` (and the common one)
    /// inside its own top folder, the root inside that, no objects borrowed from elsewhere — a `gitdir:` file, a
    /// `core.worktree` or alternates would let a vault's settings show another repository's files.
    fn repo(&self) -> Result<Option<Repo>> {
        let o = self.run(&["rev-parse", "--show-prefix", "--path-format=absolute", "--show-toplevel", "--git-dir", "--git-common-dir"])?;
        if !o.ok {
            let why = o.why();
            if why.contains("not a git repository") {
                return Ok(None);
            }
            bail!("{why}");
        }
        let text = o.text();
        let lines: Vec<&str> = text.lines().collect();
        let [prefix, top, git_dir, common] = lines[..] else { bail!("this folder isn't in a repository's files (a bare repository or inside .git)") };
        let real = |p: &str| dunce::canonicalize(p).map_err(|e| anyhow!("{p}: {e}"));
        let (top, git_dir, common) = (real(top)?, real(git_dir)?, real(common)?);
        if !self.root.starts_with(&top) || !git_dir.starts_with(&top) || !common.starts_with(&top) {
            bail!("this repository keeps its history outside its folder (a linked worktree, a submodule or core.worktree): Echo doesn't use it");
        }
        if common.join("objects/info/alternates").exists() {
            bail!("this repository borrows objects from another one (alternates): Echo doesn't use it");
        }
        Ok(Some(Repo { prefix: prefix.to_string(), git_dir }))
    }

    fn need_repo(&self) -> Result<Repo> {
        self.repo()?.ok_or_else(|| anyhow!("this folder isn't in a git repository"))
    }

    /// The commit HEAD is at, or None on a branch with no commit yet.
    fn head(&self) -> Result<Option<String>> {
        let o = self.run(&["rev-parse", "-q", "--verify", "HEAD^{commit}"])?;
        Ok(o.ok.then(|| o.text()).filter(|h| !h.is_empty()))
    }
}

/// The granted root (canonical), for a plugin with `uses.folders`.
fn read_root(id: &str, root: &str) -> Result<PathBuf> {
    uses(id, "folders")?;
    crate::grants::resolve(id, root, "")
}

/// …and one it may change (`folders.write`), not while Claude prepares and asks.
fn write_root(id: &str, root: &str) -> Result<PathBuf> {
    read_only_here("changing files or settings")?;
    uses_flag(id, "folders", "write")?;
    read_root(id, root)
}

/// The paths a call names (1 to `PATHS_MAX`), each checked by `check` and given as git sees it: relative to the root,
/// by its real names, with `/`.
fn named(root: &Path, list: &Value, check: impl Fn(&str) -> Result<PathBuf>) -> Result<Vec<String>> {
    let list = list.as_array().ok_or_else(|| anyhow!("paths is a list of paths in the folder"))?;
    if list.is_empty() || list.len() > PATHS_MAX {
        bail!("paths names 1 to {PATHS_MAX} files");
    }
    list.iter()
        .map(|p| {
            let rel = p.as_str().ok_or_else(|| anyhow!("paths is a list of paths in the folder"))?;
            let real = check(rel)?;
            let rel = real.strip_prefix(root).map_err(|_| anyhow!("“{rel}” leads outside the folder"))?;
            if rel.as_os_str().is_empty() {
                bail!("not the folder itself");
            }
            Ok(rel.to_string_lossy().replace('\\', "/"))
        })
        .collect()
}

/// `ctx.git.info(root)`.
pub(super) fn info(id: &str, root: &str) -> Result<Value> {
    let g = Git::new(read_root(id, root)?)?;
    let Some(repo) = g.repo()? else { return Ok(json!({"repo": false})) };
    // A branch with no commit yet has its name too; a detached HEAD has none.
    let branch = g.run(&["symbolic-ref", "-q", "--short", "HEAD"])?;
    let branch = Some(branch.text()).filter(|b| branch.ok && !b.is_empty());
    let busy = [
        ("MERGE_HEAD", "merge"),
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
        ("REVERT_HEAD", "revert"),
        ("BISECT_LOG", "bisect"),
    ]
    .iter()
    .find(|(f, _)| repo.git_dir.join(f).exists())
    .map(|(_, w)| *w);
    Ok(json!({"repo": true, "branch": branch, "head": g.head()?, "detached": branch.is_none(), "busy": busy, "subfolder": !repo.prefix.is_empty()}))
}

/// `ctx.git.status(root, {paths})`: `[{path, code, from?}]` — git's two-letter code (porcelain v1: `" M"`, `"A "`,
/// `"??"`…), the paths relative to the root.
pub(super) fn status(id: &str, root: &str, o: &Value) -> Result<Value> {
    let real_root = read_root(id, root)?;
    let paths = match o.get("paths").filter(|p| !p.is_null()) {
        Some(list) => named(&real_root, list, |rel| crate::grants::resolve(id, root, rel))?,
        None => vec![".".into()],
    };
    let g = Git::new(real_root)?;
    let repo = g.need_repo()?;
    let mut args = vec!["status", "--porcelain=v1", "-z", "--untracked-files=all", "--ignore-submodules=all", "--"];
    args.extend(paths.iter().map(String::as_str));
    let out = g.ok(&args)?;
    let rel = |p: &[u8]| String::from_utf8_lossy(p).strip_prefix(repo.prefix.as_str()).map(str::to_owned);
    let mut list = vec![];
    let mut items = out.stdout.split(|b| *b == 0).filter(|e| !e.is_empty());
    while let Some(e) = items.next() {
        if e.len() < 4 {
            continue;
        }
        let code = String::from_utf8_lossy(&e[..2]).into_owned();
        // A rename or copy: its old path follows.
        let from = if code.contains('R') || code.contains('C') { items.next().and_then(rel) } else { None };
        let Some(path) = rel(&e[3..]) else { continue };
        let mut item = json!({"path": path, "code": code});
        if let Some(f) = from {
            item["from"] = json!(f);
        }
        list.push(item);
    }
    Ok(json!(list))
}

/// `ctx.git.log(root, {path, max})`: the commits touching the root — or the file at `path`, through its renames, with
/// its path at each — newest first: `[{rev, author, email, date, subject, echo, path?}]`; `echo` names the plugin
/// that committed it through Echo, else null.
pub(super) fn log(id: &str, root: &str, o: &Value) -> Result<Value> {
    let real_root = read_root(id, root)?;
    let path = match o["path"].as_str().filter(|p| !p.is_empty()) {
        Some(p) => Some(crate::grants::plain(&real_root, p)?.to_string_lossy().replace('\\', "/")),
        None => None,
    };
    let max = o["max"].as_u64().filter(|m| *m > 0).unwrap_or(LOG_MAX).min(LOG_MAX_ALL).to_string();
    let g = Git::new(real_root)?;
    let repo = g.need_repo()?;
    if g.head()?.is_none() {
        return Ok(json!([]));
    }
    let max = format!("--max-count={max}");
    let mut args = vec!["log", "-z", "--no-show-signature", "--no-notes", "--format=%H%x00%an%x00%ae%x00%aI%x00%ce%x00%s", max.as_str()];
    if path.is_some() {
        args.extend(["--follow", "--name-only"]);
    }
    args.push("--");
    args.push(path.as_deref().unwrap_or("."));
    let out = g.ok(&args)?;
    // NUL-separated: six fields a commit, then (following a file) its path as a field of its own after a newline.
    let mut fields = out.stdout.split(|b| *b == 0).map(|f| String::from_utf8_lossy(f).into_owned()).peekable();
    let mut list = vec![];
    while let Some(rev) = fields.next() {
        let rev = rev.trim_start_matches('\n').to_string();
        if rev.is_empty() {
            continue;
        }
        let f: Vec<String> = fields.by_ref().take(5).collect();
        let [author, email, date, committer, subject] = &f[..] else { break };
        let mut item = json!({"rev": rev, "author": author, "email": email, "date": date, "subject": subject, "echo": committer.strip_suffix(ECHO_MAIL).filter(|p| plugins::valid_id(p))});
        if path.is_some() && fields.peek().is_some_and(|n| n.starts_with('\n')) {
            let at = fields.next().unwrap_or_default();
            item["path"] = json!(at.trim_start_matches('\n').strip_prefix(repo.prefix.as_str()));
        }
        list.push(item);
    }
    Ok(json!(list))
}

/// A version git may be asked for: `HEAD`, `HEAD~N` or a commit's id (4–64 hex) — never an option or a range.
fn valid_rev(rev: &str) -> bool {
    let hex = |s: &str| (4..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit());
    rev == "HEAD" || rev.strip_prefix("HEAD~").is_some_and(|n| (1..=6).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit())) || hex(rev)
}

/// `ctx.git.show(root, rev, path, {max})`: `{text, size, truncated}` of the file at that version, or null when it
/// wasn't there; its text as it is in the repository (no filter, no conversion), cut at `max`.
pub(super) fn show(id: &str, root: &str, rev: &str, path: &str, o: &Value) -> Result<Value> {
    let real_root = read_root(id, root)?;
    if !valid_rev(rev) {
        bail!("“{}” isn't a version: HEAD, HEAD~N or a commit's id", rev.chars().take(80).collect::<String>());
    }
    let path = crate::grants::plain(&real_root, path)?.to_string_lossy().replace('\\', "/");
    let max = o["max"].as_u64().filter(|m| *m > 0).unwrap_or(SHOW_MAX).min(SHOW_MAX_ALL) as usize;
    let g = Git::new(real_root)?;
    g.need_repo()?;
    // `./` — relative to the root: never a file of the repository outside it.
    let spec = format!("{rev}:./{path}");
    let kind = g.run(&["cat-file", "-t", &spec])?;
    if !kind.ok {
        return Ok(Value::Null);
    }
    if kind.text() != "blob" {
        bail!("“{path}” is a folder at that version");
    }
    let size: u64 = g.ok(&["cat-file", "-s", &spec])?.text().parse().unwrap_or(0);
    if size > OUT_MAX as u64 {
        bail!("“{path}” is over {} MB at that version", OUT_MAX >> 20);
    }
    let blob = g.ok(&["cat-file", "blob", &spec])?;
    let cut = blob.stdout.len() > max;
    let text = String::from_utf8_lossy(&blob.stdout[..blob.stdout.len().min(max)]).into_owned();
    Ok(json!({"text": text, "size": size, "truncated": cut}))
}

/// A branch name `init` may make: plain letters, digits, `.` `_` `-` `/`, not starting with `-` or `.`, no `..`.
fn valid_branch(b: &str) -> bool {
    (1..=100).contains(&b.len())
        && b.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-/".contains(&c))
        && !b.starts_with(['-', '.', '/'])
        && !b.ends_with(['/', '.'])
        && !b.contains("..")
        && !b.contains("//")
        && !b.ends_with(".lock")
}

/// `ctx.git.init(root, {branch})`: a new repository in the root (on `main` unless `branch`), never inside another one,
/// with no template (no sample hooks). Its `info`.
pub(super) fn init(id: &str, root: &str, o: &Value) -> Result<Value> {
    let real_root = write_root(id, root)?;
    let branch = o["branch"].as_str().unwrap_or("main");
    if !valid_branch(branch) {
        bail!("“{}” isn't a branch name Echo makes", branch.chars().take(100).collect::<String>());
    }
    let g = Git::new(real_root)?;
    if g.repo()?.is_some() {
        bail!("this folder is in a git repository already");
    }
    let initial = format!("--initial-branch={branch}");
    g.ok(&["init", "-q", "--template=", &initial])?;
    info(id, root)
}

/// Who a plugin's commit is by, as command-line settings: Echo, with the plugin's id, commits it; the user is its
/// author when git knows their name and address (theirs, or the repository's), else Echo is that too.
fn identity(g: &Git, id: &str) -> Vec<(String, String)> {
    let get = |k: &str| g.run(&["config", "--get", k]).ok().filter(|o| o.ok).map(|o| o.text()).filter(|v| !v.trim().is_empty());
    let (name, mail) = (format!("Echo ({id})"), format!("{id}{ECHO_MAIL}"));
    let mut who = vec![("committer.name".to_string(), name.clone()), ("committer.email".to_string(), mail.clone())];
    if get("user.name").is_none() || get("user.email").is_none() {
        who.extend([("author.name".to_string(), name), ("author.email".to_string(), mail)]);
    }
    who
}

/// `ctx.git.commit(root, {paths, message})`: one commit of exactly those files as they are on the disk (a file gone:
/// its removal), the user's other staged and unstaged work left as it is; each path checked as `ctx.fs.write` checks it.
/// `{rev}`, or `{rev: null}` when they hold no change. No hook runs.
pub(super) fn commit(id: &str, root: &str, o: &Value) -> Result<Value> {
    let real_root = write_root(id, root)?;
    let message = o["message"].as_str().unwrap_or("").trim();
    if message.is_empty() || message.chars().count() > MESSAGE_MAX || message.contains('\0') {
        bail!("a commit's message is plain text of 1 to {MESSAGE_MAX} characters");
    }
    let paths = named(&real_root, &o["paths"], |rel| {
        let f = crate::grants::writable(id, root, rel)?;
        if f.is_dir() {
            bail!("“{rel}” is a folder: commit names files");
        }
        Ok(f)
    })?;
    let g = Git::new(real_root)?;
    g.need_repo()?;
    // Files git doesn't know yet go into its index first (`--only` takes only known ones); taken out again on a failure.
    let mut known = vec!["ls-files", "-z", "--"];
    known.extend(paths.iter().map(String::as_str));
    let known = g.ok(&known)?;
    let known: Vec<String> = known.stdout.split(|b| *b == 0).filter(|e| !e.is_empty()).map(|e| String::from_utf8_lossy(e).into_owned()).collect();
    let new: Vec<&str> = paths.iter().map(String::as_str).filter(|p| !known.iter().any(|k| k == p)).collect();
    if let Some(gone) = new.iter().find(|p| !g.root.join(p).exists()) {
        bail!("“{gone}” isn't there and git doesn't know it");
    }
    let mut changed = vec!["status", "--porcelain=v1", "-z", "--untracked-files=all", "--ignore-submodules=all", "--"];
    changed.extend(paths.iter().map(String::as_str));
    if g.ok(&changed)?.stdout.is_empty() {
        return Ok(json!({"rev": null}));
    }
    if !new.is_empty() {
        let mut add = vec!["add", "--"];
        add.extend(new.iter().copied());
        g.ok(&add)?;
    }
    let message = format!("--message={message}");
    let mut args = vec!["commit", "--only", "-q", "--no-verify", "--no-gpg-sign", "--cleanup=whitespace", message.as_str(), "--"];
    args.extend(paths.iter().map(String::as_str));
    let mut config = g.config.clone();
    config.extend(identity(&g, id));
    match spawn(&g.git, &g.root, &args, &config) {
        Ok(o) if o.ok => Ok(json!({"rev": g.head()?})),
        other => {
            if !new.is_empty() {
                let mut back = vec!["rm", "--cached", "-q", "--ignore-unmatch", "--"];
                back.extend(new.iter().copied());
                let _ = g.run(&back);
            }
            bail!("git didn't commit: {}", other.map(|o| o.why()).unwrap_or_else(|e| format!("{e:#}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN: &str = r#"
        export function apply(ctx) {
          const r = () => ctx.fs.folders()[0]
          ctx.command('info', async () => await ctx.git.info(r()))
          ctx.command('status', async (a) => await ctx.git.status(r(), a))
          ctx.command('log', async (a) => await ctx.git.log(r(), a))
          ctx.command('show', async (a) => await ctx.git.show(r(), a.rev, a.path, a))
          ctx.command('init', async (a) => await ctx.git.init(r(), a))
          ctx.command('commit', async (a) => await ctx.git.commit(r(), a))
        }
    "#;

    /// Plugin `id` installed from a local marketplace, `dir` given to it.
    fn install(root: &Path, id: &str, uses: Value, dir: &Path) {
        let mk = root.join("market").join(id);
        let d = mk.join("plugins").join(id);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("echo-plugin.json"), json!({"id": id, "name": id, "version": "0.1.0", "main": "main.js", "uses": uses}).to_string()).unwrap();
        std::fs::write(d.join("main.js"), MAIN).unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": format!("m-{id}"), "plugins": [{"id": id, "source": format!("./plugins/{id}")}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some(&format!("m-{id}"))).unwrap();
        plugins::refresh(&name).unwrap();
        let agreed = plugins::review(id, Some(&name)).unwrap().uses;
        plugins::install(id, Some(&name), &agreed).unwrap();
        std::fs::create_dir_all(dir).unwrap();
        crate::grants::grant(id, dir).unwrap();
        sync();
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

    /// The user's own git, as they would run it: their identity from the test's home.
    fn user_git(dir: &Path, args: &[&str]) -> String {
        let o = Command::new(program().unwrap()).arg("-C").arg(dir).args(args).output().unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).into_owned()
    }

    /// The test's home holds the user's git identity (`HOME` is the test's own `user/`).
    fn user_identity(root: &Path) {
        std::fs::write(root.join("user/.gitconfig"), "[user]\n\tname = Sam Rivera\n\temail = sam@initech.example\n[commit]\n\tgpgsign = false\n").unwrap();
    }

    fn rel(v: &Value, key: &str) -> Vec<String> {
        v.as_array().unwrap().iter().map(|x| x[key].as_str().unwrap_or("").to_string()).collect()
    }

    #[test]
    fn a_plugin_starts_a_repository_commits_and_reads_its_history_and_versions() {
        let (_env, root) = paths::testenv::folders("jshost-git-verbs");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let vault = root.join("vault");
        install(&root, "keeper", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        assert_eq!(run("keeper", "info", json!({})).unwrap(), json!({"repo": false}));
        assert!(run("keeper", "status", json!({})).unwrap_err().contains("isn't in a git repository"));
        assert!(run("keeper", "init", json!({"branch": "-x"})).unwrap_err().contains("isn't a branch name"));
        let i = run("keeper", "init", json!({})).unwrap();
        assert_eq!((i["repo"].clone(), i["branch"].clone(), i["head"].clone(), i["subfolder"].clone()), (json!(true), json!("main"), Value::Null, json!(false)));
        assert!(!vault.join(".git/hooks").exists() || std::fs::read_dir(vault.join(".git/hooks")).unwrap().next().is_none(), "no template: no sample hooks");
        assert!(run("keeper", "init", json!({})).unwrap_err().contains("in a git repository already"));
        assert_eq!(run("keeper", "log", json!({})).unwrap(), json!([]), "no commit yet");

        std::fs::create_dir_all(vault.join("Notes")).unwrap();
        std::fs::write(vault.join("Notes/Acme.md"), "# Acme\nv1\n").unwrap();
        std::fs::write(vault.join("Notes/Globex.md"), "# Globex\n").unwrap();
        let st = run("keeper", "status", json!({})).unwrap();
        assert_eq!(rel(&st, "path"), ["Notes/Acme.md", "Notes/Globex.md"]);
        assert_eq!(rel(&st, "code"), ["??", "??"]);
        assert_eq!(rel(&run("keeper", "status", json!({"paths": ["Notes/Acme.md"]})).unwrap(), "path"), ["Notes/Acme.md"]);
        let first = run("keeper", "commit", json!({"paths": ["Notes/Acme.md"], "message": "Acme: first notes"})).unwrap()["rev"].as_str().unwrap().to_string();
        assert_eq!(rel(&run("keeper", "status", json!({})).unwrap(), "path"), ["Notes/Globex.md"], "only its path was committed");
        assert_eq!(run("keeper", "commit", json!({"paths": ["Notes/Acme.md"], "message": "again"})).unwrap(), json!({"rev": null}), "nothing changed");

        std::fs::write(vault.join("Notes/Acme.md"), "# Acme\nv2\n").unwrap();
        assert_eq!(rel(&run("keeper", "status", json!({})).unwrap(), "code"), [" M", "??"]);
        run("keeper", "commit", json!({"paths": ["Notes/Acme.md"], "message": "Acme: second"})).unwrap();
        user_git(&vault, &["mv", "Notes/Acme.md", "Notes/Acme Corp.md"]);
        user_git(&vault, &["commit", "-q", "-m", "renamed by the user"]);

        let log = run("keeper", "log", json!({"path": "Notes/Acme Corp.md"})).unwrap();
        assert_eq!(rel(&log, "subject"), ["renamed by the user", "Acme: second", "Acme: first notes"]);
        assert_eq!(rel(&log, "path"), ["Notes/Acme Corp.md", "Notes/Acme.md", "Notes/Acme.md"], "through the rename");
        assert_eq!(log[0]["echo"], Value::Null, "the user's own commit");
        assert_eq!((log[1]["echo"].clone(), log[1]["author"].clone(), log[1]["email"].clone()), (json!("keeper"), json!("Sam Rivera"), json!("sam@initech.example")), "the user's author, Echo's committer");
        assert_eq!(user_git(&vault, &["log", "-1", "--skip=1", "--format=%cn <%ce>"]).trim(), "Echo (keeper) <keeper@plugins.echo.invalid>");
        assert_eq!(run("keeper", "log", json!({"max": 1})).unwrap().as_array().unwrap().len(), 1);

        assert_eq!(run("keeper", "show", json!({"rev": first, "path": "Notes/Acme.md"})).unwrap(), json!({"text": "# Acme\nv1\n", "size": 10, "truncated": false}));
        assert_eq!(run("keeper", "show", json!({"rev": "HEAD~1", "path": "Notes/Acme.md"})).unwrap()["text"], "# Acme\nv2\n");
        assert_eq!(run("keeper", "show", json!({"rev": "HEAD", "path": "Notes/Acme.md"})).unwrap(), Value::Null, "not there at that version");
        assert_eq!(run("keeper", "show", json!({"rev": "HEAD", "path": "Notes/Acme Corp.md", "max": 4})).unwrap(), json!({"text": "# Ac", "size": 10, "truncated": true}));
        assert!(run("keeper", "show", json!({"rev": "HEAD", "path": "Notes"})).unwrap_err().contains("a folder"));
        for bad in ["--output=/tmp/x", "HEAD..main", "main", "HEAD^{tree}", "@{-1}"] {
            assert!(run("keeper", "show", json!({"rev": bad, "path": "Notes/Acme.md"})).unwrap_err().contains("isn't a version"), "{bad}");
        }
        assert!(run("keeper", "show", json!({"rev": "HEAD", "path": "../outside.md"})).unwrap_err().contains("isn't a path inside"));

        // A file removed is committed as removed.
        std::fs::remove_file(vault.join("Notes/Acme Corp.md")).unwrap();
        run("keeper", "commit", json!({"paths": ["Notes/Acme Corp.md"], "message": "Acme: gone"})).unwrap();
        assert_eq!(user_git(&vault, &["ls-files"]).trim(), "", "nothing tracked: Globex was never committed");
        let i = run("keeper", "info", json!({})).unwrap();
        assert_eq!((i["branch"].clone(), i["detached"].clone(), i["busy"].clone()), (json!("main"), json!(false), Value::Null));
        assert_eq!(i["head"].as_str().unwrap().len(), 40);
        plugins::remove("keeper", true).unwrap();
        sync();
    }

    #[test]
    fn a_commit_takes_only_its_paths_literally_and_leaves_the_users_work_as_it_is() {
        let (_env, root) = paths::testenv::folders("jshost-git-literal");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let top = root.join("repo");
        let vault = top.join("Vault");
        std::fs::create_dir_all(&vault).unwrap();
        user_git(&top, &["init", "-q", "-b", "main"]);
        std::fs::write(top.join("README.md"), "# Initech\n").unwrap();
        std::fs::write(vault.join("a1.md"), "one\n").unwrap();
        user_git(&top, &["add", "-A"]);
        user_git(&top, &["commit", "-q", "-m", "the user's start"]);
        install(&root, "lit", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        // The user's own work: a staged change above the vault, an unstaged one inside it.
        std::fs::write(top.join("README.md"), "# Initech, staged\n").unwrap();
        user_git(&top, &["add", "README.md"]);
        std::fs::write(vault.join("a1.md"), "one, the user's\n").unwrap();
        for name in ["a*.md", ":(glob)x.md", "-n.md", "[b].md"] {
            std::fs::write(vault.join(name), "Echo's\n").unwrap();
        }
        let i = run("lit", "info", json!({})).unwrap();
        assert_eq!(i["subfolder"], true, "a vault inside a larger repository");
        let mut st = rel(&run("lit", "status", json!({})).unwrap(), "path");
        st.sort();
        assert_eq!(st, ["-n.md", ":(glob)x.md", "[b].md", "a*.md", "a1.md"], "the vault's own files only, relative to it");
        for name in ["a*.md", ":(glob)x.md", "-n.md", "[b].md"] {
            run("lit", "commit", json!({"paths": [name], "message": format!("Echo: {name}")})).unwrap();
            assert_eq!(user_git(&top, &["-c", "core.quotePath=false", "show", "--format=", "--name-only", "HEAD"]).trim(), format!("Vault/{name}"), "{name} is itself, not a pattern");
        }
        let status = user_git(&top, &["status", "--porcelain"]);
        assert!(status.contains("M  README.md") && status.contains(" M Vault/a1.md"), "the user's staged and unstaged work stays: {status}");
        let subjects = rel(&run("lit", "log", json!({})).unwrap(), "subject");
        assert_eq!(subjects.last().unwrap(), "the user's start");
        assert!(run("lit", "show", json!({"rev": "HEAD", "path": "README.md"})).unwrap().is_null(), "./ — never a file above the vault");
        // Refused as ctx.fs.write refuses them: hidden, a link, outside, nothing there.
        std::fs::create_dir_all(root.join("outside")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("outside"), vault.join("Out")).unwrap();
            std::os::unix::fs::symlink(vault.join("a1.md"), vault.join("Link.md")).unwrap();
        }
        let status = user_git(&top, &["status", "--porcelain"]);
        #[cfg(unix)]
        {
            assert!(run("lit", "commit", json!({"paths": ["Out"], "message": "x"})).unwrap_err().contains("leads outside"));
            assert!(run("lit", "commit", json!({"paths": ["Link.md"], "message": "x"})).unwrap_err().contains("is a link"));
        }
        for (p, why) in [(".obsidian/app.json", "hidden"), ("../README.md", "isn't a path inside"), ("Missing/none.md", "isn't there and git doesn't know it")] {
            assert!(run("lit", "commit", json!({"paths": [p], "message": "x"})).unwrap_err().contains(why), "{p}");
        }
        assert!(run("lit", "commit", json!({"paths": [], "message": "x"})).unwrap_err().contains("1 to 1000"));
        assert!(run("lit", "commit", json!({"paths": ["a1.md"], "message": "  "})).unwrap_err().contains("plain text"));
        assert!(run("lit", "commit", json!({"paths": ["a1.md"], "message": "x".repeat(MESSAGE_MAX + 1)})).unwrap_err().contains("plain text"));
        assert_eq!(user_git(&top, &["status", "--porcelain"]), status, "nothing staged or committed by a refusal");
        plugins::remove("lit", true).unwrap();
        sync();
    }

    /// A repository whose settings, hooks and attributes each run a script that leaves a mark in `marks`, were git to
    /// run them.
    #[cfg(unix)]
    fn trap(dir: &Path, marks: &Path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(dir).unwrap();
        std::fs::create_dir_all(marks).unwrap();
        let tag = dir.file_name().unwrap().to_string_lossy().into_owned();
        let script = |name: &str| {
            let f = marks.parent().unwrap().join(format!("{tag}-{name}.sh"));
            std::fs::write(&f, format!("#!/bin/sh\ntouch \"{}/{name}\"\ncat\n", marks.display())).unwrap();
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
            f.display().to_string()
        };
        user_git(dir, &["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("a.md"), "# A\n").unwrap();
        std::fs::write(dir.join(".gitattributes"), "*.md filter=evil diff=evil\n").unwrap();
        user_git(dir, &["add", "-A"]);
        user_git(dir, &["-c", "core.hooksPath=/nonexistent", "commit", "-q", "-m", "start"]);
        for hook in ["pre-commit", "prepare-commit-msg", "commit-msg", "post-commit", "reference-transaction", "post-index-change"] {
            let s = script(hook);
            for d in [".git/hooks", "hooks-here"] {
                std::fs::create_dir_all(dir.join(d)).unwrap();
                std::fs::copy(&s, dir.join(d).join(hook)).unwrap();
            }
        }
        let included = marks.parent().unwrap().join(format!("{tag}-included.gitconfig"));
        std::fs::write(&included, format!("[core]\n\tsshCommand = {}\n[filter \"inc\"]\n\tclean = {}\n", script("ssh"), script("included-filter"))).unwrap();
        let mut cfg = std::fs::read_to_string(dir.join(".git/config")).unwrap();
        cfg.push_str(&format!(
            "[core]\n\thooksPath = hooks-here\n\tfsmonitor = {}\n\tpager = {}\n\teditor = {}\n\taskPass = {}\n\
             [filter \"evil\"]\n\tclean = {}\n\tsmudge = {}\n\trequired = true\n\
             [diff \"evil\"]\n\ttextconv = {}\n\tcommand = {}\n[diff]\n\texternal = {}\n\
             [pager]\n\tlog = {}\n\tstatus = {}\n[log]\n\tshowSignature = true\n\
             [commit]\n\tgpgSign = true\n[gpg]\n\tprogram = {}\n[credential]\n\thelper = {}\n\
             [include]\n\tpath = {}\n[gc]\n\tauto = 1\n[maintenance]\n\tauto = true\n",
            script("fsmonitor"),
            script("pager"),
            script("editor"),
            script("askpass"),
            script("filter-clean"),
            script("filter-smudge"),
            script("textconv"),
            script("diff-command"),
            script("diff-external"),
            script("pager-log"),
            script("pager-status"),
            script("gpg"),
            script("credential"),
            included.display()
        ));
        std::fs::write(dir.join(".git/config"), cfg).unwrap();
        std::fs::write(dir.join("a.md"), "# A, changed\n").unwrap();
        std::fs::write(dir.join("b.md"), "# B\n").unwrap();
    }

    #[cfg(unix)]
    fn marks(d: &Path) -> Vec<String> {
        let mut m: Vec<String> = std::fs::read_dir(d).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
        m.sort();
        m
    }

    #[cfg(unix)]
    #[test]
    fn a_repositorys_own_settings_run_nothing() {
        let (_env, root) = paths::testenv::folders("jshost-git-trap");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        // The trap works: git as the user runs it, in a repository set up the same way, runs those programs.
        let control = root.join("control");
        trap(&control, &root.join("control-marks"));
        user_git(&control, &["status", "--porcelain"]);
        let _ = Command::new(program().unwrap()).arg("-C").arg(&control).args(["commit", "-q", "-m", "x", "--", "a.md"]).output();
        let ran = marks(&root.join("control-marks"));
        for m in ["fsmonitor", "filter-clean", "pre-commit"] {
            assert!(ran.contains(&m.to_string()), "the control ran {m}: {ran:?}");
        }
        // Echo's git, on a vault set up the same way: none of them.
        let vault = root.join("vault");
        let marked = root.join("vault-marks");
        trap(&vault, &marked);
        install(&root, "safe", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        assert_eq!(run("safe", "info", json!({})).unwrap()["repo"], true);
        let mut st = rel(&run("safe", "status", json!({})).unwrap(), "path");
        st.sort();
        assert_eq!(st, ["a.md", "b.md", "hooks-here/commit-msg", "hooks-here/post-commit", "hooks-here/post-index-change", "hooks-here/pre-commit", "hooks-here/prepare-commit-msg", "hooks-here/reference-transaction"]);
        assert_eq!(rel(&run("safe", "log", json!({"path": "a.md"})).unwrap(), "subject"), ["start"]);
        assert_eq!(run("safe", "show", json!({"rev": "HEAD", "path": "a.md"})).unwrap()["text"], "# A\n");
        run("safe", "commit", json!({"paths": ["a.md", "b.md"], "message": "Echo: two notes"})).unwrap();
        let plain = |args: &[&str]| user_git(&vault, &[&["-c", "core.fsmonitor=false", "-c", "core.hooksPath=/nonexistent", "-c", "log.showSignature=false"], args].concat());
        assert_eq!(plain(&["cat-file", "blob", "HEAD:b.md"]), "# B\n", "the file as it is on the disk: no filter");
        assert_eq!(plain(&["log", "-1", "--format=%s"]).trim(), "Echo: two notes");
        assert_eq!(marks(&marked), Vec::<String>::new(), "nothing the repository named ran");
        // Init too: no template, nothing of the user's or system's run.
        let fresh = root.join("fresh");
        install(&root, "starter", json!({"folders": {"what": "vaults", "write": true}}), &fresh);
        assert_eq!(run("starter", "init", json!({"branch": "notes"})).unwrap()["branch"], "notes");
        assert_eq!(marks(&marked), Vec::<String>::new());
        for id in ["safe", "starter"] {
            plugins::remove(id, true).unwrap();
        }
        sync();
    }

    #[test]
    fn a_repository_whose_history_is_elsewhere_is_refused() {
        let (_env, root) = paths::testenv::folders("jshost-git-elsewhere");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        // Another repository of the user's, with a file the vault must never show.
        let other = root.join("private");
        std::fs::create_dir_all(&other).unwrap();
        user_git(&other, &["init", "-q", "-b", "main"]);
        std::fs::write(other.join("secret.md"), "budget of someone else\n").unwrap();
        user_git(&other, &["add", "-A"]);
        user_git(&other, &["commit", "-q", "-m", "private"]);
        let vault = root.join("vault");
        install(&root, "far", json!({"folders": {"what": "vaults"}}), &vault);
        // A `gitdir:` file pointing at it.
        std::fs::write(vault.join(".git"), format!("gitdir: {}\n", other.join(".git").display())).unwrap();
        for (cmd, a) in [("info", json!({})), ("log", json!({})), ("show", json!({"rev": "HEAD", "path": "secret.md"})), ("status", json!({}))] {
            assert!(run("far", cmd, a).unwrap_err().contains("outside its folder"), "{cmd}");
        }
        // `core.worktree` pointing elsewhere.
        std::fs::remove_file(vault.join(".git")).unwrap();
        user_git(&vault, &["init", "-q", "-b", "main"]);
        user_git(&vault, &["config", "core.worktree", &other.display().to_string()]);
        assert!(run("far", "show", json!({"rev": "HEAD", "path": "secret.md"})).is_err());
        // Objects borrowed from it.
        user_git(&vault, &["config", "--unset", "core.worktree"]);
        std::fs::write(vault.join(".git/objects/info/alternates"), format!("{}\n", other.join(".git/objects").display())).unwrap();
        let head = user_git(&other, &["rev-parse", "HEAD"]);
        std::fs::write(vault.join(".git/refs/heads/main"), head).unwrap();
        assert!(run("far", "show", json!({"rev": "HEAD", "path": "secret.md"})).unwrap_err().contains("alternates"));
        plugins::remove("far", true).unwrap();
        sync();
    }

    #[test]
    fn git_is_reached_only_as_the_sheet_said_and_not_written_while_claude_prepares_and_asks() {
        let (_env, root) = paths::testenv::folders("jshost-git-uses");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let (rv, wv, nv) = (root.join("rvault"), root.join("wvault"), root.join("nvault"));
        install(&root, "reader", json!({"folders": {"what": "vaults"}}), &rv);
        install(&root, "writer", json!({"folders": {"what": "vaults", "write": true}}), &wv);
        install(&root, "none", json!({}), &nv);
        for d in [&rv, &wv] {
            user_git(d, &["init", "-q", "-b", "main"]);
            std::fs::write(d.join("A.md"), "acme\n").unwrap();
        }
        // Read-only: history yes, init and commit no.
        assert_eq!(run("reader", "info", json!({})).unwrap()["repo"], true);
        assert!(run("reader", "status", json!({})).is_ok());
        for (cmd, a) in [("init", json!({})), ("commit", json!({"paths": ["A.md"], "message": "x"}))] {
            assert!(run("reader", cmd, a).unwrap_err().contains("folders.write"), "{cmd}");
        }
        // Without uses.folders: nothing.
        for cmd in ["info", "status", "log"] {
            assert!(run("none", cmd, json!({})).unwrap_err().contains("“folders”"), "{cmd}");
        }
        // While Claude prepares and asks: reads only.
        std::env::set_var(crate::guard::RUN_MODE, "routine");
        let asked = run("writer", "commit", json!({"paths": ["A.md"], "message": "x"}));
        let read = run("writer", "status", json!({}));
        std::env::remove_var(crate::guard::RUN_MODE);
        assert!(asked.unwrap_err().contains("prepares and asks"));
        assert_eq!(rel(&read.unwrap(), "path"), ["A.md"]);
        // No git on this computer: said so.
        std::env::set_var("ECHOCTL_TEST_NO_GIT", "1");
        let none = run("writer", "info", json!({}));
        std::env::remove_var("ECHOCTL_TEST_NO_GIT");
        assert_eq!(none.unwrap_err(), "git isn't installed on this computer");
        for id in ["reader", "writer", "none"] {
            plugins::remove(id, true).unwrap();
        }
        sync();
    }

    #[test]
    fn only_the_listed_verbs_run_with_none_of_echos_or_claudes_variables() {
        let (_env, root) = paths::testenv::folders("jshost-git-env");
        let Ok(git) = program() else { return };
        for verb in ["push", "fetch", "clone", "remote", "-c", "--exec-path=/tmp", "submodule", "checkout", "reset", "gc"] {
            assert!(spawn(&git, &root, &[verb], &[]).err().unwrap().to_string().contains("doesn't run"), "{verb}");
        }
        // What git's environment holds: a stand-in that prints it.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let fake = root.join("git");
            std::fs::write(&fake, "#!/bin/sh\nenv\n").unwrap();
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
            let set = [("ECHOCTL_SECRET_TEST", "s"), ("CLAUDE_CODE_OAUTH_TOKEN", "t"), ("ANTHROPIC_API_KEY", "k"), ("GIT_DIR", "/elsewhere"), ("GIT_CONFIG_PARAMETERS", "'core.fsmonitor=x'")];
            for (k, v) in set {
                std::env::set_var(k, v);
            }
            let env = spawn(&fake, &root, &["status"], &[("core.fsmonitor".into(), "false".into())]).unwrap().text();
            for (k, _) in set {
                std::env::remove_var(k);
            }
            for gone in ["ECHOCTL", "CLAUDE", "ANTHROPIC", "GIT_DIR", "GIT_CONFIG_PARAMETERS"] {
                assert!(!env.lines().any(|l| l.starts_with(gone)), "{gone} in {env}");
            }
            for want in ["GIT_LITERAL_PATHSPECS=1", "GIT_TERMINAL_PROMPT=0", "GIT_CONFIG_COUNT=1", "GIT_CONFIG_KEY_0=core.fsmonitor", "GIT_CONFIG_VALUE_0=false"] {
                assert!(env.lines().any(|l| l == want), "{want}");
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_slow_git_is_stopped_and_its_output_capped() {
        use std::os::unix::fs::PermissionsExt;
        let (_env, root) = paths::testenv::folders("jshost-git-slow");
        let fake = root.join("git");
        let child = root.join("child-alive");
        // It starts a child of its own and waits: both stopped.
        std::fs::write(&fake, format!("#!/bin/sh\n(sleep 1; touch \"{}\") &\nsleep 30\n", child.display())).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let t = Instant::now();
        let e = spawn_within(&fake, &root, &["status"], &[], Duration::from_millis(300)).err().unwrap();
        assert!(e.to_string().contains("took longer"), "{e}");
        assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
        std::thread::sleep(Duration::from_millis(1500));
        assert!(!child.exists(), "its whole group was stopped");
        // More than the cap: read to its end, kept up to the cap.
        std::fs::write(&fake, format!("#!/bin/sh\nhead -c {} /dev/zero\nhead -c 200000 /dev/zero >&2\n", OUT_MAX + 1000)).unwrap();
        let o = spawn_within(&fake, &root, &["log"], &[], Duration::from_secs(20)).unwrap();
        assert_eq!((o.stdout.len(), o.over, o.stderr.len()), (OUT_MAX, true, ERR_MAX));
    }
}
