//! Where plugins come from: marketplace sources (`owner/repo[@ref]`, a git address, a local folder), fetching them,
//! and checking, hashing and copying plugin trees.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use sha2::{Digest, Sha256};

use super::Limit;
use crate::app::NoWindow;
use crate::paths;

#[derive(Debug, PartialEq)]
pub(super) enum Source {
    Github { repo: String, git_ref: Option<String> },
    Git { url: String, git_ref: Option<String> },
    Local(PathBuf),
}

pub(super) fn plain_ref(r: &str) -> bool {
    !r.is_empty() && r.len() <= 100 && !r.starts_with('-') && r.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b)) && !r.contains("..")
}

pub(super) fn parse_source(s: &str) -> Result<Source> {
    let s = s.trim();
    if s.starts_with("https://") || s.starts_with("ssh://") || s.starts_with("git@") {
        let (url, r) = match s.rsplit_once('#') {
            Some((u, r)) => (u, Some(r)),
            None => (s, None),
        };
        if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
            bail!("“{s}” isn't a git address");
        }
        if let Some(r) = r.filter(|r| !plain_ref(r)) {
            bail!("“{r}” isn't a plain branch, tag or commit");
        }
        return Ok(Source::Git { url: url.into(), git_ref: r.map(str::to_string) });
    }
    let local = s.starts_with('/') || s.starts_with('.') || s.starts_with('~') || Path::new(s).is_absolute();
    if local {
        let p = if let Some(rest) = s.strip_prefix("~/") { paths::user_home().join(rest) } else { PathBuf::from(s) };
        let p = dunce::canonicalize(&p).with_context(|| format!("no folder {s}"))?;
        if !p.is_dir() {
            bail!("{s} isn't a folder");
        }
        return Ok(Source::Local(p));
    }
    let (repo, r) = match s.split_once('@') {
        Some((repo, r)) => (repo, Some(r)),
        None => (s, None),
    };
    let ok = |p: &str| !p.is_empty() && p.len() <= 100 && !p.starts_with('.') && p.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
    match repo.split_once('/') {
        Some((o, n)) if ok(o) && ok(n) => {}
        _ => bail!("“{s}” isn't owner/repo, a git address (https://…) or a folder"),
    }
    if let Some(r) = r.filter(|r| !plain_ref(r)) {
        bail!("“{r}” isn't a plain branch, tag or commit");
    }
    Ok(Source::Github { repo: repo.into(), git_ref: r.map(str::to_string) })
}

/// A fetched tree: where its files are, and the commit they are (`local-<hash>` for a folder).
pub(super) struct Snapshot {
    pub(super) dir: PathBuf,
    pub(super) commit: String,
}

pub(super) fn hex40(s: &str) -> Option<String> {
    let s = s.trim();
    (s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())).then(|| s.to_ascii_lowercase())
}

