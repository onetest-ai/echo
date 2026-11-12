//! `ctx.signin`: the user's own sign-in to a host on this computer, handed to a plugin's host half that named the host
//! in `uses.signin.hosts` (`plugins::may_use_signin`). A plugin can't run programs, and the GitHub CLI keeps its token
//! in the system's keychain, so Echo asks for it: the GitHub CLI first (`gh auth token`), then git's credential helper
//! (`git credential fill`: the keychain, Git Credential Manager). Each asked without a window or a prompt, 10 s at most.
//! The token is never written or logged by Echo; it goes to the host half only.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::app::NoWindow;

/// How long one program is given to answer.
const LIMIT: Duration = Duration::from_secs(10);

/// The user's sign-in to `host`: `{token, from: "gh" | "git", user?}`, or `None` when neither program has one.
pub fn get(host: &str) -> Option<Value> {
    get_with(host, program("gh").as_deref(), program("git").as_deref())
}

/// Which of the programs that could hold a sign-in are on this computer (no sign-in read): `{gh, git}`.
pub fn sources() -> Value {
    json!({"gh": program("gh").is_some(), "git": program("git").is_some()})
}

/// [`get`] with the programs given (tests: made-up ones).
pub(crate) fn get_with(host: &str, gh: Option<&Path>, git: Option<&Path>) -> Option<Value> {
    if let Some(gh) = gh {
        let mut c = Command::new(gh).no_window();
        c.args(["auth", "token", "--hostname", host]).env("GH_PROMPT_DISABLED", "1").env("GH_NO_UPDATE_NOTIFIER", "1");
        if let Some((true, out)) = run(c, None) {
            let token = out.trim();
            if !token.is_empty() && !token.contains(char::is_whitespace) {
                return Some(json!({"token": token, "from": "gh"}));
            }
        }
    }
    let git = git?;
    let mut c = Command::new(git).no_window();
    // Nobody is asked: no terminal prompt, no askpass program (one that can't start), no Credential Manager window.
    c.args(["credential", "fill"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "echo-asks-nobody")
        .env("SSH_ASKPASS", "echo-asks-nobody")
        .env("GCM_INTERACTIVE", "never")
        .env("GCM_GUI_PROMPT", "false");
    let (ok, out) = run(c, Some(&format!("protocol=https\nhost={host}\n\n")))?;
    if !ok {
        return None;
    }
    let field = |k: &str| out.lines().find_map(|l| l.strip_prefix(k)?.strip_prefix('=')).filter(|v| !v.is_empty());
    let token = field("password")?;
    let mut v = json!({"token": token, "from": "git"});
    if let Some(user) = field("username") {
        v["user"] = json!(user);
    }
    Some(v)
}

/// A program on this computer that can be run without a window: as `system.which` finds it, plus where the GitHub CLI
/// and Git for Windows install themselves; on macOS not the `/usr/bin/git` stand-in that would offer to install
/// Apple's developer tools.
fn program(name: &str) -> Option<PathBuf> {
    let mut found = vec![PathBuf::from(crate::app::find_program(name))];
    if cfg!(windows) {
        for k in ["ProgramFiles", "LOCALAPPDATA"] {
            if let Some(d) = std::env::var_os(k) {
                let d = PathBuf::from(d);
                found.push(if name == "gh" { d.join("GitHub CLI/gh.exe") } else { d.join(format!("Programs/Git/cmd/{name}.exe")) });
                found.push(d.join(format!("Git/cmd/{name}.exe")));
            }
        }
    }
    found.into_iter().find(|p| p.is_file() && usable(p))
}

fn usable(p: &Path) -> bool {
    if cfg!(target_os = "macos") && p == Path::new("/usr/bin/git") {
        return ["/Library/Developer/CommandLineTools/usr/bin/git", "/Applications/Xcode.app/Contents/Developer/usr/bin/git"].iter().any(|g| Path::new(g).is_file());
    }
    true
}

/// Run `c` with `input` on its stdin: (exited 0, its stdout), or `None` when it can't start or takes over [`LIMIT`]
/// (then it is stopped). Its stderr is dropped: nothing it says is logged.
fn run(mut c: Command, input: Option<&str>) -> Option<(bool, String)> {
    c.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = c.spawn().ok()?;
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        let _ = stdin.write_all(text.as_bytes());
    }
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        let _ = tx.send(out);
    });
    let t = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if t.elapsed() < LIMIT => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    // A helper it started may still hold its output open: waited for within the same limit.
    let out = rx.recv_timeout(LIMIT.saturating_sub(t.elapsed()).max(Duration::from_millis(500))).ok()?;
    Some((status.success(), out))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A made-up program: a shell script that writes what it was asked (args, stdin, the no-prompt variables) to
    /// `<name>.log` beside it, then runs `body`.
    fn fake(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        let log = dir.join(format!("{name}.log"));
        std::fs::write(&p, format!("#!/bin/sh\necho \"args=$* prompt=$GIT_TERMINAL_PROMPT gcm=$GCM_INTERACTIVE askpass=$GIT_ASKPASS\" >> '{}'\ncat >> '{}' 2>/dev/null\n{body}\n", log.display(), log.display())).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("echo-signin-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_github_cli_is_asked_first_then_git_and_nothing_prompts() {
        let d = dir("order");
        let gh = fake(&d, "gh", "echo gho_madeup123");
        let git = fake(&d, "git", "printf 'protocol=https\\nhost=github.com\\nusername=ann\\npassword=ghp_fromgit\\n'");
        // The CLI has it: git isn't asked.
        assert_eq!(get_with("github.com", Some(&gh), Some(&git)), Some(json!({"token": "gho_madeup123", "from": "gh"})));
        assert!(std::fs::read_to_string(d.join("gh.log")).unwrap().contains("args=auth token --hostname github.com"));
        assert!(!d.join("git.log").exists());
        // The CLI isn't signed in there: git's helper, asked for that host only, with no prompt allowed.
        let gh = fake(&d, "gh", "echo 'no oauth token' >&2; exit 1");
        assert_eq!(get_with("github.com", Some(&gh), Some(&git)), Some(json!({"token": "ghp_fromgit", "from": "git", "user": "ann"})));
        let asked = std::fs::read_to_string(d.join("git.log")).unwrap();
        assert!(asked.contains("args=credential fill prompt=0 gcm=never askpass=echo-asks-nobody"), "{asked}");
        assert!(asked.contains("protocol=https\nhost=github.com\n"), "{asked}");
        assert!(!asked.contains("approve") && !asked.contains("erase") && !asked.contains("reject"));
        // No CLI at all: git alone.
        assert_eq!(get_with("github.com", None, Some(&git)).unwrap()["from"], "git");
        // Neither has one: null.
        let git = fake(&d, "git", "echo 'fatal: could not read Username: terminal prompts disabled' >&2; exit 128");
        assert_eq!(get_with("github.com", Some(&gh), Some(&git)), None);
        assert_eq!(get_with("github.com", None, None), None);
        // An empty answer isn't a sign-in.
        let gh = fake(&d, "gh", "echo");
        let git = fake(&d, "git", "printf 'username=ann\\npassword=\\n'");
        assert_eq!(get_with("github.com", Some(&gh), Some(&git)), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_program_that_hangs_is_given_up_on() {
        let d = dir("hang");
        let gh = fake(&d, "gh", "sleep 30");
        let git = fake(&d, "git", "printf 'password=ghp_fromgit\\n'");
        let t = Instant::now();
        assert_eq!(get_with("github.com", Some(&gh), Some(&git)).unwrap()["from"], "git");
        assert!(t.elapsed() < LIMIT + Duration::from_secs(5), "{:?}", t.elapsed());
        let _ = std::fs::remove_dir_all(&d);
    }
}
