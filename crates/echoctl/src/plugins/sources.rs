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
    run_out(&mut c).with_context(|| format!("git can't fetch {url}"))?;
    let sha = run_out(Command::new(&git).no_window().arg("-C").arg(dest).args(["rev-parse", "HEAD"]))?;
    let sha = hex40(&String::from_utf8_lossy(&sha)).ok_or_else(|| anyhow!("git gave no commit"))?;
    fs::remove_dir_all(dest.join(".git"))?;
    Ok(sha)
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

pub(super) fn copy_tree(from: &Path, to: &Path) -> Result<()> {
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
