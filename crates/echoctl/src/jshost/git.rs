//! `ctx.git.*`: a granted folder's git history, changes and commits, for a plugin that may not start programs itself
//! (echo-plugins R17). Git runs here only with the verbs of `VERBS`, never through a shell, each path literal after
//! `--` (`GIT_LITERAL_PATHSPECS`), within `TIMEOUT` and the output caps, in an environment of a few named variables
//! (none of Echo's secrets, no `CLAUDE*` / `ANTHROPIC*`, no `GIT_*` of the user's but `GIT_CONFIG_GLOBAL`), and with every setting that names a
//! program switched off on its command line (`overrides`): a vault's own `.git/config` runs nothing. No network verb.
//! Only a repository whose git dir no plugin can write (`top_of`), named to git outright (`GIT_DIR`, `GIT_WORK_TREE`):
//! its settings stay as `overrides` read them while git runs.

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

/// Where git is looked for first: where the system or its package manager installs it (Windows: `git.exe` of Git for
/// Windows, never a `.cmd`, which runs through cmd.exe). macOS: `/usr/bin/git` only while developer tools are
/// installed — without them it opens Apple's install dialog instead of running.
fn system_candidates() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"]
            .iter()
            .filter_map(std::env::var_os)
            .flat_map(|d| ["cmd", "bin"].map(|b| PathBuf::from(&d).join("Git").join(b).join("git.exe")))
            .collect()
    }
    #[cfg(target_os = "macos")]
    {
        let mut list: Vec<PathBuf> = ["/opt/homebrew/bin/git", "/usr/local/bin/git"].map(PathBuf::from).into();
        // Asked once a process.
        static TOOLS: OnceLock<bool> = OnceLock::new();
        let tools = *TOOLS.get_or_init(|| {
            let o = Command::new("/usr/bin/xcode-select").arg("-p").stdin(Stdio::null()).stderr(Stdio::null()).output();
            o.ok().filter(|o| o.status.success()).is_some_and(|o| Path::new(String::from_utf8_lossy(&o.stdout).trim()).join("usr/bin/git").is_file())
        });
        if tools {
            list.push("/usr/bin/git".into());
        }
        list
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        ["/usr/bin/git", "/usr/local/bin/git"].map(PathBuf::from).into()
    }
}

/// Then `git` (Windows: `git.exe`) in the folders of a `PATH`, in its order, each by its real path: never a relative
/// folder, one inside a folder given to any plugin or inside Echo's data or own folder (a plugin could put a git
/// there), nor macOS's `/usr/bin/git` (in `system_candidates` when it runs); only a file that can be started.
fn path_candidates(path: &std::ffi::OsStr) -> Vec<PathBuf> {
    let granted = granted();
    let theirs = |p: &Path| !plugins_reach(p, &granted);
    std::env::split_paths(path)
        .filter(|d| d.is_absolute() && theirs(d))
        .filter_map(|d| dunce::canonicalize(d.join(if cfg!(windows) { "git.exe" } else { "git" })).ok())
        .filter(|f| theirs(f) && !(cfg!(target_os = "macos") && f == Path::new("/usr/bin/git")) && runnable(f))
        .collect()
}

/// The folders given to any plugin, each by its real path.
fn granted() -> Vec<PathBuf> {
    crate::grants::all_folders().into_iter().map(|g| dunce::canonicalize(&g).unwrap_or(g)).collect()
}

/// Whether a plugin may write at `p`: in a folder given to any plugin, or in Echo's data or own folder (where plugins
/// keep their stores).
fn plugins_reach(p: &Path, granted: &[PathBuf]) -> bool {
    // macOS and Windows file systems ignore case by default: so does this.
    let fold = |p: &Path| if cfg!(any(target_os = "macos", windows)) { PathBuf::from(p.to_string_lossy().to_lowercase()) } else { p.to_path_buf() };
    crate::ai::wrapper::outside_echo(p).is_err() || granted.iter().any(|g| fold(p).starts_with(fold(g)))
}

/// `p` as the file system finds it, there or not: each folder that is there by its real path (links followed, `..`
/// taken from there), the rest as written.
fn physical(p: &Path) -> PathBuf {
    let mut at = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                at.pop();
            }
            std::path::Component::Normal(n) => {
                at.push(n);
                if let Ok(real) = dunce::canonicalize(&at) {
                    at = real;
                }
            }
            other => at.push(other),
        }
    }
    at
}

