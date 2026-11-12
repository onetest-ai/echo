//! `ctx.signin`: the sign-in saved on this computer for a host, handed to a plugin's host half that named the host in
//! `uses.signin.hosts` (`plugins::may_use_signin`). A plugin can't run programs, and the GitHub CLI keeps its token in
//! the system's keychain, so Echo asks for it: the GitHub CLI first (`gh auth token`), then git's credential helper
//! (`git credential fill`: the keychain, Git Credential Manager; for a self-hosted git this may be the user's password).
//! Echo asks nobody for a password (the system's keychain may ask once to allow it), each program at most 10 s, with
//! all it starts stopped on time-out (on Unix its own process group, on Windows a job object closed when it is done),
//! from an empty folder with the git and GitHub CLI settings of Echo's own environment removed — a repository's
//! `.git/config` or an inherited `GIT_CONFIG_*` / `GH_TOKEN` never chooses the helper or the account. One lookup per
//! host at a time, kept a minute (none found: 10 s) and then dropped from memory; `ctx.signin.get(host, {fresh: true})`
//! asks the programs again and replaces what was kept (a plugin whose token was refused). The token is never written
//! or logged by Echo; it goes to the host half only.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::app::NoWindow;

/// How long one program is given to answer.
const LIMIT: Duration = Duration::from_secs(10);
/// How long a sign-in found (or none) is kept before the programs are asked again.
const KEEP: Duration = Duration::from_secs(60);
const KEEP_NONE: Duration = Duration::from_secs(10);
/// The most of a program's output read.
const MAX_OUT: usize = 64 * 1024;

/// The user's sign-in to `host`: `{token, from: "gh" | "git", user?}`, or `None` when neither program has one.
/// `fresh`: ask the programs again instead of what was kept, and keep their answer.
pub fn get(host: &str, fresh: bool) -> Option<Value> {
    cached(host, fresh, (KEEP, KEEP_NONE), || get_with(host, program("gh").as_deref(), program("git").as_deref(), &|_| {}))
}

/// Which of the programs that could hold a sign-in are on this computer (no sign-in read): `{gh, git}`.
pub fn sources() -> Value {
    json!({"gh": program("gh").is_some(), "git": program("git").is_some()})
}

type Slot = Arc<Mutex<Option<(Instant, Option<Value>)>>>;

/// The answers kept, by host.
fn slots() -> &'static Mutex<HashMap<String, Slot>> {
    static SLOTS: OnceLock<Mutex<HashMap<String, Slot>>> = OnceLock::new();
    SLOTS.get_or_init(Default::default)
}

/// `look` for `host` once at a time, its answer kept `keep.0` (none: `keep.1`) — a widget's refresh doesn't start the
/// programs each time — then dropped (the token isn't held in memory longer); `fresh` looks again whatever is kept.
fn cached(host: &str, fresh: bool, keep: (Duration, Duration), look: impl FnOnce() -> Option<Value>) -> Option<Value> {
    let slot = slots().lock().unwrap_or_else(|e| e.into_inner()).entry(host.to_string()).or_default().clone();
    let mut kept = slot.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, v)) = &*kept {
        if !fresh && at.elapsed() < if v.is_some() { keep.0 } else { keep.1 } {
            return v.clone();
        }
    }
    let v = look();
    let at = Instant::now();
    *kept = Some((at, v.clone()));
    drop(kept);
    let (host, ttl) = (host.to_string(), if v.is_some() { keep.0 } else { keep.1 });
    std::thread::spawn(move || {
        std::thread::sleep(ttl);
        forget(&host, &slot, at);
    });
    v
}