pub(super) fn run_out(c: &mut Command) -> Result<Vec<u8>> {
    let out = c.output().with_context(|| format!("{:?} can't start", c.get_program()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("{}", err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("failed").trim());
    }
    Ok(out.stdout)
}

/// GitHub: the commit a ref (default: the default branch) points to.
pub(super) fn github_commit(repo: &str, git_ref: Option<&str>) -> Result<String> {
    let r = git_ref.unwrap_or("HEAD");
    let api = format!("https://api.github.com/repos/{repo}/commits/{r}");
    let curl = run_out(Command::new("curl").no_window().args(["-fsSL", "-H", "Accept: application/vnd.github.sha", &api]));
    if let Some(sha) = curl.as_ref().ok().and_then(|o| hex40(&String::from_utf8_lossy(o))) {
        return Ok(sha);
    }
    // A private repository: the user's own `gh` sign-in.
    let gh = crate::app::find_program("gh");
    let out = run_out(Command::new(&gh).no_window().args(["api", "-H", "Accept: application/vnd.github.sha", &format!("repos/{repo}/commits/{r}")]))
        .map_err(|e| anyhow!("can't read {repo} on GitHub ({}; gh: {e})", curl.err().map(|e| e.to_string()).unwrap_or_else(|| "no commit".into())))?;
    hex40(&String::from_utf8_lossy(&out)).ok_or_else(|| anyhow!("GitHub gave no commit for {repo}@{r}"))
}

/// Unpack a GitHub commit's tarball into `dest` (empty, created).
pub(super) fn github_unpack(repo: &str, sha: &str, dest: &Path, work: &Path) -> Result<()> {
    let tgz = work.join("tree.tar.gz");
    let url = format!("https://codeload.github.com/{repo}/tar.gz/{sha}");
    let curl = Command::new("curl").no_window().args(["-fsSL", "--max-filesize", "200000000", "-o"]).arg(&tgz).arg(&url).status();
    if !curl.is_ok_and(|s| s.success()) {
        let gh = crate::app::find_program("gh");
        let bytes = run_out(Command::new(&gh).no_window().args(["api", &format!("repos/{repo}/tarball/{sha}")])).with_context(|| format!("downloading {repo}"))?;
        fs::write(&tgz, bytes)?;
    }
    fs::create_dir_all(dest)?;
    // bsdtar (macOS, Windows 10+) and GNU tar both drop absolute paths and refuse `..`; links are
    // refused after, by `check_tree`.
    run_out(Command::new("tar").no_window().arg("-xzf").arg(&tgz).arg("-C").arg(dest).arg("--strip-components").arg("1")).context("unpacking")?;
    let _ = fs::remove_file(&tgz);
    Ok(())
}

pub(super) fn git_fetch(url: &str, git_ref: Option<&str>, dest: &Path) -> Result<String> {
    let git = crate::app::find_program("git");
    let mut c = Command::new(&git).no_window();
    c.args(["clone", "--quiet", "--depth", "1", "--no-tags", "-c", "core.symlinks=false"]);
    if let Some(r) = git_ref {
        c.args(["--branch", r]);
    }
    c.arg("--").arg(url).arg(dest).env("GIT_TERMINAL_PROMPT", "0");
    let out = c.output().map_err(|_| anyhow!(git_failure(url, git_ref, None)))?;
    if !out.status.success() {
        bail!("{}", git_failure(url, git_ref, Some(&String::from_utf8_lossy(&out.stderr))));
    }
    let sha = run_out(Command::new(&git).no_window().arg("-C").arg(dest).args(["rev-parse", "HEAD"]))?;
    let sha = hex40(&String::from_utf8_lossy(&sha)).ok_or_else(|| anyhow!("git gave no commit"))?;
    fs::remove_dir_all(dest.join(".git"))?;
    Ok(sha)
}

/// What a failed `git clone` of `url` (at `git_ref`) tells the user, in Echo's words: what happened,
/// what to do about it, then git's last line as the detail. `stderr` is None when git didn't start.
/// The address is shown without any credentials typed into it.
fn git_failure(url: &str, git_ref: Option<&str>, stderr: Option<&str>) -> String {
    let Some(stderr) = stderr else {
        return "Echo couldn't get the marketplace: git isn't installed.\nInstall git, then try again.".into();
    };
    let last = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("failed").trim();
    let (shown, host, ssh) = match url.split_once("://") {
        Some((scheme, rest)) => {
            let (auth, path) = rest.split_once('/').unwrap_or((rest, ""));
            let (userinfo, host) = auth.rsplit_once('@').map_or(("", auth), |(u, h)| (u, h));
            let ssh = scheme.eq_ignore_ascii_case("ssh");
            let user = if ssh { userinfo.split(':').next().unwrap_or("") } else { "" };
            let at = if user.is_empty() { String::new() } else { format!("{user}@") };
            (format!("{scheme}://{at}{host}/{path}"), host.split(':').next().unwrap_or(host).to_string(), ssh)
        }
        None => match url.split_once(':').filter(|(h, _)| h.contains('@')) {
            Some((h, _)) => (url.to_string(), h.rsplit('@').next().unwrap_or(h).to_string(), true),
            None => (url.to_string(), url.to_string(), false),
        },
    };
    let low = stderr.to_lowercase();
    let has = |ms: &[&str]| ms.iter().any(|m| low.contains(m));
    let sign_in = if ssh {
        let key = if cfg!(target_os = "macos") { "ssh-add --apple-use-keychain <key>" } else { "ssh-add <key>" };
        format!("Add your SSH key to {host} and load it into the agent (`{key}`); check with `git clone {shown}` in a terminal, then try again.")
    } else {
        format!("Sign in to {host} once in a terminal: run `git clone {shown}` there. Once it works without asking (the login stays in the system's credential store), try again.")
    };
    let (what, todo) = if has(&["could not read username", "could not read password", "terminal prompts disabled", "authentication failed", "http basic: access denied", "permission denied (publickey", "returned error: 401", "returned error: 403"]) {
        (if ssh { "the SSH key wasn't accepted" } else { "it asks to sign in" }.to_string(), sign_in)
    } else if has(&["remote branch", "couldn't find remote ref"]) {
        (format!("no branch or tag {} there", git_ref.unwrap_or("with that name")), "Check the name after # in the address, then try again.".to_string())
    } else if has(&["repository not found", "does not appear to be a git repository"]) || (low.contains("not found") && low.contains("repository")) {
        ("no repository at this address, or it's private and needs a sign-in".to_string(), format!("If it's private: {}", sign_in[..1].to_lowercase() + &sign_in[1..]))
    } else if has(&["could not resolve host", "could not resolve hostname", "failed to connect", "connection timed out", "network is unreachable", "connection refused", "no route to host"]) {
        (format!("can't reach {host}"), "Check the address and the network, then try again.".to_string())
    } else {
        return format!("Echo couldn't get the marketplace from {host}.\ngit said: {last}");
    };
    format!("Echo couldn't get the marketplace from {host}: {what}.\n{todo}\ngit said: {last}")
}

/// Files, bytes: refuses links, special files and anything over the limit.
pub(super) fn check_tree(dir: &Path, limit: Limit) -> Result<(usize, u64)> {
    let (mut files, mut bytes) = (0usize, 0u64);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d)? {
            let e = e?;
            if e.file_name() == ".git" {
                continue;
            }
            let md = fs::symlink_metadata(e.path())?;
            let rel = e.path().strip_prefix(dir).unwrap_or(&e.path()).display().to_string();
            if md.file_type().is_symlink() {
                bail!("it holds a link ({rel}) — refused");
            } else if md.is_dir() {
                stack.push(e.path());
            } else if md.is_file() {
                files += 1;
                bytes += md.len();
            } else {
                bail!("it holds a special file ({rel})");
            }
            if files > limit.files || bytes > limit.bytes {
                bail!("it is too big (over {} files or {} MB)", limit.files, limit.bytes >> 20);
            }
        }
    }
    Ok((files, bytes))
}