/// A settings file git would read at `p` that a plugin could change while git runs, by its real path: inside a folder
/// given to a plugin or Echo's own, or a file of the work tree `top` outside a hidden folder.
fn changeable(p: &Path, top: &Path, granted: &[PathBuf]) -> Option<PathBuf> {
    let p = physical(p);
    let in_tree = p.strip_prefix(top).is_ok_and(|rel| !rel.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')));
    (plugins_reach(&p, granted) || in_tree).then_some(p)
}

/// The user's home, as git expands `~`.
fn git_home() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// The file an `include.path` / `includeIf.*.path` names, resolved as git does: `~/` from the home, `%(prefix)/` from
/// the folder above git's own, a relative path from the folder of the file that includes it. None: one Echo can't place.
fn include_target(value: &str, origin: &str, git: &Path) -> Option<PathBuf> {
    if value == "~" || value.starts_with("~/") {
        return Some(git_home()?.join(value.trim_start_matches('~').trim_start_matches('/')));
    }
    if value.starts_with('~') {
        return None;
    }
    if let Some(rest) = value.strip_prefix("%(prefix)/") {
        return Some(dunce::canonicalize(git).ok()?.parent()?.parent()?.join(rest));
    }
    // A drive with no root (`C:tools\git.cfg`): Windows resolves it against that drive's current folder, git's not Echo's.
    let b = value.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && !matches!(b.get(2), Some(b'/' | b'\\')) {
        return None;
    }
    if Path::new(value).is_absolute() {
        return Some(PathBuf::from(value));
    }
    Some(Path::new(origin.strip_prefix("file:")?).parent()?.join(value))
}

/// A file that can be started (Unix: executable).
fn runnable(f: &Path) -> bool {
    let Ok(m) = std::fs::metadata(f) else { return false };
    #[cfg(unix)]
    let can = {
        use std::os::unix::fs::PermissionsExt;
        m.permissions().mode() & 0o111 != 0
    };
    #[cfg(not(unix))]
    let can = true;
    m.is_file() && can
}

/// The git program: the system's (`system_candidates`), else the first on the user's `PATH` — a terminal's, the login
/// shell's (`path_candidates`).
fn program() -> Result<PathBuf> {
    #[cfg(test)]
    if std::env::var_os("ECHOCTL_TEST_NO_GIT").is_some() {
        bail!("git isn't installed on this computer");
    }
    system_candidates()
        .into_iter()
        .find(|p| p.is_file())
        .or_else(|| path_candidates(&crate::ai::wrapper::login_path().unwrap_or_default()).into_iter().next())
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

/// The variables git gets from the user's environment; nothing else of it (no other `GIT_*`, none of Echo's or
/// Claude's). `GIT_CONFIG_GLOBAL` is checked as the user's settings are (`user_files`).
const ENV_KEPT: &[&str] = &["PATH", "HOME", "USERPROFILE", "HOMEDRIVE", "HOMEPATH", "SystemRoot", "SYSTEMROOT", "TEMP", "TMP", "TMPDIR", "XDG_CONFIG_HOME", "GIT_CONFIG_GLOBAL"];

/// The user's settings files git may read: `$XDG_CONFIG_HOME/git/config`, `~/.gitconfig` and `~/.config/git/config`,
/// and the one `GIT_CONFIG_GLOBAL` names when Echo's environment sets it (git 2.32 and later read that instead of
/// them; 2.31 ignores it). A `GIT_CONFIG_GLOBAL` that isn't a full path is refused: git would read it from the folder
/// it runs in.
fn user_files() -> Result<Vec<PathBuf>> {
    let global = std::env::var_os("GIT_CONFIG_GLOBAL").filter(|g| !g.is_empty()).map(PathBuf::from);
    if let Some(g) = global.as_ref().filter(|g| !g.is_absolute()) {
        bail!("GIT_CONFIG_GLOBAL names {}, not a full path: Echo doesn't run git with it", g.display());
    }
    let xdg = std::env::var_os("XDG_CONFIG_HOME").filter(|x| !x.is_empty()).map(|x| PathBuf::from(x).join("git/config"));
    Ok(global.into_iter().chain(xdg).chain(git_home().iter().flat_map(|h| [h.join(".gitconfig"), h.join(".config/git/config")])).collect())
}

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
fn overrides(git: &Path, dir: &Path, top: &Path, found: bool, hooks: &Path) -> Result<Vec<(String, String)>> {
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
    // Read with the fixed ones in force (a config read runs nothing) — whole, or the repository isn't used: a setting
    // not read (past the cap, a listing that failed, a name that isn't text) couldn't be cleared.
    // No settings file a plugin could change between this reading and git's own: the user's, the repository's, and
    // every file they include.
    let granted = granted();
    let refuse = |p: &Path| anyhow!("this repository's settings read {}, a file a plugin could change: Echo doesn't use it", p.display());
    if let Some(p) = user_files()?.iter().find_map(|f| changeable(f, top, &granted)) {
        return Err(refuse(&p));
    }
    // The repository's own, in its real `.git` (`top_of`) unless a link leads elsewhere.
    for f in ["config", "config.worktree"].map(|f| top.join(".git").join(f)).iter().filter(|_| found) {
        if let Some(p) = changeable(f, top, &granted).filter(|p| p != f) {
            return Err(refuse(&p));
        }
    }
    let listed = spawn_within(git, dir, Some(top), &["config", "--list", "--show-origin", "-z"], &fixed, TIMEOUT)?;
    if !listed.ok || listed.over {
        bail!("this repository's settings couldn't be read whole ({}): Echo doesn't use it", if listed.over { format!("over {} MB", OUT_MAX >> 20) } else { listed.why() });
    }
    // `<origin>\0<key>\n<value>\0`: a pair a setting.
    let mut parts: Vec<&[u8]> = listed.stdout.split(|b| *b == 0).collect();
    if parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    if !parts.len().is_multiple_of(2) {
        bail!("this repository's settings couldn't be read whole (a listing out of step): Echo doesn't use it");
    }
    let mut out = vec![];
    for pair in parts.chunks(2) {
        let (origin, entry) = (pair[0], pair[1]);
        let (key, value) = match entry.iter().position(|b| *b == b'\n') {
            Some(n) => (&entry[..n], &entry[n + 1..]),
            None => (entry, &[][..]),
        };
        let key = std::str::from_utf8(key).map_err(|_| anyhow!("a setting of this repository is named in bytes that aren't text (UTF-8): Echo doesn't use it"))?;
        let (section, last) = (key.split('.').next().unwrap_or("").to_ascii_lowercase(), key.rsplit('.').next().unwrap_or("").to_ascii_lowercase());
        // A file included, there or not: git skips a missing one, and would read it once a plugin wrote it.
        if (section == "include" || section == "includeif") && last == "path" && key.contains('.') {
            let (Ok(value), Ok(origin)) = (std::str::from_utf8(value), std::str::from_utf8(origin)) else {
                bail!("this repository's settings include a file named in bytes that aren't text (UTF-8): Echo doesn't use it");
            };
            let target = include_target(value, origin, git).ok_or_else(|| anyhow!("this repository's settings include “{value}”, a file Echo can't place: Echo doesn't use it"))?;
            if let Some(p) = changeable(&target, top, &granted) {
                return Err(refuse(&p));
            }
        }
        // Its files kept elsewhere than beside its .git: never used (Echo names the work tree itself).
        if section == "core" && last == "worktree" && key.matches('.').count() == 1 {
            bail!("this repository sets core.worktree (its files kept elsewhere than beside its .git): Echo doesn't use it");
        }
        // A partial clone fetches what it lacks from its promisor remote, by any read: never used.
        if (section == "extensions" && last == "partialclone") || (section == "remote" && last == "promisor") {
            bail!("this repository is a partial clone, which fetches what it lacks from elsewhere: Echo doesn't use it");
        }
        if section == "pager" || (section == "filter" && last == "required") {
            out.push((key.to_string(), "false".to_string()));
        } else if section == "protocol" && last == "allow" {
            out.push((key.to_string(), "never".to_string()));
        } else if section == "hook" && key.matches('.').count() >= 2 {
            // A hook set in the settings (`hook.<name>.command` run on `hook.<name>.event`): no event, off.
            let name = &key[..key.rfind('.').unwrap_or(key.len())];
            out.push((format!("{name}.event"), String::new()));
            out.push((format!("{name}.enabled"), "false".to_string()));
        }
        if PROGRAM_KEYS.contains(&last.as_str()) && key.contains('.') {
            out.push((key.to_string(), String::new()));
        }
    }
    out.extend(fixed);
    Ok(out)
}

/// `git <args>` in `dir` with only `config` set on its command line: no shell, no stdin, its own process group, killed
/// at `TIMEOUT`, both streams read as they come (a full pipe would stall it) and capped.
fn spawn(git: &Path, dir: &Path, args: &[&str], config: &[(String, String)]) -> Result<Out> {
    spawn_within(git, dir, None, args, config, TIMEOUT)
}

/// …in the repository whose top folder is `top` when given: its git dir `<top>/.git` named outright, never looked for.
fn spawn_within(git: &Path, dir: &Path, top: Option<&Path>, args: &[&str], config: &[(String, String)], timeout: Duration) -> Result<Out> {
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
        // Nothing fetched, by any protocol: a missing object is missing, never asked of a remote.
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_ALLOW_PROTOCOL", "")
        .env("GIT_CONFIG_COUNT", config.len().to_string());
    if let Some(top) = top {
        c.env("GIT_DIR", top.join(".git")).env("GIT_WORK_TREE", top);
        if let Some(up) = top.parent() {
            c.env("GIT_CEILING_DIRECTORIES", up);
        }
    }
    for (n, (k, v)) in config.iter().enumerate() {
        c.env(format!("GIT_CONFIG_KEY_{n}"), k).env(format!("GIT_CONFIG_VALUE_{n}"), v);
    }
    c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    // With all it starts, stopped together (Unix: its own session; Windows: a job that ends them all).
    let (mut child, tree) = crate::plugins::signin::Tree::spawn(c).ok_or_else(|| anyhow!("git couldn't start"))?;
    let (tx, rx) = std::sync::mpsc::channel();
    for (n, (r, cap)) in [(child.stdout.take().map(|r| Box::new(r) as Box<dyn Read + Send>), OUT_MAX), (child.stderr.take().map(|r| Box::new(r) as Box<dyn Read + Send>), ERR_MAX)].into_iter().enumerate() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let (mut kept, mut over, mut buf) = (Vec::new(), false, [0u8; 65536]);
            if let Some(mut r) = r {
                while let Ok(m) = r.read(&mut buf) {
                    if m == 0 {
                        break;
                    }
                    let room = cap.saturating_sub(kept.len());
                    kept.extend_from_slice(&buf[..m.min(room)]);
                    over |= m > room;
                }
            }
            let _ = tx.send((n, kept, over));
        });
    }
    let t = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if t.elapsed() < timeout => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                tree.stop(&mut child);
                bail!("git took longer than {} s and was stopped", timeout.as_secs());
            }
        }
    };
    // Its output ends with it — unless something it started still holds it: that is stopped, and the call fails.
    let mut got: [Option<(Vec<u8>, bool)>; 2] = [None, None];
    let mut stopped = false;
    while got.iter().any(Option::is_none) {
        match rx.recv_timeout(Duration::from_secs(if stopped { 1 } else { 2 })) {
            Ok((n, kept, over)) => got[n] = Some((kept, over)),
            Err(_) if !stopped => {
                tree.stop_rest();
                stopped = true;
            }
            Err(_) => bail!("git's output didn't end: something it started still holds it"),
        }
    }
    let [Some((stdout, over)), Some((stderr, _))] = got else { unreachable!("both read") };
    Ok(Out { ok: status.success(), stdout, over, stderr })
}

/// Git, ready to run in one granted folder: the program, the folder (canonical), the top folder of its repository
/// (`found`; the folder itself when there is none, where `init` makes one) and the overrides for it.
struct Git {
    git: PathBuf,
    root: PathBuf,
    top: PathBuf,
    found: bool,
    config: Vec<(String, String)>,
}

const ELSEWHERE: &str = "this repository keeps its history outside its folder (a linked worktree, a submodule or a link in .git): Echo doesn't use it";

