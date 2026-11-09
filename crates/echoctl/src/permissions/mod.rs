//! `echoctl doctor`: what the OS must allow before we can capture, why, and how to grant it.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Granted,
    Denied,
    #[cfg_attr(windows, allow(dead_code))]
    NotDetermined,
    Unknown,
}

#[derive(Debug, Serialize)]
pub struct Check {
    /// Stable id, also accepted by `doctor --open <id>`
    pub id: &'static str,
    pub name: &'static str,
    pub status: Status,
    /// Without it, can we still record something useful?
    pub required: bool,
    pub why: &'static str,
    pub how: String,
    pub settings_url: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub platform: &'static str,
    pub os_version: String,
    pub os_supported: bool,
    pub os_requirement: &'static str,
    /// The app the OS attributes our permissions to (Echo.app, or the terminal running the CLI)
    pub responsible_app: Option<String>,
    pub checks: Vec<Check>,
    pub ready: bool,
}

pub fn report() -> Result<Report> {
    #[cfg(target_os = "macos")]
    return macos::report();
    #[cfg(windows)]
    return windows::report();
    #[cfg(not(any(target_os = "macos", windows)))]
    anyhow::bail!("unsupported platform")
}

fn request_all() -> Result<()> {
    #[cfg(target_os = "macos")]
    return macos::request_all();
    #[cfg(windows)]
    return Ok(()); // nothing can be prompted on Windows; the user flips the privacy toggle
    #[cfg(not(any(target_os = "macos", windows)))]
    anyhow::bail!("unsupported platform")
}

pub fn open_settings(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(url).status()?;
    #[cfg(windows)]
    // Not `cmd /C start`: cmd would split the URL at `&`.
    std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).status()?;
    Ok(())
}

/// The permission report of *this* process, optionally after triggering the prompts and/or
/// probing System Audio Recording (macOS).
pub fn collect(request: bool, probe: bool) -> Result<Report> {
    if request {
        request_all()?;
    }
    #[allow(unused_mut)]
    let mut r = report()?;
    #[cfg(target_os = "macos")]
    if probe || request {
        macos::apply_probe(&mut r, macos::probe_peak()?);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = probe; // Windows loopback needs no permission
    Ok(r)
}

/// `echoctl doctor`. Permissions belong to the process that records — the app — so when the
/// app is running it checks (and prompts) in its own process; otherwise this process checks
/// and says whose permissions those are.
pub fn doctor(json: bool, request: bool, probe: bool, open: Option<&str>, no_ai: bool) -> Result<()> {
    let (r, from_app) = match crate::app::app_health(request, probe)? {
        Some(v) => (v, true),
        None => (serde_json::to_value(collect(request, probe)?)?, false),
    };
    if let Some(id) = open {
        let url = r["checks"].as_array().into_iter().flatten().find(|c| c["id"] == id).and_then(|c| c["settings_url"].as_str());
        match url {
            Some(url) => open_settings(url)?,
            None => anyhow::bail!("no settings page for '{id}'"),
        }
    }
    // Local AI profiles' context (#224): their models are loaded to check, so not on --no-ai or the repeated --request / --probe / --open.
    let local = if no_ai || request || probe || open.is_some() { vec![] } else { crate::ollama::local_profiles() };
    if json {
        let mut out = r.clone();
        out["checked_by"] = serde_json::json!(if from_app { "app" } else { "this process" });
        out["ai"] = local.iter().map(crate::ollama::report).collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    let s = |v: &serde_json::Value| v.as_str().unwrap_or("").to_string();
    println!("echoctl doctor — {} {}", s(&r["platform"]), s(&r["os_version"]));
    if r["os_supported"] == false {
        println!("  ✗ OS too old: requires {}", s(&r["os_requirement"]));
    }
    if from_app {
        println!("  checked by the running echoctl app");
    } else {
        println!("  the echoctl app is not running — these are the permissions of {}", r["responsible_app"].as_str().unwrap_or("this terminal"));
        println!("  (recording happens in the app: start it with `echoctl app start` and run doctor again)");
    }
    println!();
    for c in r["checks"].as_array().into_iter().flatten() {
        let status = s(&c["status"]);
        let mark = match status.as_str() {
            "granted" => "✓",
            "denied" => "✗",
            _ => "?",
        };
        let req = if c["required"] == true { "" } else { " (optional)" };
        println!("{mark} {}{req} — {status}", s(&c["name"]));
        println!("    why: {}", s(&c["why"]));
        if status != "granted" {
            println!("    how: {}", s(&c["how"]));
            if !c["settings_url"].is_null() {
                println!("    run: echoctl doctor --open {}", s(&c["id"]));
            }
        }
    }
    if !local.is_empty() {
        println!();
        println!("AI profiles — loading each local model to check its context (up to 2 min each)…");
        for p in &local {
            crate::ollama::print(&crate::ollama::report(p));
        }
    }
    println!();
    println!("{}", if r["ready"] == true { "READY to record" } else { "NOT READY — fix the ✗/? items above" });
    Ok(())
}