/// sha256 over the tree's files in path order (path, size, bytes); what an install records.
pub(super) fn tree_hash(dir: &Path) -> Result<String> {
    let mut list = vec![];
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d)? {
            let p = e?.path();
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            let md = fs::symlink_metadata(&p)?;
            if md.is_dir() {
                stack.push(p);
            } else if md.is_file() {
                list.push(p);
            }
        }
    }
    let rel = |p: &Path| p.strip_prefix(dir).unwrap_or(p).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
    list.sort_by_key(|p| rel(p));
    let mut h = Sha256::new();
    for p in list {
        let b = fs::read(&p)?;
        h.update(rel(&p).as_bytes());
        h.update([0]);
        h.update((b.len() as u64).to_le_bytes());
        h.update(&b);
    }
    Ok(format!("{:x}", h.finalize()))
}

pub(crate) fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        if e.file_name() == ".git" {
            continue;
        }
        let md = fs::symlink_metadata(e.path())?;
        let dest = to.join(e.file_name());
        if md.is_dir() {
            copy_tree(&e.path(), &dest)?;
        } else if md.is_file() {
            fs::copy(e.path(), &dest)?;
        }
    }
    Ok(())
}

/// A folder name for a marketplace (its files and lock).
pub(super) fn market_key(name: &str) -> String {
    let k: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c.to_ascii_lowercase() } else { '-' }).collect();
    let h = format!("{:x}", Sha256::digest(name.as_bytes()));
    format!("{}-{}", k.trim_matches('-').chars().take(40).collect::<String>(), &h[..8])
}

/// Fetch a source into `<base>/<commit>/` (kept if already there); older commits are removed.
pub(super) fn fetch(source: &str, base: &Path, limit: Limit) -> Result<Snapshot> {
    match parse_source(source)? {
        Source::Local(p) => {
            check_tree(&p, limit)?;
            Ok(Snapshot { commit: format!("local-{}", &tree_hash(&p)?[..12]), dir: p })
        }
        src => {
            fs::create_dir_all(base)?;
            let work = base.join(format!(".fetch-{}", std::process::id()));
            let _ = fs::remove_dir_all(&work);
            fs::create_dir_all(&work)?;
            let tree = work.join("tree");
            let got = (|| -> Result<String> {
                match &src {
                    Source::Github { repo, git_ref } => {
                        let sha = github_commit(repo, git_ref.as_deref())?;
                        if base.join(&sha).is_dir() {
                            return Ok(sha);
                        }
                        github_unpack(repo, &sha, &tree, &work)?;
                        Ok(sha)
                    }
                    Source::Git { url, git_ref } => git_fetch(url, git_ref.as_deref(), &tree),
                    Source::Local(_) => unreachable!(),
                }
            })();
            let sha = match got {
                Ok(s) => s,
                Err(e) => {
                    let _ = fs::remove_dir_all(&work);
                    return Err(e);
                }
            };
            let dir = base.join(&sha);
            if !dir.is_dir() {
                if let Err(e) = check_tree(&tree, limit) {
                    let _ = fs::remove_dir_all(&work);
                    return Err(e);
                }
                fs::rename(&tree, &dir)?;
            }
            let _ = fs::remove_dir_all(&work);
            for e in fs::read_dir(base)?.flatten() {
                if e.path() != dir && e.file_name().to_string_lossy().len() == 40 {
                    let _ = fs::remove_dir_all(e.path());
                }
            }
            Ok(Snapshot { dir, commit: sha })
        }
    }
}

