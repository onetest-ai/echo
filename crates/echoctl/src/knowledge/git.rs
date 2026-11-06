//! git, as little as Echo needs and nothing more (spec X-G): what a vault's repository is like, and — only where the
//! vault allows it — one commit of exactly the files Echo wrote. Never push, pull, fetch, checkout, switch, branch,
//! stash, reset, rebase, merge, amend or `--no-verify`. Every call has a timeout; git is optional.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::io::Read;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use crate::app::NoWindow as _;

const TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitInfo {
    /// The repository's top folder (the vault may sit inside a larger repository).
    pub top: PathBuf,
    /// The branch, or `None` on a detached HEAD.
    pub branch: Option<String>,
    pub remotes: Vec<String>,
    /// Files changed and not committed (anyone's).
    pub changed: usize,
    /// A merge, rebase or cherry-pick in progress, in words.
    pub busy: Option<String>,
}

/// The git program, or `None` when there is none (or a test said so).
pub fn program() -> Option<String> {
    #[cfg(any(test, debug_assertions))]
    if std::env::var_os("ECHOCTL_NO_GIT").is_some() {
        return None;
    }
    let p = crate::app::find_program("git");
    let found = Path::new(&p).is_absolute() && Path::new(&p).is_file();
    found.then_some(p)
}

/// `git -C <dir> <args>`, within the timeout; `None` when git is missing, fails to start or takes too long.
pub fn run(dir: &Path, args: &[&str]) -> Option<Output> {
    let git = program()?;
    let mut c = Command::new(git).no_window();
    c.arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        // `[x].md` is a file's name, never a pattern that could take in the user's other files.
        .env("GIT_LITERAL_PATHSPECS", "1");
    run_cmd(c, TIMEOUT)
}

/// Run `cmd` to its end within `timeout`, reading both streams as they come (a full pipe would stall it). Too slow:
/// SIGTERM first — git then removes its lock files — SIGKILL a second later, and `None`.
fn run_cmd(mut cmd: Command, timeout: Duration) -> Option<Output> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    fn drain<R: Read + Send + 'static>(r: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut b = Vec::new();
            if let Some(mut r) = r {
                let _ = r.read_to_end(&mut b);
            }
            b
        })
    }
    let (out, err) = (drain(child.stdout.take()), drain(child.stderr.take()));
    let t = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(Output { status, stdout: out.join().unwrap_or_default(), stderr: err.join().unwrap_or_default() }),
            Ok(None) if t.elapsed() < timeout => std::thread::sleep(Duration::from_millis(10)),
            // The readers aren't waited for: a hook's leftover child may still hold the pipes.
            _ => {
                stop(&mut child);
                return None;
            }
        }
    }
}