/// Drop `host`'s answer kept since `at` (not a newer one), and its slot when nobody else is using it.
fn forget(host: &str, slot: &Slot, at: Instant) {
    {
        let mut kept = slot.lock().unwrap_or_else(|e| e.into_inner());
        if kept.as_ref().is_some_and(|(t, _)| *t == at) {
            *kept = None;
        }
    }
    let mut all = slots().lock().unwrap_or_else(|e| e.into_inner());
    // Only the map's and this one's: no lookup has it, and (unlocked) nothing new is kept in it.
    if all.get(host).is_some_and(|s| Arc::ptr_eq(s, slot) && Arc::strong_count(s) == 2 && s.try_lock().is_ok_and(|k| k.is_none())) {
        all.remove(host);
    }
}

/// [`get`] with the programs given (tests: made-up ones), uncached; `inherit` sets up what Echo's own environment
/// would hand the programs (tests: a planted repository, `GIT_CONFIG_PARAMETERS`) before Echo clears it.
pub(crate) fn get_with(host: &str, gh: Option<&Path>, git: Option<&Path>, inherit: &dyn Fn(&mut Command)) -> Option<Value> {
    if gh.is_none() && git.is_none() {
        return None;
    }
    let empty = EmptyDir::new()?;
    if let Some(gh) = gh {
        let mut c = Command::new(gh).no_window();
        inherit(&mut c);
        c.args(["auth", "token", "--hostname", host]);
        clean(&mut c, &empty.0);
        c.env("GH_PROMPT_DISABLED", "1").env("GH_NO_UPDATE_NOTIFIER", "1");
        if let Some((true, out)) = run(c, None) {
            if let Some(token) = Some(out.trim()).filter(|t| plain(t) && !t.contains(' ')) {
                return Some(json!({"token": token, "from": "gh"}));
            }
        }
    }
    let git = git?;
    let mut c = Command::new(git).no_window();
    inherit(&mut c);
    // `credential.interactive` (git 2.46+; an older git ignores a setting it doesn't know): helpers mustn't prompt.
    c.args(["-c", "credential.interactive=false", "credential", "fill"]);
    clean(&mut c, &empty.0);
    // Nobody is asked: no terminal prompt, no askpass program (one that can't start), no Credential Manager window.
    c.env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "echo-asks-nobody")
        .env("SSH_ASKPASS", "echo-asks-nobody")
        .env("GCM_INTERACTIVE", "never")
        .env("GCM_GUI_PROMPT", "false");
    let (ok, out) = run(c, Some(&format!("protocol=https\nhost={host}\n\n")))?;
    if !ok {
        return None;
    }
    // A value is the rest of its line (a password may hold spaces), less a Windows line end.
    let field = |k: &str| out.lines().find_map(|l| l.strip_prefix(k)?.strip_prefix('=')).map(|v| v.trim_end_matches('\r')).filter(|v| plain(v));
    let token = field("password")?;
    let mut v = json!({"token": token, "from": "git"});
    if let Some(user) = field("username") {
        v["user"] = json!(user);
    }
    Some(v)
}

/// A sign-in's text: not empty, no line breaks or other control characters.
fn plain(v: &str) -> bool {
    !v.is_empty() && !v.chars().any(char::is_control)
}

/// Run from `dir` (empty, Echo's own) with nothing of Echo's environment that would choose another helper, config,
/// repository or account: `GIT_*` (`GIT_CONFIG_PARAMETERS`, `GIT_CONFIG_COUNT`/`_KEY_n`/`_VALUE_n`, `GIT_DIR`,
/// `GIT_WORK_TREE`, `GIT_CONFIG_GLOBAL`, `GIT_EXEC_PATH` …), `GCM_*`, `GH_*` (`GH_TOKEN`, `GH_HOST`, `GH_CONFIG_DIR` …),
/// `GITHUB_TOKEN`, `GITHUB_ENTERPRISE_TOKEN`; git looks for no repository above `dir`.
fn clean(c: &mut Command, dir: &Path) {
    let names: Vec<OsString> = std::env::vars_os().map(|(k, _)| k).chain(c.get_envs().map(|(k, _)| k.to_os_string())).collect();
    for k in names {
        let up = k.to_string_lossy().to_ascii_uppercase();
        if ["GIT_", "GH_", "GCM_"].iter().any(|p| up.starts_with(p)) || up == "GITHUB_TOKEN" || up == "GITHUB_ENTERPRISE_TOKEN" {
            c.env_remove(&k);
        }
    }
    c.current_dir(dir);
    if let Some(parent) = dir.parent() {
        c.env("GIT_CEILING_DIRECTORIES", parent);
    }
}

