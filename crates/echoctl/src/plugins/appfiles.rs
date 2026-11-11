//! `uses.files.read`: other apps' own files a plugin reads, each named in full.

use anyhow::{anyhow, bail, Result};
use std::path::PathBuf;

/// An entry of `uses.files.read`: its base (`~`, `%APPDATA%`, `%LOCALAPPDATA%`) and the plain relative path after it —
/// no `..`, no pattern, not a place that holds secrets.
pub fn app_file_rel(entry: &str) -> Result<(&'static str, &str)> {
    let (base, rel) = [("~/", "~"), ("%APPDATA%/", "%APPDATA%"), ("%LOCALAPPDATA%/", "%LOCALAPPDATA%")]
        .iter()
        .find_map(|(p, b)| entry.strip_prefix(p).map(|r| (*b, r)))
        .ok_or_else(|| anyhow!("“{entry}”: a file of another app starts with ~/, %APPDATA%/ or %LOCALAPPDATA%/"))?;
    let parts: Vec<&str> = rel.split('/').collect();
    // Plain names only — a trailing dot or space, or `~`, is another name on Windows (`.ssh.`, `SSH~1`).
    if rel.is_empty() || entry.len() > 200 || parts.iter().any(|p| p.is_empty() || *p == "." || *p == ".." || p.contains(['*', '?', '\\', ':', '~']) || p.ends_with(['.', ' '])) {
        bail!("“{entry}”: one file by its plain path");
    }
    // An app's own settings only: `~/Library/Application Support/<app>/…`, `~/.config/<app>/…`, or
    // `%APPDATA%/<app>/…` / `%LOCALAPPDATA%/<app>/…` — never Echo's, never a place or name that holds credentials.
    let app = match (base, parts.as_slice()) {
        ("~", ["Library", "Application Support", app, _, ..]) | ("~", [".config", app, _, ..]) => *app,
        ("%APPDATA%" | "%LOCALAPPDATA%", [app, _, ..]) => *app,
        _ => bail!("“{entry}”: an app's own file under ~/Library/Application Support/<app>/, ~/.config/<app>/, %APPDATA%/<app>/ or %LOCALAPPDATA%/<app>/"),
    };
    const NOT_APPS: [&str; 10] = ["echo", "gh", "git", "gcloud", "claude", "op", "1password", "google", "microsoft", "mozilla"];
    const SECRETS: [&str; 10] = ["cookies", "login data", "credentials", "hosts.yml", "token", "tokens", "secrets", "keychains", "auth.json", ".credentials.json"];
    let lower = |p: &&str| p.to_ascii_lowercase();
    if NOT_APPS.contains(&app.to_ascii_lowercase().as_str()) || parts.iter().map(lower).any(|p| SECRETS.iter().any(|s| p == *s || p.starts_with(&format!("{s}.")))) {
        bail!("“{entry}”: not a file that holds Echo's or another app's credentials");
    }
    Ok((base, rel))
}

/// Where an entry of `uses.files.read` is on this computer (its base from the environment), if its base exists here.
pub fn app_file(entry: &str, var: impl Fn(&str) -> Option<PathBuf>) -> Result<Option<PathBuf>> {
    let (base, rel) = app_file_rel(entry)?;
    let root = match base {
        "~" => var("HOME").or_else(|| var("USERPROFILE")),
        "%APPDATA%" => var("APPDATA"),
        _ => var("LOCALAPPDATA"),
    };
    Ok(root.map(|r| r.join(rel)))
}

/// The file an entry of `uses.files.read` names, when it is there as named: no link anywhere below its base (a folder
/// above it that is a link or junction would lead elsewhere, e.g. to Echo's own settings) — its real path is its
/// base's real path joined with the named parts.
pub fn app_file_real(entry: &str, var: impl Fn(&str) -> Option<PathBuf>) -> Result<Option<PathBuf>> {
    let (_, rel) = app_file_rel(entry)?;
    let Some(f) = app_file(entry, var)? else { return Ok(None) };
    let base = f.to_string_lossy().strip_suffix(rel).map(PathBuf::from);
    let (Some(base), Ok(real)) = (base, dunce::canonicalize(&f)) else { return Ok(None) };
    let Ok(base) = dunce::canonicalize(base) else { return Ok(None) };
    Ok((real == base.join(rel) && real.is_file()).then_some(real))
}