fn stop(child: &mut Child) {
    #[cfg(unix)]
    {
        // SAFETY: signalling our own child by its pid.
        unsafe {
            libc::kill(child.id() as i32, libc::SIGTERM);
        }
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(1) {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// The trimmed standard output of a git command that succeeded.
pub fn out(dir: &Path, args: &[&str]) -> Option<String> {
    run(dir, args).filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim_end().to_string())
}

/// What the repository holding `root` is like; `None` when it isn't in one (or there is no git).
pub fn info(root: &Path) -> Option<GitInfo> {
    if !root.is_dir() {
        return None;
    }
    let top = PathBuf::from(out(root, &["rev-parse", "--show-toplevel"])?);
    let branch = out(root, &["symbolic-ref", "--short", "-q", "HEAD"]).filter(|b| !b.is_empty());
    let remotes = out(root, &["remote"]).map(|s| s.lines().map(str::to_owned).filter(|l| !l.is_empty()).collect()).unwrap_or_default();
    let changed = out(root, &["status", "--porcelain"]).map(|s| s.lines().filter(|l| !l.trim().is_empty()).count()).unwrap_or(0);
    let busy = out(root, &["rev-parse", "--absolute-git-dir"]).and_then(|d| busy_in(Path::new(&d)));
    Some(GitInfo { top, branch, remotes, changed, busy })
}

/// An operation git is in the middle of, in words.
fn busy_in(git_dir: &Path) -> Option<String> {
    [
        ("MERGE_HEAD", "a merge is in progress"),
        ("rebase-merge", "a rebase is in progress"),
        ("rebase-apply", "a rebase is in progress"),
        ("CHERRY_PICK_HEAD", "a cherry-pick is in progress"),
        ("REVERT_HEAD", "a revert is in progress"),
    ]
    .iter()
    .find(|(f, _)| git_dir.join(f).exists())
    .map(|(_, w)| w.to_string())
}

/// What came of a commit Echo meant to make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Commit {
    /// The new commit's short id.
    Done(String),
    /// Why it wasn't made (the files are written all the same).
    Skipped(String),
    /// The vault's "commit what Echo writes" is off.
    Off,
}

impl Commit {
    /// As recorded: the id, `skipped: <why>` or `off`.
    pub fn label(&self) -> String {
        match self {
            Commit::Done(id) => id.clone(),
            Commit::Skipped(why) => format!("skipped: {why}"),
            Commit::Off => "off".into(),
        }
    }
}

/// A name and an address git will sign commits with (its config, or git's own environment).
pub fn identity(dir: &Path) -> bool {
    let env = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    let name = env("GIT_AUTHOR_NAME") || out(dir, &["config", "user.name"]).is_some_and(|n| !n.is_empty());
    let mail = env("GIT_AUTHOR_EMAIL") || out(dir, &["config", "user.email"]).is_some_and(|m| !m.is_empty());
    name && mail
}

/// A new repository on `main`.
pub fn init(dir: &Path) -> anyhow::Result<()> {
    if run(dir, &["init", "-q", "-b", "main"]).is_some_and(|o| o.status.success()) {
        return Ok(());
    }
    // git before 2.28 has no `-b`.
    let ok = run(dir, &["init", "-q"]).is_some_and(|o| o.status.success()) && run(dir, &["symbolic-ref", "HEAD", "refs/heads/main"]).is_some_and(|o| o.status.success());
    if ok { Ok(()) } else { anyhow::bail!("git couldn't make a repository in {}", dir.display()) }
}

/// The first commit of a repository Echo just made: everything in it.
pub fn first_commit(dir: &Path, message: &str) -> Commit {
    if !identity(dir) {
        return Commit::Skipped("git has no user name and email set".into());
    }
    let added = run(dir, &["add", "-A"]).is_some_and(|o| o.status.success());
    match run(dir, &["commit", "-q", "-m", message]) {
        Some(o) if added && o.status.success() => Commit::Done(out(dir, &["rev-parse", "--short", "HEAD"]).unwrap_or_default()),
        Some(o) => Commit::Skipped(first_line(&o.stderr).unwrap_or_else(|| "git refused the commit".into())),
        None => Commit::Skipped("git didn't answer".into()),
    }
}

fn first_line(b: &[u8]) -> Option<String> {
    String::from_utf8_lossy(b).lines().map(str::trim).find(|l| !l.is_empty()).map(|l| l.chars().take(200).collect())
}

fn arg(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Which of `paths` have uncommitted changes (staged, unstaged or untracked) — before Echo writes them. A path git
/// can't answer for (not a repository, a broken one, too slow, no git) counts as changed: Echo never commits a file git
/// didn't say was clean (ruling R30).
pub fn dirty(root: &Path, paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter(|p| match out(root, &["status", "--porcelain", "--", &arg(p)]) {
            Some(s) => p.exists() && !s.trim().is_empty(),
            None => true,
        })
        .cloned()
        .collect()
}

/// Why a commit can't be made in the repository holding `root` now, if it can't.
pub fn blocked(root: &Path) -> Option<String> {
    let Some(i) = info(root) else { return Some(if program().is_none() { "git isn't installed".into() } else { "the vault isn't in a git repository".into() }) };
    if let Some(b) = i.busy {
        return Some(b);
    }
    if i.branch.is_none() {
        return Some("HEAD is detached".into());
    }
    if out(root, &["rev-parse", "--absolute-git-dir"]).is_some_and(|d| Path::new(&d).join("index.lock").exists()) {
        return Some("git is busy (index.lock)".into());
    }
    if !identity(root) {
        return Some("git has no user name and email set".into());
    }
    None
}

/// One commit of exactly `paths` (files Echo just wrote, all of them clean before), the user's other staged and
/// unstaged work left as it is. Hooks run as usual; a failing one leaves the files written and says why.
pub fn commit_paths(root: &Path, paths: &[PathBuf], message: &str) -> Commit {
    if paths.is_empty() {
        return Commit::Skipped("nothing to commit".into());
    }
    if let Some(why) = blocked(root) {
        return Commit::Skipped(why);
    }
    // New files have to be known to git for `--only`; on a refusal they are taken out of the index again.
    let new: Vec<String> = paths.iter().map(|p| arg(p)).filter(|a| out(root, &["ls-files", "--error-unmatch", "--", a]).is_none()).collect();
    for a in &new {
        if !run(root, &["add", "--", a]).is_some_and(|o| o.status.success()) {
            return Commit::Skipped(format!("git couldn't add {a}"));
        }
    }
    let mut args: Vec<String> = ["commit", "--only", "-q", "-m", message, "--"].iter().map(|s| s.to_string()).collect();
    args.extend(paths.iter().map(|p| arg(p)));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let r = run(root, &refs);
    match r {
        Some(o) if o.status.success() => Commit::Done(out(root, &["rev-parse", "--short", "HEAD"]).unwrap_or_default()),
        other => {
            for a in &new {
                let _ = run(root, &["rm", "--cached", "-q", "--ignore-unmatch", "--", a]);
            }
            Commit::Skipped(other.and_then(|o| first_line(&o.stderr).or_else(|| first_line(&o.stdout))).unwrap_or_else(|| "git didn't answer".into()))
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// git reads only a test config (an identity, no signing) while the guard lives — never the user's own.
    pub struct Isolated(PathBuf);
    impl Drop for Isolated {
        fn drop(&mut self) {
            std::env::remove_var("GIT_CONFIG_GLOBAL");
            std::env::remove_var("GIT_CONFIG_NOSYSTEM");
            let _ = std::fs::remove_file(&self.0);
        }
    }
    pub fn isolated(root: &Path, with_identity: bool) -> Isolated {
        let f = root.join("test.gitconfig");
        let id = if with_identity { "[user]\n\tname = Test User\n\temail = test@example.com\n" } else { "" };
        std::fs::write(&f, format!("{id}[commit]\n\tgpgsign = false\n[init]\n\tdefaultBranch = main\n")).unwrap();
        std::env::set_var("GIT_CONFIG_GLOBAL", &f);
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
        Isolated(f)
    }

    /// A repository with one commit, made with a test identity.
    pub fn repo(dir: &Path) -> bool {
        if program().is_none() {
            return false;
        }
        std::fs::create_dir_all(dir).unwrap();
        for args in [vec!["init", "-q", "-b", "main"], vec!["config", "user.name", "Test User"], vec!["config", "user.email", "test@example.com"], vec!["config", "commit.gpgsign", "false"]] {
            assert!(run(dir, &args).unwrap().status.success(), "{args:?}");
        }
        std::fs::write(dir.join("first.md"), "# first\n").unwrap();
        assert!(run(dir, &["add", "-A"]).unwrap().status.success());
        assert!(run(dir, &["commit", "-q", "-m", "first"]).unwrap().status.success());
        true
    }

    #[test]
    fn git_info_reads_branch_changes_and_busy() {
        let (_env, root) = crate::paths::testenv::folders("kn-gitinfo");
        let r = root.join("r");
        if !repo(&r) {
            return;
        }
        std::fs::create_dir_all(r.join("sub")).unwrap();
        std::fs::write(r.join("first.md"), "# changed\n").unwrap();
        std::fs::write(r.join("sub/new.md"), "# new\n").unwrap();
        let i = info(&r.join("sub")).unwrap();
        assert_eq!(dunce::canonicalize(&i.top).unwrap(), dunce::canonicalize(&r).unwrap());
        assert_eq!(i.branch.as_deref(), Some("main"));
        assert_eq!(i.changed, 2);
        assert_eq!(i.busy, None);
        std::fs::write(r.join(".git/MERGE_HEAD"), "0000000000000000000000000000000000000000\n").unwrap();
        assert_eq!(info(&r).unwrap().busy.as_deref(), Some("a merge is in progress"));
        assert_eq!(info(&root.join("not-a-repo-but-a-folder")), None);
    }

    #[test]
    fn a_large_output_is_read_not_waited_on() {
        let mut c = Command::new("sh");
        c.args(["-c", "head -c 200000 /dev/zero | tr '\\0' a; head -c 100000 /dev/zero | tr '\\0' b >&2"]);
        let t = Instant::now();
        let o = run_cmd(c, Duration::from_secs(5)).expect("it finishes");
        assert_eq!((o.stdout.len(), o.stderr.len()), (200_000, 100_000), "more than a pipe holds, both streams");
        assert!(t.elapsed() < Duration::from_secs(4), "not killed at the timeout: {:?}", t.elapsed());
    }

    #[cfg(unix)]
    #[test]
    fn a_timeout_asks_git_to_stop_first() {
        let (_env, root) = crate::paths::testenv::folders("kn-git-term");
        let f = root.join("got-term");
        let mut c = Command::new("sh");
        c.args(["-c", &format!("trap 'touch \"{}\"; exit 1' TERM; sleep 5 & wait", f.display())]);
        assert!(run_cmd(c, Duration::from_millis(400)).is_none(), "too slow: no answer");
        assert!(f.exists(), "SIGTERM first, so git can remove its index.lock");
    }

    #[test]
    fn pathspecs_are_literal_so_a_commit_takes_only_its_files() {
        let (_env, root) = crate::paths::testenv::folders("kn-git-literal");
        let r = root.join("r");
        let _iso = isolated(&root, true);
        if !repo(&r) {
            return;
        }
        std::fs::write(r.join("x.md"), "# yours\n").unwrap();
        assert!(run(&r, &["add", "x.md"]).unwrap().status.success());
        assert!(run(&r, &["commit", "-q", "-m", "x"]).unwrap().status.success());
        std::fs::write(r.join("x.md"), "# yours, changed and not committed\n").unwrap();
        std::fs::write(r.join("[x].md"), "# Echo's\n").unwrap();
        assert!(matches!(commit_paths(&r, &[r.join("[x].md")], "Echo: a page"), Commit::Done(_)));
        let status = out(&r, &["status", "--porcelain"]).unwrap();
        assert!(status.contains("x.md") && !status.contains("[x].md"), "the user's x.md is still uncommitted: {status}");
        assert_eq!(dirty(&r, &[r.join("[x].md")]), Vec::<PathBuf>::new(), "a [x] name is itself, not a pattern");
    }

    #[test]
    fn no_git_means_no_info() {
        let (_env, root) = crate::paths::testenv::folders("kn-nogit");
        std::env::set_var("ECHOCTL_NO_GIT", "1");
        let r = info(&root);
        std::env::remove_var("ECHOCTL_NO_GIT");
        assert_eq!(r, None);
    }

    #[test]
    fn a_status_git_cant_answer_counts_as_changed() {
        let (_env, root) = crate::paths::testenv::folders("kn-git-unanswered");
        let _iso = isolated(&root, true);
        // Ruling R30: when git can't say a file is clean, Echo never commits it.
        let r = root.join("broken");
        std::fs::create_dir_all(&r).unwrap();
        std::fs::write(r.join("page.md"), "# A page\n").unwrap();
        std::fs::write(r.join(".git"), format!("gitdir: {}\n", root.join("no-such-repository").display())).unwrap();
        let asked = vec![r.join("page.md"), r.join("new.md")];
        if program().is_some() {
            assert_eq!(dirty(&r, &asked), asked, "a broken repository: every asked path");
        }
        std::env::set_var("ECHOCTL_NO_GIT", "1");
        let none = dirty(&r, &asked);
        std::env::remove_var("ECHOCTL_NO_GIT");
        assert_eq!(none, asked, "no git: every asked path");
        // A repository that answers: a clean file and a new one are nobody's changes.
        let good = root.join("good");
        if !repo(&good) {
            return;
        }
        assert!(dirty(&good, &[good.join("first.md"), good.join("new.md")]).is_empty());
    }
}