/// A new empty folder of Echo's for the programs to run from, removed after with whatever they left in it.
struct EmptyDir(PathBuf);

impl EmptyDir {
    fn new() -> Option<Self> {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        let d = std::env::temp_dir().join(format!("echo-signin-{}-{n}-{nanos}", std::process::id()));
        std::fs::create_dir(&d).ok()?;
        Some(EmptyDir(d))
    }
}

impl Drop for EmptyDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A program on this computer that can be run without a window (tests: only the made-up ones they set).
#[cfg(not(test))]
fn program(name: &str) -> Option<PathBuf> {
    let mut found = crate::app::program_candidates(name);
    if cfg!(windows) {
        let under = |k: &str, rel: &str| std::env::var_os(k).map(|d| PathBuf::from(d).join(rel));
        let extra = if name == "gh" {
            vec![under("ProgramFiles", r"GitHub CLI\gh.exe"), under("ProgramFiles(x86)", r"GitHub CLI\gh.exe")]
        } else {
            vec![under("ProgramFiles", r"Git\cmd\git.exe"), under("ProgramFiles(x86)", r"Git\cmd\git.exe"), under("LOCALAPPDATA", r"Programs\Git\cmd\git.exe")]
        };
        found.extend(extra.into_iter().flatten());
    }
    pick(found)
}

#[cfg(test)]
pub(crate) static FAKE_PROGRAMS: Mutex<Vec<(&str, PathBuf)>> = Mutex::new(Vec::new());

#[cfg(test)]
fn program(name: &str) -> Option<PathBuf> {
    FAKE_PROGRAMS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(n, _)| *n == name).map(|(_, p)| p.clone())
}

/// The first of `found` that is a file by its full path (an empty PATH entry's `gh` is not) and can run: on macOS not
/// the `/usr/bin/git` stand-in that would offer to install Apple's developer tools — the next one instead.
#[cfg_attr(test, allow(dead_code))]
fn pick(found: Vec<PathBuf>) -> Option<PathBuf> {
    found.into_iter().find(|p| p.is_absolute() && p.is_file() && usable(p))
}

fn usable(p: &Path) -> bool {
    if cfg!(target_os = "macos") && p == Path::new("/usr/bin/git") {
        return ["/Library/Developer/CommandLineTools/usr/bin/git", "/Applications/Xcode.app/Contents/Developer/usr/bin/git"].iter().any(|g| Path::new(g).is_file());
    }
    true
}

/// Run `c` with `input` on its stdin: (exited 0, its stdout), or `None` when it can't start or takes over [`LIMIT`].
/// It runs with all it starts in a [`Tree`]: on time-out that is stopped (a helper it started, `security`, a `sleep`),
/// and when it ends with something it started still holding its output, that is stopped after 2 s and what it wrote
/// kept. Its stderr is dropped: nothing it says is logged.
fn run(mut c: Command, input: Option<&str>) -> Option<(bool, String)> {
    c.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::null());
    let (mut child, tree) = Tree::spawn(c)?;
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        let _ = stdin.write_all(text.as_bytes());
    }
    let t = Instant::now();
    let Some(stdout) = child.stdout.take() else {
        tree.stop(&mut child);
        return None;
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read_until(stdout, t + LIMIT + Duration::from_secs(5)));
    });
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if t.elapsed() < LIMIT => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                tree.stop(&mut child);
                return None;
            }
        }
    };
    let out = match rx.recv_timeout(LIMIT.saturating_sub(t.elapsed()).min(Duration::from_secs(2)).max(Duration::from_millis(200))) {
        Ok(out) => out,
        Err(_) => {
            // What it started still holds its output: stopped, which closes it (the reader then ends with what it read).
            tree.stop_rest();
            rx.recv_timeout(Duration::from_secs(2)).ok().flatten()
        }
    }?;
    Some((status.success(), out))
}