#[cfg(test)]
mod git_failure_tests {
    use super::git_failure;

    fn say(url: &str, r: Option<&str>, e: &str) -> String {
        git_failure(url, r, Some(e))
    }

    #[test]
    fn https_without_a_sign_in_says_so_then_how_to_sign_in_then_gits_line() {
        let m = say("https://git.example.com/group/repo.git", None, "Cloning into 'x'...\nfatal: could not read Username for 'https://git.example.com': terminal prompts disabled\n");
        let lines: Vec<_> = m.lines().collect();
        assert_eq!(lines[0], "Echo couldn't get the marketplace from git.example.com: it asks to sign in.");
        assert!(lines[1].starts_with("Sign in to git.example.com once in a terminal: run `git clone https://git.example.com/group/repo.git` there."), "{m}");
        assert_eq!(lines[2], "git said: fatal: could not read Username for 'https://git.example.com': terminal prompts disabled");
    }

    #[test]
    fn ssh_without_an_accepted_key_says_to_add_the_key_and_reads_past_gits_last_line() {
        let e = "git@git.example.com: Permission denied (publickey).\nfatal: Could not read from remote repository.\n";
        for url in ["git@git.example.com:group/repo.git", "ssh://git@git.example.com/group/repo.git"] {
            let m = say(url, None, e);
            assert!(m.starts_with("Echo couldn't get the marketplace from git.example.com: the SSH key wasn't accepted.\nAdd your SSH key to git.example.com and load it into the agent (`ssh-add"), "{m}");
            assert!(m.contains(&format!("`git clone {url}`")), "{m}");
            assert!(m.ends_with("\ngit said: fatal: Could not read from remote repository."), "{m}");
        }
    }

    #[test]
    fn a_gitlab_access_denied_is_a_sign_in_failure() {
        let m = say("https://git.example.com/g/r.git", None, "remote: HTTP Basic: Access denied\nfatal: Authentication failed for 'https://git.example.com/g/r.git/'\n");
        assert!(m.contains(": it asks to sign in.\nSign in to git.example.com once"), "{m}");
    }

    #[test]
    fn repository_not_found_may_be_a_private_one() {
        let m = say("https://example.com/o/r.git", None, "remote: Repository not found.\nfatal: repository 'https://example.com/o/r.git/' not found\n");
        assert!(m.starts_with("Echo couldn't get the marketplace from example.com: no repository at this address, or it's private and needs a sign-in.\nIf it's private: sign in to example.com once"), "{m}");
    }

    #[test]
    fn a_host_that_cant_be_reached_says_to_check_the_address_and_network() {
        let m = say("https://nohost.example.com/r.git", None, "fatal: unable to access 'https://nohost.example.com/r.git/': Could not resolve host: nohost.example.com\n");
        assert_eq!(m, "Echo couldn't get the marketplace from nohost.example.com: can't reach nohost.example.com.\nCheck the address and the network, then try again.\ngit said: fatal: unable to access 'https://nohost.example.com/r.git/': Could not resolve host: nohost.example.com");
        assert!(say("git@nohost.example.com:r.git", None, "ssh: Could not resolve hostname nohost.example.com\n").contains("can't reach nohost.example.com"));
    }

    #[test]
    fn a_missing_branch_or_tag_names_it() {
        let m = say("https://example.com/r.git", Some("v9"), "warning: Could not find remote branch v9 to clone.\nfatal: Remote branch v9 not found in upstream origin\n");
        assert!(m.starts_with("Echo couldn't get the marketplace from example.com: no branch or tag v9 there.\n"), "{m}");
        assert!(!m.contains("ign in"), "{m}");
    }

    #[test]
    fn git_not_installed_says_to_install_it() {
        assert_eq!(git_failure("https://example.com/r.git", None, None), "Echo couldn't get the marketplace: git isn't installed.\nInstall git, then try again.");
    }

    #[test]
    fn an_unrecognised_failure_keeps_gits_line() {
        let m = say("https://example.com/r.git", None, "fatal: something odd\n");
        assert_eq!(m, "Echo couldn't get the marketplace from example.com.\ngit said: fatal: something odd");
    }

    #[test]
    fn credentials_typed_into_the_address_are_not_shown() {
        let m = say("https://me:s3cret@git.example.com/g/r.git", None, "fatal: Authentication failed for 'https://git.example.com/g/r.git/'\n");
        assert!(!m.contains("s3cret") && !m.contains("me:") && !m.contains("me@"), "{m}");
        assert!(m.contains("git clone https://git.example.com/g/r.git`"), "{m}");
        let m = say("ssh://git:pw9@git.example.com/g/r.git", None, "Permission denied (publickey).\n");
        assert!(!m.contains("pw9") && m.contains("ssh://git@git.example.com/g/r.git"), "{m}");
    }
}