/// The top folder of the repository `root` is in, or None — looked for as git does, upwards from it, but only one
/// shape taken: a real folder `.git` (not a link, not a `gitdir:` file), the user's own, in the root or a folder above
/// it. Its git dir is then nothing a plugin writes: `grants::writable` refuses hidden names, above the root it is
/// outside the grant, and no plugin is given a folder in it. A bare repository (a folder holding `HEAD`, `objects`
/// and `refs` itself — a plugin can lay those out) is refused, as is being inside a `.git`.
fn top_of(root: &Path) -> Result<Option<PathBuf>> {
    for dir in root.ancestors() {
        let dot = dir.join(".git");
        match std::fs::symlink_metadata(&dot) {
            Ok(m) if m.is_dir() => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    // SAFETY: geteuid only reads this process's user id.
                    if m.uid() != unsafe { libc::geteuid() } {
                        bail!("this repository belongs to another user of this computer: Echo doesn't use it");
                    }
                }
                if crate::grants::all_folders().iter().any(|g| g.starts_with(&dot)) {
                    bail!("a folder inside this repository's .git is given to a plugin: Echo doesn't use it");
                }
                return Ok(Some(dir.to_path_buf()));
            }
            Ok(_) => bail!("{ELSEWHERE}"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => bail!("{}: {e}", dot.display()),
        }
        if dir.join("HEAD").is_file() && dir.join("objects").is_dir() && dir.join("refs").is_dir() {
            bail!("this folder is in a bare repository (its HEAD, objects and refs not in a .git): Echo doesn't use it");
        }
    }
    Ok(None)
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
        let found = top_of(&root)?;
        let top = found.clone().unwrap_or_else(|| root.clone());
        let config = overrides(&git, &root, &top, found.is_some(), &no_hooks()?)?;
        Ok(Git { git, root, top, found: found.is_some(), config })
    }

    fn run(&self, args: &[&str]) -> Result<Out> {
        self.run_with(args, &self.config)
    }

    fn run_with(&self, args: &[&str], config: &[(String, String)]) -> Result<Out> {
        spawn_within(&self.git, &self.root, Some(&self.top), args, config, TIMEOUT)
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

    /// The repository the root is in (`top_of`), or None. Only one whose files are all in it: git agrees its top is
    /// that folder and its git dir that `.git` (the common one too), no objects borrowed from elsewhere — a
    /// `commondir`, links in `.git` or alternates would let a vault's settings show another repository's files.
    fn repo(&self) -> Result<Option<Repo>> {
        if !self.found {
            return Ok(None);
        }
        let o = self.ok(&["rev-parse", "--show-prefix", "--path-format=absolute", "--show-toplevel", "--git-dir", "--git-common-dir"])?;
        let text = o.text();
        let lines: Vec<&str> = text.lines().collect();
        let [prefix, top, git_dir, common] = lines[..] else { bail!("this folder isn't in a repository's files (a bare repository or inside .git)") };
        let real = |p: &str| dunce::canonicalize(p).map_err(|e| anyhow!("{p}: {e}"));
        let (top, git_dir, common) = (real(top)?, real(git_dir)?, real(common)?);
        if top != self.top || git_dir != self.top.join(".git") || common != git_dir {
            bail!("{ELSEWHERE}");
        }
        // Its history's parts too, each where it really is: a link to another repository's objects or refs (or a
        // folder of objects, a pack linked in) would show that one's files.
        let mut parts: Vec<PathBuf> = ["objects", "refs", "packed-refs", "info", "logs"].iter().map(|p| common.join(p)).chain(["HEAD", "index"].map(|p| git_dir.join(p))).collect();
        for d in ["objects", "objects/pack", "objects/info"] {
            parts.extend(std::fs::read_dir(common.join(d)).into_iter().flatten().flatten().map(|e| e.path()));
        }
        let outside = |p: &PathBuf| p.symlink_metadata().is_ok() && !dunce::canonicalize(p).is_ok_and(|r| r.starts_with(&top));
        if !self.root.starts_with(&top) || !git_dir.starts_with(&top) || !common.starts_with(&top) || parts.iter().any(outside) {
            bail!("this repository keeps its history outside its folder (a linked worktree, a submodule, core.worktree or a link in .git): Echo doesn't use it");
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

/// …and one it may change (`folders.write`).
fn write_root(id: &str, root: &str) -> Result<PathBuf> {
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
    Ok(json!({"repo": true, "branch": branch, "head": g.head()?, "detached": branch.is_none(), "busy": busy(&repo), "subfolder": !repo.prefix.is_empty()}))
}

/// What git is in the middle of in this repository, if anything: a merge, rebase, cherry-pick, revert or bisect.
fn busy(repo: &Repo) -> Option<&'static str> {
    [
        ("MERGE_HEAD", "merge"),
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
        ("REVERT_HEAD", "revert"),
        ("BISECT_LOG", "bisect"),
    ]
    .iter()
    .find(|(f, _)| repo.git_dir.join(f).exists())
    .map(|(_, w)| *w)
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
    // A commit's id, never a tree's or a file's.
    let what = g.run(&["cat-file", "-t", rev])?;
    if !what.ok {
        return Ok(Value::Null);
    }
    if what.text() != "commit" {
        bail!("“{rev}” isn't a commit");
    }
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
    let text = super::folders::text_of(blob.stdout[..blob.stdout.len().min(max)].to_vec(), cut, o["strict"] == true, &path)?;
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
        // A skill committed runs nothing (one written another way than ctx.fs.write may).
        if crate::grants::in_skills(root, rel, &f) && f.is_file() {
            let md = std::fs::read(&f)?;
            crate::plugins::claude_skills::skill_md_ok(&String::from_utf8_lossy(&md)).map_err(|e| anyhow!("“{rel}”: {e}"))?;
        }
        Ok(f)
    })?;
    let g = Git::new(real_root)?;
    let repo = g.need_repo()?;
    if let Some(what) = busy(&repo) {
        bail!("a {what} is under way in this repository: Echo commits once it's done");
    }
    // Files git doesn't know yet go into its index first (`--only` takes only known ones); taken out again on a failure.
    let mut known = vec!["ls-files", "-z", "--"];
    known.extend(paths.iter().map(String::as_str));
    let known = g.ok(&known)?;
    let known: Vec<String> = known.stdout.split(|b| *b == 0).filter(|e| !e.is_empty()).map(|e| String::from_utf8_lossy(e).into_owned()).collect();
    let new: Vec<&str> = paths.iter().map(String::as_str).filter(|p| !known.iter().any(|k| k == p)).collect();
    if let Some(gone) = new.iter().find(|p| !g.root.join(p).exists()) {
        bail!("“{gone}” isn't there and git doesn't know it");
    }
    let mut changed = vec!["status", "--porcelain=v1", "-z", "--untracked-files=all", "--ignored=matching", "--ignore-submodules=all", "--"];
    changed.extend(paths.iter().map(String::as_str));
    let changed = g.ok(&changed)?.stdout;
    // `!! <path>`: one the repository ignores, which git won't take.
    if let Some(e) = changed.split(|b| *b == 0).find(|e| e.starts_with(b"!! ")) {
        let path = String::from_utf8_lossy(&e[3..]);
        bail!("“{}” is ignored by this repository (.gitignore): Echo doesn't commit it", path.strip_prefix(repo.prefix.as_str()).unwrap_or(&path));
    }
    if changed.is_empty() {
        return Ok(json!({"rev": null}));
    }
    // What was added is taken out of the index again when anything after fails, the add itself too.
    let back = || {
        if !new.is_empty() {
            let mut back = vec!["rm", "--cached", "-q", "--ignore-unmatch", "--"];
            back.extend(new.iter().copied());
            let _ = g.run(&back);
        }
    };
    if !new.is_empty() {
        let mut add = vec!["add", "--"];
        add.extend(new.iter().copied());
        if let Err(e) = g.ok(&add) {
            back();
            bail!("git didn't add them: {e:#}");
        }
    }
    let message = format!("--message={message}");
    let mut args = vec!["commit", "--only", "-q", "--no-verify", "--no-gpg-sign", "--cleanup=whitespace", message.as_str(), "--"];
    args.extend(paths.iter().map(String::as_str));
    let mut config = g.config.clone();
    config.extend(identity(&g, id));
    match g.run_with(&args, &config) {
        Ok(o) if o.ok => Ok(json!({"rev": g.head()?})),
        other => {
            back();
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

    /// A setup's commit leaves no `git maintenance run --auto --detach` behind it: started now and slowed by load, it
    /// would read the trap's settings written right after (gc.auto=1) and its gc read the index — running the
    /// repository's fsmonitor as if Echo had.
    const NO_MAINTENANCE: &[&str] = &["-c", "maintenance.auto=false"];

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
        // Refused as ctx.fs.write refuses them: hidden, a link, outside, nothing there; a skill put there another way
        // (not by ctx.fs.write) that runs a command.
        std::fs::create_dir_all(vault.join(".claude/skills/acme-x")).unwrap();
        std::fs::write(vault.join(".claude/skills/acme-x/SKILL.md"), "---\nname: acme-x\nhooks:\n  Stop: id\n---\n").unwrap();
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
        for (p, why) in [
            (".obsidian/plugins/x/main.js", "hidden"),
            (".claude/settings.json", "hidden"),
            (".claude/skills/acme-x/SKILL.md", "a skill can't run commands"),
            ("../README.md", "isn't a path inside"),
            ("Missing/none.md", "isn't there and git doesn't know it"),
        ] {
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
        std::fs::create_dir_all(dir).unwrap();
        std::fs::create_dir_all(marks).unwrap();
        let tag = dir.file_name().unwrap().to_string_lossy().into_owned();
        let script = |name: &str, reads: bool| marker(&marks.parent().unwrap().join(format!("{tag}-{name}.sh")), &marks.join(name), reads);
        user_git(dir, &["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("a.md"), "# A\n").unwrap();
        std::fs::write(dir.join("c.txt"), "C\n").unwrap();
        std::fs::write(dir.join("d.inc"), "D\n").unwrap();
        std::fs::write(dir.join(".gitattributes"), "*.md filter=evil diff=evil\n*.inc filter=inc\n").unwrap();
        user_git(dir, &["add", "-A"]);
        user_git(dir, &[&["-c", "core.hooksPath=/nonexistent"], NO_MAINTENANCE, &["commit", "-q", "-m", "start"]].concat());
        for hook in ["pre-commit", "prepare-commit-msg", "commit-msg", "post-commit", "reference-transaction", "post-index-change"] {
            let s = script(hook, true);
            for d in [".git/hooks", "hooks-here"] {
                std::fs::create_dir_all(dir.join(d)).unwrap();
                std::fs::copy(&s, dir.join(d).join(hook)).unwrap();
            }
        }
        // ssh reads nothing: git waits on it first.
        let included = marks.parent().unwrap().join(format!("{tag}-included.gitconfig"));
        std::fs::write(&included, format!("[core]\n\tsshCommand = {}\n[filter \"inc\"]\n\tclean = {}\n", script("ssh", false), script("included-filter", true))).unwrap();
        let mut cfg = std::fs::read_to_string(dir.join(".git/config")).unwrap();
        cfg.push_str(&format!(
            "[core]\n\thooksPath = hooks-here\n\tfsmonitor = {}\n\tpager = {}\n\teditor = {}\n\taskPass = {}\n\
             [filter \"evil\"]\n\tclean = {}\n\tsmudge = {}\n\trequired = true\n\
             [diff \"evil\"]\n\ttextconv = {}\n\tcommand = {}\n[diff]\n\texternal = {}\n\
             [pager]\n\tlog = {}\n\tstatus = {}\n[log]\n\tshowSignature = true\n\
             [commit]\n\tgpgSign = true\n[gpg]\n\tprogram = {}\n[credential]\n\thelper = {}\n\
             [hook \"evil\"]\n\tevent = pre-commit\n\tevent = post-commit\n\tevent = reference-transaction\n\tcommand = {}\n\
             [include]\n\tpath = {}\n[gc]\n\tauto = 1\n[maintenance]\n\tauto = true\n",
            script("fsmonitor", true),
            script("pager", true),
            script("editor", true),
            script("askpass", true),
            script("filter-clean", true),
            script("filter-smudge", true),
            script("textconv", true),
            script("diff-command", true),
            script("diff-external", true),
            script("pager-log", true),
            script("pager-status", true),
            script("gpg", true),
            script("credential", true),
            script("config-hook", true),
            included.display()
        ));
        std::fs::write(dir.join(".git/config"), cfg).unwrap();
        std::fs::write(dir.join("a.md"), "# A, changed\n").unwrap();
        std::fs::write(dir.join("b.md"), "# B\n").unwrap();
        std::fs::write(dir.join("c.txt"), "C, changed\n").unwrap();
        std::fs::write(dir.join("d.inc"), "D, changed\n").unwrap();
    }

    #[cfg(unix)]
    fn marks(d: &Path) -> Vec<String> {
        let mut m: Vec<String> = std::fs::read_dir(d).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
        m.sort();
        m
    }

    /// Plain git as the user runs it in a terminal, so that it pages; with `input` on its stdin. Nothing of the test's
    /// own environment picks the programs instead of the repository's settings, nor the system's keychain helper.
    #[cfg(unix)]
    fn plain_git_as_user(dir: &Path, args: &[&str], input: Option<&str>, terminal: bool) {
        use std::io::Write;
        use std::os::fd::FromRawFd;
        let mut c = Command::new(program().unwrap());
        c.arg("-C").arg(dir).args(args).env("GIT_CONFIG_NOSYSTEM", "1").stderr(Stdio::null());
        for k in ["GIT_PAGER", "PAGER", "GIT_EDITOR", "GIT_SEQUENCE_EDITOR", "GIT_SSH", "GIT_SSH_COMMAND", "GIT_ASKPASS", "SSH_ASKPASS", "GIT_EXTERNAL_DIFF"] {
            c.env_remove(k);
        }
        let mut terminal_end = None;
        if terminal {
            let (mut main, mut other) = (0, 0);
            // SAFETY: openpty fills two descriptors we then own.
            assert_eq!(unsafe { libc::openpty(&mut main, &mut other, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) }, 0);
            // SAFETY: both just opened, each owned once.
            let (main, other) = unsafe { (std::fs::File::from_raw_fd(main), std::fs::File::from_raw_fd(other)) };
            c.stdout(Stdio::from(other));
            terminal_end = Some(main);
        } else {
            c.stdout(Stdio::null());
        }
        c.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() });
        let mut child = c.spawn().unwrap();
        if let (Some(text), Some(mut w)) = (input, child.stdin.take()) {
            let _ = w.write_all(text.as_bytes());
        }
        let _ = child.wait();
        drop(terminal_end);
    }

    /// Whether the git on this computer is at least `major.minor` (`git version 2.54.0 (Apple Git-157)`).
    fn git_at_least(major: u32, minor: u32) -> bool {
        let out = std::process::Command::new("git").arg("--version").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        let v: Vec<u32> = out.split_whitespace().nth(2).unwrap_or("").split('.').take(2).filter_map(|n| n.parse().ok()).collect();
        v.len() == 2 && (v[0], v[1]) >= (major, minor)
    }

    /// The setups' own git starts nothing that outlives it (git's trace of the programs it starts, from the home's
    /// settings): what runs in a trapped repository afterwards is only what the test runs.
    #[cfg(unix)]
    #[test]
    fn a_setup_leaves_no_git_running_behind_it() {
        let (_env, root) = paths::testenv::folders("jshost-git-setup");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let trace = root.join("trace.json");
        let mut home = std::fs::read_to_string(root.join("user/.gitconfig")).unwrap();
        home.push_str(&format!("[trace2]\n\teventTarget = {}\n", trace.display()));
        std::fs::write(root.join("user/.gitconfig"), home).unwrap();
        trap(&root.join("trapped"), &root.join("marks"));
        one_commit(&root.join("plain"));
        let started: Vec<String> = std::fs::read_to_string(&trace).unwrap().lines().filter(|l| l.contains("\"event\":\"child_start\"")).map(String::from).collect();
        assert!(!started.iter().any(|l| l.contains("maintenance") || l.contains("\"gc\"")), "{started:?}");
    }

    #[test]
    fn a_repositorys_own_settings_run_nothing() {
        let (_env, root) = paths::testenv::folders("jshost-git-trap");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        // Each trap works: plain git, in a repository set up the same way, runs every one of those programs.
        let control = root.join("control");
        trap(&control, &root.join("control-marks"));
        let plain = |args: &[&str]| plain_git_as_user(&control, args, None, false);
        plain(&["status", "--porcelain"]);
        plain(&["add", "a.md", "d.inc"]);
        plain(&["diff", "HEAD", "--", "a.md"]);
        plain(&["diff", "--no-ext-diff", "HEAD", "--", "a.md"]);
        plain(&["diff", "HEAD", "--", "c.txt"]);
        plain(&["commit", "-q", "-m", "x"]);
        plain(&["-c", "commit.gpgSign=false", "commit", "-q", "--allow-empty"]);
        plain(&["ls-remote", "ssh://acme.example/notes"]);
        plain_git_as_user(&control, &["credential", "fill"], Some("protocol=https\nhost=acme.example\n\n"), false);
        for args in [&["log", "-1"][..], &["status"], &["--paginate", "rev-parse", "HEAD"]] {
            plain_git_as_user(&control, args, None, true);
        }
        let ran = marks(&root.join("control-marks"));
        for (m, how) in [
            ("fsmonitor", "status"),
            ("filter-clean", "add"),
            ("included-filter", "add of a file .gitattributes gives the included filter"),
            ("diff-command", "diff"),
            ("textconv", "diff --no-ext-diff"),
            ("diff-external", "diff of a file without a driver"),
            ("pre-commit", "commit (core.hooksPath)"),
            ("config-hook", "commit (hook.evil.command)"),
            ("gpg", "commit (commit.gpgSign)"),
            ("editor", "commit without a message"),
            ("ssh", "ls-remote over ssh"),
            ("credential", "credential fill"),
            ("askpass", "credential fill without a user"),
            ("pager-log", "log on a terminal"),
            ("pager-status", "status on a terminal"),
            ("pager", "--paginate"),
        ] {
            // Hooks set in the settings came with git 2.54: an older git runs none, Echo's clearing stays.
            if m == "config-hook" && !git_at_least(2, 54) {
                continue;
            }
            assert!(ran.contains(&m.to_string()), "the control ran {m} on {how}: {ran:?}");
        }
        // Echo's git, on a vault set up the same way: none of them.
        let vault = root.join("vault");
        let marked = root.join("vault-marks");
        trap(&vault, &marked);
        install(&root, "safe", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        assert_eq!(run("safe", "info", json!({})).unwrap()["repo"], true);
        let mut st = rel(&run("safe", "status", json!({})).unwrap(), "path");
        st.sort();
        let hooks = ["commit-msg", "post-commit", "post-index-change", "pre-commit", "prepare-commit-msg", "reference-transaction"].map(|h| format!("hooks-here/{h}"));
        assert_eq!(st, [&["a.md", "b.md", "c.txt", "d.inc"].map(String::from)[..], &hooks[..]].concat());
        assert_eq!(rel(&run("safe", "log", json!({"path": "a.md"})).unwrap(), "subject"), ["start"]);
        assert_eq!(run("safe", "show", json!({"rev": "HEAD", "path": "a.md"})).unwrap()["text"], "# A\n");
        run("safe", "commit", json!({"paths": ["a.md", "b.md", "c.txt", "d.inc"], "message": "Echo: four notes"})).unwrap();
        let plain = |args: &[&str]| user_git(&vault, &[&["-c", "core.fsmonitor=false", "-c", "core.hooksPath=/nonexistent", "-c", "log.showSignature=false"], args].concat());
        assert_eq!(plain(&["cat-file", "blob", "HEAD:b.md"]), "# B\n", "the file as it is on the disk: no filter");
        assert_eq!(plain(&["cat-file", "blob", "HEAD:d.inc"]), "D, changed\n", "no included filter");
        assert_eq!(plain(&["log", "-1", "--format=%s"]).trim(), "Echo: four notes");
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
    fn git_is_reached_only_as_the_sheet_said() {
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
            for want in ["GIT_LITERAL_PATHSPECS=1", "GIT_TERMINAL_PROMPT=0", "GIT_NO_LAZY_FETCH=1", "GIT_ALLOW_PROTOCOL=", "GIT_CONFIG_COUNT=1", "GIT_CONFIG_KEY_0=core.fsmonitor", "GIT_CONFIG_VALUE_0=false"] {
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
        let e = spawn_within(&fake, &root, None, &["status"], &[], Duration::from_millis(300)).err().unwrap();
        assert!(e.to_string().contains("took longer"), "{e}");
        assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
        std::thread::sleep(Duration::from_millis(1500));
        assert!(!child.exists(), "its whole group was stopped");
        // More than the cap: read to its end, kept up to the cap.
        std::fs::write(&fake, format!("#!/bin/sh\nhead -c {} /dev/zero\nhead -c 200000 /dev/zero >&2\n", OUT_MAX + 1000)).unwrap();
        let o = spawn_within(&fake, &root, None, &["log"], &[], Duration::from_secs(20)).unwrap();
        assert_eq!((o.stdout.len(), o.over, o.stderr.len()), (OUT_MAX, true, ERR_MAX));
        // It ends, but leaves something of its own (outside its group) holding its output: the call doesn't wait for it.
        let held = root.join("held-pid");
        std::fs::write(&fake, format!("#!/bin/sh\nset -m\nsleep 30 &\necho $! > \"{}\"\necho partial\n", held.display())).unwrap();
        let t = Instant::now();
        let e = spawn_within(&fake, &root, None, &["log"], &[], Duration::from_secs(20)).err().unwrap();
        assert!(e.to_string().contains("didn't end"), "{e}");
        assert!(t.elapsed() < Duration::from_secs(6), "{:?}", t.elapsed());
        if let Some(pid) = std::fs::read_to_string(&held).ok().and_then(|p| p.trim().parse::<i32>().ok()) {
            // SAFETY: the sleep the stand-in started, ended by the test.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
    }

    /// A git at `dir/git` (Windows: `git.exe`, and a `git.cmd` beside it that is never taken) that can be started.
    fn put_git(dir: &Path) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let f = dir.join(if cfg!(windows) { "git.exe" } else { "git" });
        std::fs::write(&f, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::write(dir.join("git.cmd"), "").unwrap();
        dunce::canonicalize(f).unwrap()
    }

    #[test]
    fn git_is_found_where_the_system_installs_it_then_on_the_users_path_never_in_a_plugins_folder_or_echos() {
        let (_env, root) = paths::testenv::folders("jshost-git-where");
        // The user's own tools folder on PATH: its git found. A folder given to a plugin, Echo's data folder, Echo's
        // own folder and a relative entry: skipped, even before it.
        let theirs = put_git(&root.join("tools/bin"));
        let granted = root.join("vault");
        put_git(&granted.join("bin"));
        crate::grants::grant("some-plugin", &granted).unwrap();
        put_git(&crate::paths::home().join("bin"));
        put_git(&crate::paths::config_dir().join("bin"));
        let path = std::env::join_paths([PathBuf::from("bin"), granted.join("bin"), crate::paths::home().join("bin"), crate::paths::config_dir().join("bin"), root.join("tools/bin")]).unwrap();
        assert_eq!(path_candidates(&path), std::slice::from_ref(&theirs));
        // A PATH folder that is a link into the folder given to a plugin: skipped too.
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(granted.join("bin"), root.join("linked-bin")).unwrap();
            let path = std::env::join_paths([root.join("linked-bin"), root.join("tools/bin")]).unwrap();
            assert_eq!(path_candidates(&path), std::slice::from_ref(&theirs));
        }
        // A git file that can't be started (Unix: not executable) isn't one.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&theirs, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(path_candidates(&path).is_empty());
            std::fs::set_permissions(&theirs, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        // The system's places, looked in first: none of the user's.
        let list = system_candidates();
        assert!(list.iter().all(|c| c.is_absolute() && !c.starts_with(&root)), "{list:?}");
        if let Ok(git) = program() {
            assert!(git.is_absolute() && !git.starts_with(&root), "{git:?}");
        }
    }

    /// A plugin with `folders.write` can lay out a bare repository's files in its own folder (`HEAD`, `config`,
    /// `objects/`, `refs/`, `info/attributes` aren't hidden) and change its `config` while git runs: such a folder is no
    /// repository Echo runs git in — only a real `.git` folder is, which no plugin can write.
    #[cfg(unix)]
    #[test]
    fn a_repository_a_plugin_laid_out_in_its_own_folder_is_never_used() {
        let (_env, root) = paths::testenv::folders("jshost-git-bare");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let vault = root.join("vault");
        install(&root, "bare", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        let v = dunce::canonicalize(&vault).unwrap().display().to_string();
        let mark = root.join("filtered");
        let filter = marker(&root.join("filter.sh"), &mark, true);
        let write = |rel: &str, text: &str| service("bare", "fs.write", &json!([v, rel, text])).unwrap();
        let benign = format!("[core]\n\trepositoryformatversion = 0\n\tbare = false\n\tworktree = {v}\n");
        let evil = format!("{benign}[filter \"evil\"]\n\tclean = {filter}\n");
        write("HEAD", "ref: refs/heads/main\n");
        write("config", &benign);
        write("info/attributes", "*.md filter=evil\n");
        write("a.md", "# A\n");
        for d in ["objects", "refs/heads"] {
            service("bare", "fs.mkdir", &json!([v, d])).unwrap();
        }
        // While git runs, the plugin flips its settings between harmless and a filter.
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flipper = {
            let (stop, v, benign, evil) = (stop.clone(), v.clone(), benign.clone(), evil.clone());
            std::thread::spawn(move || {
                let mut n = 0;
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = service("bare", "fs.write", &json!([v, "config", if n % 2 == 0 { &evil } else { &benign }]));
                    n += 1;
                    std::thread::sleep(Duration::from_millis(1));
                }
            })
        };
        let mut answers = vec![];
        for i in 0..20 {
            std::fs::write(vault.join("a.md"), format!("# A {i}\n")).unwrap();
            answers.push(run("bare", "commit", json!({"paths": ["a.md"], "message": "x"})));
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        flipper.join().unwrap();
        write("config", &benign);
        assert!(!mark.exists(), "a filter the plugin set ran");
        for a in answers {
            assert!(a.as_ref().is_err_and(|e| e.contains("a bare repository")), "{a:?}");
        }
        assert!(run("bare", "info", json!({})).unwrap_err().contains("a bare repository"));
        assert!(run("bare", "init", json!({})).unwrap_err().contains("a bare repository"));
        plugins::remove("bare", true).unwrap();
        sync();
    }

    /// The plugin writes nothing into a `.git` of its folder: by `ctx.fs.write`, `append`, `mkdir`, `move` or `copyIn`.
    #[test]
    fn a_plugin_writes_nothing_into_its_folders_git() {
        let (_env, root) = paths::testenv::folders("jshost-git-dotgit");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let vault = root.join("vault");
        install(&root, "inner", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        one_commit(&vault);
        let v = dunce::canonicalize(&vault).unwrap().display().to_string();
        let config = std::fs::read(vault.join(".git/config")).unwrap();
        std::fs::write(root.join("evil.txt"), "[core]\n\tfsmonitor = /bin/sh\n").unwrap();
        let handle = super::super::files::make("inner", &root.join("evil.txt")).unwrap();
        for (what, r) in [
            ("write", service("inner", "fs.write", &json!([v, ".git/config", "x"]))),
            ("write a hook", service("inner", "fs.write", &json!([v, ".git/hooks/pre-commit", "x"]))),
            ("append", service("inner", "fs.append", &json!([v, ".git/config", "x"]))),
            ("mkdir", service("inner", "fs.mkdir", &json!([v, ".git/info"]))),
            ("move", service("inner", "fs.move", &json!([v, "a.md", ".git/info/attributes"]))),
            ("copyIn", super::super::files::copy_in("inner", &handle, &v, ".git/config", false)),
            ("copyIn, a case away", super::super::files::copy_in("inner", &handle, &v, ".GIT/config", false)),
        ] {
            assert!(r.as_ref().is_err_and(|e| e.to_string().contains("hidden")), "{what}: {r:?}");
        }
        assert_eq!(std::fs::read(vault.join(".git/config")).unwrap(), config);
        assert!(!vault.join(".git/hooks/pre-commit").exists() && vault.join("a.md").exists());
        plugins::remove("inner", true).unwrap();
        sync();
    }

    /// `core.worktree` at an ancestor and `.git/objects` and `refs` linked into another repository: every path is
    /// inside that "top", and a commit would land in the other repository. Refused; so is any `core.worktree`.
    #[cfg(unix)]
    #[test]
    fn a_repository_whose_worktree_is_set_elsewhere_is_refused() {
        use std::os::unix::fs::symlink;
        let (_env, root) = paths::testenv::folders("jshost-git-worktree");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let other = root.join("private");
        one_commit(&other);
        let head = user_git(&other, &["rev-parse", "HEAD"]);
        let vault = root.join("vault");
        install(&root, "wide", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        user_git(&vault, &["init", "-q", "-b", "main"]);
        std::fs::write(vault.join("n.md"), "# N\n").unwrap();
        for d in ["objects", "refs"] {
            std::fs::remove_dir_all(vault.join(".git").join(d)).unwrap();
            symlink(other.join(".git").join(d), vault.join(".git").join(d)).unwrap();
        }
        for top in ["/", root.to_str().unwrap()] {
            user_git(&vault, &["config", "core.worktree", top]);
            for (cmd, a) in [("info", json!({})), ("status", json!({})), ("log", json!({})), ("commit", json!({"paths": ["n.md"], "message": "x"}))] {
                let r = run("wide", cmd, a);
                assert!(r.as_ref().is_err_and(|e| e.contains("core.worktree")), "{top} {cmd}: {r:?}");
            }
            assert_eq!(user_git(&other, &["rev-parse", "HEAD"]), head, "nothing committed into the other repository");
        }
        // Set to the vault itself: still refused (Echo sets the work tree; the repository never does).
        user_git(&vault, &["config", "core.worktree", vault.to_str().unwrap()]);
        assert!(run("wide", "info", json!({})).unwrap_err().contains("core.worktree"));
        plugins::remove("wide", true).unwrap();
        sync();
    }

    /// A script at `at` that leaves `mark` when it runs; `reads`: then passes its input on (a filter, a pager).
    #[cfg(unix)]
    fn marker(at: &Path, mark: &Path, reads: bool) -> String {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(at, format!("#!/bin/sh\ntouch \"{}\"\n{}", mark.display(), if reads { "cat\n" } else { "" })).unwrap();
        std::fs::set_permissions(at, std::fs::Permissions::from_mode(0o755)).unwrap();
        at.display().to_string()
    }

    /// A repository with one commit of `a.md`, then `a.md` changed on the disk.
    fn one_commit(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        user_git(dir, &["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("a.md"), "# A\n").unwrap();
        user_git(dir, &["add", "a.md"]);
        user_git(dir, &[NO_MAINTENANCE, &["commit", "-q", "-m", "start"]].concat());
        std::fs::write(dir.join("a.md"), "# A, changed\n").unwrap();
    }

    /// Plain git, as the user would run it, its answer ignored: only whether it ran the trap matters.
    #[cfg(unix)]
    fn plain_git(dir: &Path, args: &[&str]) {
        let _ = Command::new(program().unwrap()).arg("-C").arg(dir).args(args).stdin(Stdio::null()).output();
    }

    /// A partial clone missing an object asks its promisor remote for it — a shell, through `ext::`: plain git runs it,
    /// Echo's git never does (no lazy fetch, no protocol allowed), and refuses such a repository outright.
    #[cfg(unix)]
    #[test]
    fn a_partial_clone_fetches_nothing_and_is_refused() {
        let (_env, root) = paths::testenv::folders("jshost-git-lazy");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let mark = root.join("fetched");
        let rig = |dir: &Path| {
            one_commit(dir);
            let blob = user_git(dir, &["rev-parse", "HEAD:a.md"]).trim().to_string();
            std::fs::remove_file(dir.join(".git/objects").join(&blob[..2]).join(&blob[2..])).unwrap();
            let url = format!("ext::sh -c touch% {}", mark.display());
            for (k, v) in [("core.repositoryformatversion", "1"), ("extensions.partialClone", "origin"), ("remote.origin.url", url.as_str()), ("remote.origin.promisor", "true"), ("protocol.ext.allow", "always")] {
                user_git(dir, &["config", k, v]);
            }
        };
        let control = root.join("control");
        rig(&control);
        plain_git(&control, &["cat-file", "blob", "HEAD:a.md"]);
        assert!(mark.exists(), "the control: plain git fetched it through the shell");
        std::fs::remove_file(&mark).unwrap();

        let vault = root.join("vault");
        rig(&vault);
        install(&root, "lazy", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        for (cmd, a) in [("info", json!({})), ("status", json!({})), ("log", json!({"path": "a.md"})), ("show", json!({"rev": "HEAD", "path": "a.md"})), ("commit", json!({"paths": ["a.md"], "message": "x"}))] {
            assert!(run("lazy", cmd, a).unwrap_err().contains("partial clone"), "{cmd}");
        }
        // Even were it not refused, git as Echo starts it fetches nothing.
        let _ = spawn(&program().unwrap(), &vault, &["cat-file", "blob", "HEAD:a.md"], &[]);
        assert!(!mark.exists(), "nothing ran");
        plugins::remove("lazy", true).unwrap();
        sync();
    }

    /// A filter whose name isn't UTF-8 can't be cleared by its name read as text: the repository is refused.
    #[cfg(unix)]
    #[test]
    fn a_setting_named_in_bytes_that_arent_text_refuses_the_repository() {
        let (_env, root) = paths::testenv::folders("jshost-git-bytes");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let mark = root.join("filtered");
        let rig = |dir: &Path| {
            one_commit(dir);
            let mut cfg = std::fs::read(dir.join(".git/config")).unwrap();
            cfg.extend_from_slice(b"[filter \"\xff\"]\n\tclean = ");
            cfg.extend_from_slice(marker(&dir.with_extension("sh"), &mark, true).as_bytes());
            cfg.push(b'\n');
            std::fs::write(dir.join(".git/config"), cfg).unwrap();
            std::fs::write(dir.join(".gitattributes"), b"*.md filter=\xff\n").unwrap();
        };
        let control = root.join("control");
        rig(&control);
        plain_git(&control, &["add", "a.md"]);
        assert!(mark.exists(), "the control: plain git ran the filter");
        std::fs::remove_file(&mark).unwrap();

        let vault = root.join("vault");
        rig(&vault);
        install(&root, "bytes", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        for (cmd, a) in [("status", json!({})), ("commit", json!({"paths": ["a.md"], "message": "x"}))] {
            assert!(run("bytes", cmd, a).unwrap_err().contains("UTF-8"), "{cmd}");
        }
        assert!(!mark.exists(), "nothing ran");
        plugins::remove("bytes", true).unwrap();
        sync();
    }

    /// Settings past what Echo reads of them (a 17 MB file included first) would hide a filter after them: refused.
    #[cfg(unix)]
    #[test]
    fn settings_too_long_to_read_whole_refuse_the_repository() {
        let (_env, root) = paths::testenv::folders("jshost-git-long");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let mark = root.join("filtered");
        let pad = root.join("pad.gitconfig");
        let line = format!("\tk = {}\n", "x".repeat(1000));
        std::fs::write(&pad, format!("[pad]\n{}", line.repeat((OUT_MAX + (1 << 20)) / line.len()))).unwrap();
        let rig = |dir: &Path| {
            one_commit(dir);
            let mut cfg = std::fs::read_to_string(dir.join(".git/config")).unwrap();
            cfg.push_str(&format!("[include]\n\tpath = {}\n[filter \"evil\"]\n\tclean = {}\n", pad.display(), marker(&dir.with_extension("sh"), &mark, true)));
            std::fs::write(dir.join(".git/config"), cfg).unwrap();
            std::fs::write(dir.join(".gitattributes"), "*.md filter=evil\n").unwrap();
        };
        let control = root.join("control");
        rig(&control);
        plain_git(&control, &["add", "a.md"]);
        assert!(mark.exists(), "the control: plain git ran the filter");
        std::fs::remove_file(&mark).unwrap();

        let vault = root.join("vault");
        rig(&vault);
        install(&root, "long", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        assert!(run("long", "commit", json!({"paths": ["a.md"], "message": "x"})).unwrap_err().contains("settings"));
        assert!(!mark.exists(), "nothing ran");
        plugins::remove("long", true).unwrap();
        sync();
    }

    /// `.git/objects`, `refs` or a folder of objects linked to another repository's: its files would show. Refused.
    #[cfg(unix)]
    #[test]
    fn a_repository_whose_history_is_linked_from_elsewhere_is_refused() {
        use std::os::unix::fs::symlink;
        let (_env, root) = paths::testenv::folders("jshost-git-linked");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let other = root.join("private");
        std::fs::create_dir_all(&other).unwrap();
        user_git(&other, &["init", "-q", "-b", "main"]);
        std::fs::write(other.join("secret.md"), "budget of someone else\n").unwrap();
        user_git(&other, &["add", "-A"]);
        user_git(&other, &["commit", "-q", "-m", "private"]);
        let head = user_git(&other, &["rev-parse", "HEAD"]);
        let vault = root.join("vault");
        install(&root, "linked", json!({"folders": {"what": "vaults"}}), &vault);
        let fresh = || {
            let _ = std::fs::remove_dir_all(vault.join(".git"));
            user_git(&vault, &["init", "-q", "-b", "main"]);
        };
        // Its objects folder a link to the other's, its branch at the other's commit.
        fresh();
        std::fs::remove_dir_all(vault.join(".git/objects")).unwrap();
        symlink(other.join(".git/objects"), vault.join(".git/objects")).unwrap();
        std::fs::write(vault.join(".git/refs/heads/main"), &head).unwrap();
        assert!(run("linked", "show", json!({"rev": "HEAD", "path": "secret.md"})).unwrap_err().contains("outside its folder"), "objects");
        // Its refs and objects of one prefix linked.
        fresh();
        std::fs::remove_dir_all(vault.join(".git/refs")).unwrap();
        symlink(other.join(".git/refs"), vault.join(".git/refs")).unwrap();
        assert!(run("linked", "log", json!({})).unwrap_err().contains("outside its folder"), "refs");
        fresh();
        let blob = user_git(&other, &["rev-parse", "HEAD:secret.md"]);
        symlink(other.join(".git/objects").join(&blob[..2]), vault.join(".git/objects").join(&blob[..2])).unwrap();
        assert!(run("linked", "info", json!({})).unwrap_err().contains("outside its folder"), "a folder of objects");
        plugins::remove("linked", true).unwrap();
        sync();
    }

    /// A `.git` a plugin could write in — a folder inside it given to a plugin, or `.git` a link to a folder elsewhere:
    /// refused.
    #[test]
    fn a_git_folder_a_plugin_could_write_in_or_a_link_is_refused() {
        let (_env, root) = paths::testenv::folders("jshost-git-dotgit-reach");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let vault = root.join("vault");
        install(&root, "near", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        one_commit(&vault);
        crate::grants::grant("other-plugin", &vault.join(".git/info")).unwrap();
        assert!(run("near", "info", json!({})).unwrap_err().contains("inside this repository's .git is given to a plugin"));
        crate::grants::forget("other-plugin", &vault.join(".git/info")).unwrap();
        assert_eq!(run("near", "info", json!({})).unwrap()["repo"], true);
        #[cfg(unix)]
        {
            std::fs::rename(vault.join(".git"), root.join("real-git")).unwrap();
            std::os::unix::fs::symlink(root.join("real-git"), vault.join(".git")).unwrap();
            for (cmd, a) in [("info", json!({})), ("log", json!({})), ("commit", json!({"paths": ["a.md"], "message": "x"}))] {
                assert!(run("near", cmd, a).unwrap_err().contains("outside its folder"), "{cmd}");
            }
        }
        plugins::remove("near", true).unwrap();
        sync();
    }

    /// The user's `.git/config` includes `../tools/git.cfg`, a file in the vault a plugin may write: missing when Echo
    /// reads the settings, then written by the plugin (`ctx.fs.write`) while `commit` runs — a filter the listing never
    /// saw. Refused whether the file is there or not, and nothing runs.
    #[cfg(unix)]
    #[test]
    fn settings_that_include_a_file_a_plugin_can_write_refuse_the_repository() {
        let (_env, root) = paths::testenv::folders("jshost-git-include");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let vault = root.join("vault");
        install(&root, "inc", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        one_commit(&vault);
        let v = dunce::canonicalize(&vault).unwrap().display().to_string();
        let mark = root.join("filtered");
        let filter = marker(&root.join("filter.sh"), &mark, true);
        let config = std::fs::read_to_string(vault.join(".git/config")).unwrap();
        std::fs::write(vault.join(".git/config"), format!("{config}[include]\n\tpath = ../tools/git.cfg\n")).unwrap();
        service("inc", "fs.write", &json!([v, "tools/attrs", "*.md filter=evil\n"])).unwrap();
        let evil = format!("[filter \"evil\"]\n\tclean = {filter}\n[core]\n\tattributesFile = {v}/tools/attrs\n");
        // While Echo commits, the plugin writes the included file, then takes it away again.
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flipper = {
            let (stop, v, evil) = (stop.clone(), v.clone(), evil.clone());
            std::thread::spawn(move || {
                let mut n = 0;
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = service("inc", "fs.write", &json!([v, "tools/git.cfg", if n % 2 == 0 { evil.as_str() } else { "" }]));
                    n += 1;
                    std::thread::sleep(Duration::from_millis(1));
                }
            })
        };
        let mut answers = vec![];
        for i in 0..20 {
            std::fs::write(vault.join("a.md"), format!("# A {i}\n")).unwrap();
            answers.push(run("inc", "commit", json!({"paths": ["a.md"], "message": "x"})));
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        flipper.join().unwrap();
        assert!(!mark.exists(), "a filter the plugin set ran");
        for a in answers {
            assert!(a.as_ref().is_err_and(|e| e.contains("a plugin could change")), "{a:?}");
        }
        let refused = |why: &str| {
            for (cmd, a) in [("info", json!({})), ("status", json!({})), ("commit", json!({"paths": ["a.md"], "message": "x"}))] {
                let r = run("inc", cmd, a);
                assert!(r.as_ref().is_err_and(|e| e.contains("a plugin could change")), "{why}: {cmd}: {r:?}");
            }
        };
        // Missing, under an includeIf, in the user's own settings: refused all the same.
        std::fs::remove_file(vault.join("tools/git.cfg")).unwrap();
        refused("missing");
        std::fs::write(vault.join(".git/config"), format!("{config}[includeIf \"gitdir:/\"]\n\tpath = {v}/later/git.cfg\n")).unwrap();
        refused("includeIf, its folder not there yet");
        std::fs::write(vault.join(".git/config"), &config).unwrap();
        assert_eq!(run("inc", "info", json!({})).unwrap()["repo"], true, "without it the repository works");
        let home = root.join("user/.gitconfig");
        let user = std::fs::read_to_string(&home).unwrap();
        std::fs::write(&home, format!("{user}[include]\n\tpath = ~/../vault/tools/git.cfg\n")).unwrap();
        refused("the user's settings include it");
        // The settings files themselves linked into the vault: the repository's, the user's.
        std::fs::write(&home, &user).unwrap();
        std::fs::rename(vault.join(".git/config"), vault.join("tools/config")).unwrap();
        std::os::unix::fs::symlink(vault.join("tools/config"), vault.join(".git/config")).unwrap();
        refused("the repository's settings a link into the vault");
        std::fs::remove_file(vault.join(".git/config")).unwrap();
        std::fs::rename(vault.join("tools/config"), vault.join(".git/config")).unwrap();
        std::fs::rename(&home, vault.join("tools/gitconfig")).unwrap();
        std::os::unix::fs::symlink(vault.join("tools/gitconfig"), &home).unwrap();
        refused("the user's settings a link into the vault");
        std::fs::remove_file(&home).unwrap();
        std::fs::rename(vault.join("tools/gitconfig"), &home).unwrap();
        // A file of the work tree above the vault, outside a hidden folder: refused; in a hidden one, used.
        let top = root.join("repo");
        let inner = top.join("Vault");
        one_commit(&top);
        install(&root, "sub", json!({"folders": {"what": "vaults", "write": true}}), &inner);
        let cfg = std::fs::read_to_string(top.join(".git/config")).unwrap();
        std::fs::write(top.join(".git/config"), format!("{cfg}[include]\n\tpath = ../shared/git.cfg\n")).unwrap();
        assert!(run("sub", "info", json!({})).unwrap_err().contains("a plugin could change"));
        std::fs::write(top.join(".git/config"), format!("{cfg}[include]\n\tpath = ../.shared/git.cfg\n")).unwrap();
        assert_eq!(run("sub", "info", json!({})).unwrap()["repo"], true);
        assert!(!mark.exists(), "nothing ran");
        for id in ["inc", "sub"] {
            plugins::remove(id, true).unwrap();
        }
        sync();
    }

    /// A made-up git identity at `at`: a test environment's own settings file.
    fn test_identity(at: &Path) {
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, "[user]\n\tname = Sam Rivera\n\temail = sam@initech.example\n").unwrap();
    }

    #[test]
    fn a_commit_under_a_test_environments_own_git_settings_is_by_its_made_up_user() {
        let (_env, root) = paths::testenv::folders("jshost-git-global");
        // Git reads `GIT_CONFIG_GLOBAL` from 2.32 on.
        if program().is_err() || !git_at_least(2, 32) {
            return;
        }
        // The home's settings: another made-up person, standing for the user's own, which the environment's replace.
        std::fs::write(root.join("user/.gitconfig"), "[user]\n\tname = Pat Doe\n\temail = pat@globex.example\n").unwrap();
        let global = root.join("test-data/gitconfig");
        test_identity(&global);
        std::env::set_var("GIT_CONFIG_GLOBAL", &global);
        let vault = root.join("vault");
        install(&root, "tester", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        run("tester", "init", json!({})).unwrap();
        std::fs::write(vault.join("Acme.md"), "# Acme\n").unwrap();
        run("tester", "commit", json!({"paths": ["Acme.md"], "message": "Acme: notes"})).unwrap();
        let log = run("tester", "log", json!({})).unwrap();
        assert_eq!((log[0]["author"].clone(), log[0]["email"].clone(), log[0]["echo"].clone()), (json!("Sam Rivera"), json!("sam@initech.example"), json!("tester")));
        assert_eq!(user_git(&vault, &["log", "-1", "--format=%an <%ae> / %cn <%ce>"]).trim(), "Sam Rivera <sam@initech.example> / Echo (tester) <tester@plugins.echo.invalid>");
        // Empty: no user settings at all (git reads none), so Echo is the author too — never the home's.
        std::env::set_var("GIT_CONFIG_GLOBAL", "");
        std::fs::write(vault.join("Acme.md"), "# Acme\nv2\n").unwrap();
        run("tester", "commit", json!({"paths": ["Acme.md"], "message": "Acme: v2"})).unwrap();
        assert_eq!(run("tester", "log", json!({"max": 1})).unwrap()[0]["author"], "Echo (tester)");
        plugins::remove("tester", true).unwrap();
        sync();
    }

    #[test]
    fn a_test_environments_git_settings_a_plugin_could_change_refuse_the_repository_as_the_users_would() {
        let (_env, root) = paths::testenv::folders("jshost-git-global-reach");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let vault = root.join("vault");
        install(&root, "reach", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        one_commit(&vault);
        let top = root.join("repo");
        one_commit(&top);
        install(&root, "sub", json!({"folders": {"what": "vaults", "write": true}}), &top.join("Vault"));
        let refused = |id: &str, at: &str, why: &str, want: &str| {
            std::env::set_var("GIT_CONFIG_GLOBAL", at);
            for (cmd, a) in [("info", json!({})), ("status", json!({})), ("commit", json!({"paths": ["a.md"], "message": "x"}))] {
                let r = run(id, cmd, a);
                assert!(r.as_ref().is_err_and(|e| e.contains(want)), "{why}: {cmd}: {r:?}");
            }
        };
        let s = |p: PathBuf| p.display().to_string();
        // Inside the folder given to a plugin — a hidden folder of it too, there or not.
        let hidden = vault.join(".tools/gitconfig");
        test_identity(&hidden);
        refused("reach", &s(hidden), "in a grant's hidden folder", "a plugin could change");
        refused("reach", &s(vault.join(".later/gitconfig")), "in a grant, not there yet", "a plugin could change");
        // In the work tree outside a hidden folder, though outside the grant.
        let shared = top.join("shared/gitconfig");
        test_identity(&shared);
        refused("sub", &s(shared), "in the work tree", "a plugin could change");
        // Inside Echo's data or own folder.
        for (at, why) in [(root.join("home/gitconfig"), "Echo's data folder"), (root.join("config/gitconfig"), "Echo's own folder")] {
            test_identity(&at);
            refused("reach", &s(at), why, "a plugin could change");
        }
        // A file elsewhere that includes one a plugin can write.
        let outside = root.join("test-data/gitconfig");
        test_identity(&outside);
        std::fs::write(&outside, format!("[user]\n\tname = Sam Rivera\n[include]\n\tpath = {}\n", s(vault.join(".tools/gitconfig")))).unwrap();
        refused("reach", &s(outside.clone()), "includes a file in a grant", "a plugin could change");
        // The home's settings a link into the grant, the environment's file elsewhere: refused all the same (a git
        // older than 2.32 reads the home's).
        test_identity(&outside);
        let home = root.join("user/.gitconfig");
        std::fs::rename(&home, vault.join(".tools/home.gitconfig")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(vault.join(".tools/home.gitconfig"), &home).unwrap();
            refused("reach", &s(outside.clone()), "the home's settings in a grant", "a plugin could change");
            std::fs::remove_file(&home).unwrap();
        }
        std::fs::rename(vault.join(".tools/home.gitconfig"), &home).unwrap();
        // Not a full path: git would read it from the folder it runs in.
        refused("reach", "gitconfig", "relative", "not a full path");
        // Elsewhere, and in the work tree's hidden folder: used.
        test_identity(&outside);
        std::env::set_var("GIT_CONFIG_GLOBAL", &outside);
        assert_eq!(run("reach", "info", json!({})).unwrap()["repo"], true);
        let tucked = top.join(".shared/gitconfig");
        test_identity(&tucked);
        std::env::set_var("GIT_CONFIG_GLOBAL", &tucked);
        assert_eq!(run("sub", "info", json!({})).unwrap()["repo"], true);
        for id in ["reach", "sub"] {
            plugins::remove(id, true).unwrap();
        }
        sync();
    }

    #[test]
    fn a_version_is_a_commit_and_a_commit_waits_for_a_merge_and_takes_no_ignored_file() {
        let (_env, root) = paths::testenv::folders("jshost-git-edges");
        if program().is_err() {
            return;
        }
        user_identity(&root);
        let vault = root.join("vault");
        install(&root, "edges", json!({"folders": {"what": "vaults", "write": true}}), &vault);
        one_commit(&vault);
        // A blob's or a tree's id isn't a version.
        for id in [user_git(&vault, &["rev-parse", "HEAD:a.md"]), user_git(&vault, &["rev-parse", "HEAD^{tree}"])] {
            assert!(run("edges", "show", json!({"rev": id.trim(), "path": "a.md"})).unwrap_err().contains("isn't a commit"), "{id}");
        }
        // A version that isn't UTF-8: replaced by default, refused with `strict`; a character `max` cuts is dropped.
        std::fs::write(vault.join("latin.md"), b"caf\xe9\n").unwrap();
        std::fs::write(vault.join("utf.md"), "café\n").unwrap();
        assert!(run("edges", "commit", json!({"paths": ["latin.md", "utf.md"], "message": "two"})).unwrap()["rev"].is_string());
        assert_eq!(run("edges", "show", json!({"rev": "HEAD", "path": "latin.md"})).unwrap()["text"], "caf\u{FFFD}\n");
        assert!(run("edges", "show", json!({"rev": "HEAD", "path": "latin.md", "strict": true})).unwrap_err().contains("not UTF-8: latin.md"));
        assert_eq!(run("edges", "show", json!({"rev": "HEAD", "path": "utf.md", "strict": true})).unwrap()["text"], "café\n");
        assert_eq!(run("edges", "show", json!({"rev": "HEAD", "path": "utf.md", "max": 4, "strict": true})).unwrap()["text"], "caf");
        // An ignored file: said so, nothing staged — the other new file named with it neither.
        std::fs::write(vault.join(".gitignore"), "ignored.md\n").unwrap();
        std::fs::write(vault.join("ignored.md"), "x\n").unwrap();
        std::fs::write(vault.join("new.md"), "x\n").unwrap();
        assert!(run("edges", "commit", json!({"paths": ["ignored.md"], "message": "x"})).unwrap_err().contains("ignored"));
        assert!(run("edges", "commit", json!({"paths": ["new.md", "ignored.md"], "message": "x"})).unwrap_err().contains("ignored"));
        assert_eq!(user_git(&vault, &["diff", "--cached", "--name-only"]), "", "nothing staged");
        // A merge under way: no commit until it's done.
        std::fs::write(vault.join(".git/MERGE_HEAD"), user_git(&vault, &["rev-parse", "HEAD"])).unwrap();
        assert_eq!(run("edges", "info", json!({})).unwrap()["busy"], "merge");
        assert!(run("edges", "commit", json!({"paths": ["a.md"], "message": "x"})).unwrap_err().contains("merge"));
        std::fs::remove_file(vault.join(".git/MERGE_HEAD")).unwrap();
        assert!(run("edges", "commit", json!({"paths": ["a.md"], "message": "x"})).unwrap()["rev"].is_string());
        plugins::remove("edges", true).unwrap();
        sync();
    }

    #[test]
    fn an_include_with_a_drive_but_no_root_is_one_echo_cant_place() {
        let git = Path::new("/usr/bin/git");
        let origin = "file:/data/acme-vault/.git/config";
        for v in [r"C:tools\git.cfg", "c:git.cfg", "C:"] {
            assert_eq!(super::include_target(v, origin, git), None, "{v}");
        }
        // A drive with its root, or a plain relative path, is placed.
        for v in [r"C:\tools\git.cfg", "C:/tools/git.cfg", "tools/git.cfg"] {
            assert!(super::include_target(v, origin, git).is_some(), "{v}");
        }
    }
}