/// A program and everything it starts, stopped together: on Unix its own process group (`ai::detach`); on Windows a
/// job object it is started in (suspended until it is in), which ends them all when terminated or closed — even what
/// outlives the program, which `taskkill /T` on an ended program can't reach.
struct Tree {
    #[cfg(unix)]
    pid: u32,
    #[cfg(windows)]
    job: windows::Win32::Foundation::HANDLE,
}

#[cfg(unix)]
impl Tree {
    fn spawn(mut c: Command) -> Option<(std::process::Child, Tree)> {
        crate::ai::detach(&mut c);
        let child = c.spawn().ok()?;
        let pid = child.id();
        Some((child, Tree { pid }))
    }

    /// The program and all it started, the program reaped.
    fn stop(&self, child: &mut std::process::Child) {
        crate::ai::stop_child(child);
    }

    /// What it started, once it has ended.
    fn stop_rest(&self) {
        crate::ai::stop_group(self.pid);
    }
}

#[cfg(windows)]
impl Tree {
    fn spawn(mut c: Command) -> Option<(std::process::Child, Tree)> {
        use std::os::windows::io::AsRawHandle;
        use std::os::windows::process::CommandExt;
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32};
        use windows::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        use windows::Win32::System::Threading::{OpenThread, ResumeThread, CREATE_SUSPENDED, THREAD_SUSPEND_RESUME};
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // SAFETY: a job of our own, limits set from a struct that lives through the call.
        let tree = unsafe {
            let job = CreateJobObjectW(None, windows::core::PCWSTR::null()).ok()?;
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
            if SetInformationJobObject(job, JobObjectExtendedLimitInformation, (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(), size).is_err() {
                let _ = CloseHandle(job);
                return None;
            }
            Tree { job }
        };
        // Started suspended (windowless, as `no_window` would — this replaces its flags), so it starts nothing
        // before it is in the job.
        let no_console = unsafe { windows::Win32::System::Console::GetConsoleWindow() }.is_invalid();
        c.creation_flags(CREATE_SUSPENDED.0 | if no_console { CREATE_NO_WINDOW } else { 0 });
        let mut child = c.spawn().ok()?;
        let pid = child.id();
        // SAFETY: our child's handle, the snapshot and thread handles closed after use.
        let resumed = unsafe {
            AssignProcessToJobObject(tree.job, HANDLE(child.as_raw_handle())).is_ok() && {
                let mut n = 0;
                if let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) {
                    let mut e = THREADENTRY32 { dwSize: std::mem::size_of::<THREADENTRY32>() as u32, ..Default::default() };
                    let mut more = Thread32First(snap, &mut e).is_ok();
                    while more {
                        if e.th32OwnerProcessID == pid {
                            if let Ok(th) = OpenThread(THREAD_SUSPEND_RESUME, false, e.th32ThreadID) {
                                if ResumeThread(th) != u32::MAX {
                                    n += 1;
                                }
                                let _ = CloseHandle(th);
                            }
                        }
                        more = Thread32Next(snap, &mut e).is_ok();
                    }
                    let _ = CloseHandle(snap);
                }
                n > 0
            }
        };
        if !resumed {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        Some((child, tree))
    }

    /// The program and all it started, the program reaped.
    fn stop(&self, child: &mut std::process::Child) {
        self.stop_rest();
        let _ = child.kill();
        let _ = child.wait();
    }

    /// Everything in the job (its pipe ends with them).
    fn stop_rest(&self) {
        // SAFETY: our own job's handle, open until drop.
        let _ = unsafe { windows::Win32::System::JobObjects::TerminateJobObject(self.job, 1) };
    }
}

/// On Windows the job is closed: anything of it still running is ended (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`).
#[cfg(windows)]
impl Drop for Tree {
    fn drop(&mut self) {
        // SAFETY: our own job's handle, closed once.
        let _ = unsafe { windows::Win32::Foundation::CloseHandle(self.job) };
    }
}

/// Read `r` to its end, at most [`MAX_OUT`]; `None` past `deadline` (on Unix the pipe is read without blocking). It
/// never waits forever: on time-out or when something the program started holds the pipe, the [`Tree`] is stopped,
/// which closes it (on Windows a read then ends as at the end, with what was read).
fn read_until(mut r: std::process::ChildStdout, deadline: Instant) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let fd = r.as_raw_fd();
        // SAFETY: changing the flags of a pipe we own.
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
    }
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match r.read(&mut buf) {
            Ok(0) => break,
            Ok(n) if out.len() + n <= MAX_OUT => out.extend_from_slice(&buf[..n]),
            Ok(_) => return None,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return None,
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A made-up program: a shell script that writes what it was asked (args, stdin, the no-prompt variables, where it
    /// ran, what of the environment it got) to `<name>.log` beside it, then runs `body`.
    fn fake(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        let log = dir.join(format!("{name}.log"));
        std::fs::write(
            &p,
            format!(
                "#!/bin/sh\necho \"args=$* prompt=$GIT_TERMINAL_PROMPT gcm=$GCM_INTERACTIVE askpass=$GIT_ASKPASS pwd=$(pwd) params=$GIT_CONFIG_PARAMETERS count=$GIT_CONFIG_COUNT gitdir=$GIT_DIR ghtoken=$GH_TOKEN ghhost=$GH_HOST\" >> '{}'\ncat >> '{}' 2>/dev/null\n{body}\n",
                log.display(),
                log.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("echo-signin-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    fn nothing(_: &mut Command) {}

    /// Is process `pid` still there (a zombie not counted)?
    fn alive(pid: i32) -> bool {
        let state = Command::new("ps").args(["-o", "stat=", "-p", &pid.to_string()]).output().unwrap();
        let s = String::from_utf8_lossy(&state.stdout);
        !s.trim().is_empty() && !s.trim().starts_with('Z')
    }

    #[test]
    fn the_github_cli_is_asked_first_then_git_and_nobody_is_asked_for_a_password() {
        let d = dir("order");
        let gh = fake(&d, "gh", "echo gho_madeup123");
        let git = fake(&d, "git", "printf 'protocol=https\\nhost=github.com\\nusername=ann\\npassword=ghp_fromgit\\n'");
        // The CLI has it: git isn't asked.
        assert_eq!(get_with("github.com", Some(&gh), Some(&git), &nothing), Some(json!({"token": "gho_madeup123", "from": "gh"})));
        assert!(std::fs::read_to_string(d.join("gh.log")).unwrap().contains("args=auth token --hostname github.com"));
        assert!(!d.join("git.log").exists());
        // The CLI isn't signed in there: git's helper, asked for that host only, with no prompt allowed.
        let gh = fake(&d, "gh", "echo 'no oauth token' >&2; exit 1");
        assert_eq!(get_with("github.com", Some(&gh), Some(&git), &nothing), Some(json!({"token": "ghp_fromgit", "from": "git", "user": "ann"})));
        let asked = std::fs::read_to_string(d.join("git.log")).unwrap();
        assert!(asked.contains("args=-c credential.interactive=false credential fill prompt=0 gcm=never askpass=echo-asks-nobody"), "{asked}");
        assert!(asked.contains("protocol=https\nhost=github.com\n"), "{asked}");
        assert!(!asked.contains("approve") && !asked.contains("erase") && !asked.contains("reject"));
        // No CLI at all: git alone.
        assert_eq!(get_with("github.com", None, Some(&git), &nothing).unwrap()["from"], "git");
        // Neither has one: null.
        let git = fake(&d, "git", "echo 'fatal: could not read Username: terminal prompts disabled' >&2; exit 128");
        assert_eq!(get_with("github.com", Some(&gh), Some(&git), &nothing), None);
        assert_eq!(get_with("github.com", None, None, &nothing), None);
        // An empty answer, or one with a line break or control character in it, isn't a sign-in; a Windows line end
        // and the spaces around the CLI's token are dropped, a space inside git's password kept.
        let gh = fake(&d, "gh", "echo");
        let git = fake(&d, "git", "printf 'username=ann\\npassword=\\n'");
        assert_eq!(get_with("github.com", Some(&gh), Some(&git), &nothing), None);
        let gh = fake(&d, "gh", "printf 'gho_a b\\n'");
        let git = fake(&d, "git", "printf 'password=pa\\033ss\\n'");
        assert_eq!(get_with("github.com", Some(&gh), Some(&git), &nothing), None);
        let gh = fake(&d, "gh", "printf '  gho_madeup9 \\r\\n'");
        assert_eq!(get_with("github.com", Some(&gh), None, &nothing).unwrap()["token"], "gho_madeup9");
        let git = fake(&d, "git", "printf 'username=ann\\r\\npassword=my pass\\r\\n'");
        assert_eq!(get_with("github.com", None, Some(&git), &nothing), Some(json!({"token": "my pass", "from": "git", "user": "ann"})));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn echos_folder_and_environment_never_choose_the_helper_or_the_account() {
        let d = dir("planted");
        // What Echo might have been started with: a repository's folder, its own git config and GitHub CLI account.
        let repo = d.join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let inherit = |c: &mut Command| {
            c.current_dir(&repo)
                .env("GIT_DIR", repo.join(".git"))
                .env("GIT_WORK_TREE", &repo)
                .env("GIT_CONFIG_PARAMETERS", "'credential.helper'='!echo planted'")
                .env("GIT_CONFIG_COUNT", "1")
                .env("GIT_CONFIG_KEY_0", "credential.helper")
                .env("GIT_CONFIG_VALUE_0", "!echo planted")
                .env("GH_TOKEN", "gho_planted")
                .env("GH_HOST", "evil.example")
                .env("GH_CONFIG_DIR", &repo)
                .env("GITHUB_TOKEN", "ghp_planted");
        };
        let gh = fake(&d, "gh", "env > \"$(dirname \"$0\")/gh.env\"; exit 1");
        // It leaves a file behind in the folder it ran from.
        let git = fake(&d, "git", "env > \"$(dirname \"$0\")/git.env\"; touch left-behind; printf 'password=ghp_fromgit\\n'");
        assert_eq!(get_with("github.com", Some(&gh), Some(&git), &inherit).unwrap()["from"], "git");
        for name in ["gh", "git"] {
            let env = std::fs::read_to_string(d.join(format!("{name}.env"))).unwrap();
            for planted in ["GIT_DIR=", "GIT_WORK_TREE=", "GIT_CONFIG_PARAMETERS=", "GIT_CONFIG_COUNT=", "GIT_CONFIG_KEY_0=", "GIT_CONFIG_VALUE_0=", "GH_TOKEN=", "GH_HOST=", "GH_CONFIG_DIR=", "GITHUB_TOKEN="] {
                assert!(!env.lines().any(|l| l.starts_with(planted)), "{name} got {planted}");
            }
            // A folder of Echo's own, right in the temporary folder (not the repository, nor this test's folder).
            let log = std::fs::read_to_string(d.join(format!("{name}.log"))).unwrap();
            let ran_in = Path::new(log.split("pwd=").nth(1).unwrap().split(' ').next().unwrap());
            assert_eq!(ran_in.parent(), Some(std::env::temp_dir().canonicalize().unwrap().as_path()), "{name} ran from {log}");
            let folder = ran_in.file_name().unwrap().to_string_lossy();
            assert!(folder.starts_with(&format!("echo-signin-{}-", std::process::id())) && !folder.starts_with("echo-signin-test-"), "{name} ran from {log}");
            // It is gone after, with what was left in it.
            assert!(!ran_in.exists(), "{}", ran_in.display());
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The real git, with a made-up home and no system config, asked for a made-up host: a helper planted in the
    /// repository Echo was started in, or in an inherited `GIT_CONFIG_PARAMETERS`, never runs.
    #[test]
    fn a_helper_planted_in_a_repository_or_the_environment_never_runs() {
        let Some(real) = pick(crate::app::program_candidates("git")) else { return };
        if !Command::new(&real).arg("--version").env("GIT_CONFIG_NOSYSTEM", "1").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success()) {
            return;
        }
        let d = dir("real-git");
        let home = d.join("home");
        let no_helpers = d.join("exec-path");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&no_helpers).unwrap();
        // The real git, kept from this computer's own settings and sign-ins: a made-up home, no global or system
        // config, and an empty exec path, so no `git-credential-*` helper of its own (the keychain's) can be found;
        // everything Echo sets passes through.
        let git = d.join("git");
        std::fs::write(&git, format!("#!/bin/sh\nexport HOME='{}' GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_EXEC_PATH='{}'\nexec '{}' \"$@\"\n", home.display(), no_helpers.display(), real.display())).unwrap();
        std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
        let repo = d.join("repo");
        assert!(Command::new(&git).args(["init", "-q"]).arg(&repo).stdout(Stdio::null()).status().unwrap().success());
        let marker = d.join("planted-ran");
        // A whole answer (a username too): git asks no askpass for the rest.
        let helper = format!("!f() {{ touch '{}'; echo username=x; echo password=planted; }}; f", marker.display());
        assert!(Command::new(&git).arg("-C").arg(&repo).args(["config", "credential.helper", &helper]).status().unwrap().success());
        let env_helper = format!("'credential.helper'='!f() {{ touch {}; echo username=x; echo password=planted; }}; f'", marker.display());
        let inherit = |c: &mut Command| {
            c.current_dir(&repo).env("GIT_CONFIG_PARAMETERS", &env_helper).env("GIT_CONFIG_COUNT", "1").env("GIT_CONFIG_KEY_0", "credential.helper").env("GIT_CONFIG_VALUE_0", &helper);
        };
        let got = get_with("signin-test.invalid", None, Some(&git), &inherit);
        assert!(!marker.exists(), "a planted helper ran");
        assert!(got.is_none(), "a sign-in was handed over");
        // The same planted repository chooses the helper when Echo doesn't clear it (the test can tell) — still asking
        // nobody (no askpass of the terminal the tests run in, no prompt) and given up on after 10 s.
        let mut c = Command::new(&git);
        c.arg("-C").arg(&repo).args(["credential", "fill"]);
        for (k, _) in std::env::vars_os() {
            let up = k.to_string_lossy().to_ascii_uppercase();
            if up.starts_with("GIT_") || up.starts_with("GCM_") || up == "SSH_ASKPASS" {
                c.env_remove(&k);
            }
        }
        c.env("GIT_TERMINAL_PROMPT", "0").env("GIT_ASKPASS", "echo-asks-nobody").env("SSH_ASKPASS", "echo-asks-nobody").env("GCM_INTERACTIVE", "never");
        let _ = run(c, Some("protocol=https\nhost=signin-test.invalid\n\n"));
        assert!(marker.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_program_that_hangs_is_given_up_on_with_what_it_started() {
        let d = dir("hang");
        // It starts a `sleep` (as a helper would) and waits on it.
        let gh = fake(&d, "gh", "sleep 30 & echo $! > \"$(dirname \"$0\")/sleep.pid\"; wait");
        let git = fake(&d, "git", "printf 'password=ghp_fromgit\\n'");
        let t = Instant::now();
        assert_eq!(get_with("github.com", Some(&gh), Some(&git), &nothing).unwrap()["from"], "git");
        assert!(t.elapsed() < LIMIT + Duration::from_secs(5), "{:?}", t.elapsed());
        let pid: i32 = std::fs::read_to_string(d.join("sleep.pid")).unwrap().trim().parse().unwrap();
        assert!(!alive(pid), "its sleep {pid} is still running");
        // It answers and ends but leaves a `sleep` holding its output: that is stopped, the answer taken.
        let gh = fake(&d, "gh", "echo gho_madeup5; sleep 30 & echo $! > \"$(dirname \"$0\")/sleep.pid\"; exit 0");
        let t = Instant::now();
        assert_eq!(get_with("github.com", Some(&gh), None, &nothing), Some(json!({"token": "gho_madeup5", "from": "gh"})));
        assert!(t.elapsed() < Duration::from_secs(8), "{:?}", t.elapsed());
        let pid: i32 = std::fs::read_to_string(d.join("sleep.pid")).unwrap().trim().parse().unwrap();
        assert!(!alive(pid), "its sleep {pid} is still running");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_host_is_looked_up_once_at_a_time_and_kept_a_minute() {
        let n = std::sync::atomic::AtomicUsize::new(0);
        let look = || {
            n.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(100));
            Some(json!({"token": "gho_madeup", "from": "gh"}))
        };
        std::thread::scope(|s| {
            for _ in 0..4 {
                s.spawn(|| assert_eq!(cached("cache-test.example", false, (KEEP, KEEP_NONE), look).unwrap()["token"], "gho_madeup"));
            }
        });
        assert_eq!(n.load(std::sync::atomic::Ordering::SeqCst), 1);
        // Another host is its own.
        assert!(cached("cache-test-2.example", false, (KEEP, KEEP_NONE), || None).is_none());
        assert_eq!(cached("cache-test-2.example", false, (KEEP, KEEP_NONE), || Some(json!(1))), None);
        // `fresh` asks again whatever is kept, and its answer is the one kept after.
        assert_eq!(cached("cache-test.example", true, (KEEP, KEEP_NONE), || Some(json!({"token": "gho_new"}))).unwrap()["token"], "gho_new");
        assert_eq!(cached("cache-test.example", false, (KEEP, KEEP_NONE), || panic!("asked again")).unwrap()["token"], "gho_new");
        assert_eq!(cached("cache-test-2.example", true, (KEEP, KEEP_NONE), || Some(json!(2))), Some(json!(2)));
    }

    #[test]
    fn a_sign_in_kept_is_dropped_from_memory_when_its_time_is_up() {
        let short = (Duration::from_millis(150), Duration::from_millis(150));
        let held = |host: &str| slots().lock().unwrap().get(host).map(|s| s.lock().unwrap().clone());
        assert_eq!(cached("cache-drop.example", false, short, || Some(json!({"token": "gho_madeup"}))).unwrap()["token"], "gho_madeup");
        assert!(held("cache-drop.example").flatten().is_some_and(|(_, v)| v.is_some()));
        // A newer answer isn't dropped by the older one's time.
        std::thread::sleep(Duration::from_millis(100));
        cached("cache-drop.example", true, short, || Some(json!({"token": "gho_newer"})));
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(held("cache-drop.example").flatten().and_then(|(_, v)| v).unwrap()["token"], "gho_newer");
        // Its own time up: nothing of it is held.
        let t = Instant::now();
        while held("cache-drop.example").is_some() && t.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(held("cache-drop.example").is_none(), "still held");
        assert_eq!(cached("cache-drop.example", false, short, || None), None);
    }

    #[test]
    fn only_a_program_by_its_full_path_is_used() {
        let d = dir("pick");
        let gh = fake(&d, "gh", "true");
        // `gh` relative (an empty PATH entry) is skipped even where it exists.
        let rel = PathBuf::from("Cargo.toml");
        assert!(rel.is_file());
        assert_eq!(pick(vec![rel, d.join("nope"), gh.clone()]), Some(gh));
        assert_eq!(pick(vec![PathBuf::from("gh")]), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
