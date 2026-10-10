//! Windows: only the microphone privacy toggle can block us. Loopback capture and UI Automation
//! (speaker names) need no consent.

use crate::app::NoWindow;
use std::process::Command;

use anyhow::Result;

use super::{Check, Report, Status};

const MIC_URL: &str = "ms-settings:privacy-microphone";
const CONSENT: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";

fn reg_value(key: &str, name: &str) -> Option<String> {
    let out = Command::new("reg").no_window().args(["query", key, "/v", name]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find(|l| l.trim_start().starts_with(name))
        .and_then(|l| l.split_whitespace().last())
        .map(str::to_owned)
}

fn consent(key: &str) -> Status {
    match reg_value(key, "Value").as_deref() {
        Some("Allow") => Status::Granted,
        Some("Deny") => Status::Denied,
        _ => Status::Unknown,
    }
}

fn mic_status() -> Status {
    // Both the global toggle and "Let desktop apps access your microphone" must be on.
    let global = consent(&format!(r"HKCU\{CONSENT}"));
    let desktop = consent(&format!(r"HKCU\{CONSENT}\NonPackaged"));
    match (global, desktop) {
        (Status::Denied, _) | (_, Status::Denied) => Status::Denied,
        (Status::Granted, Status::Granted) => Status::Granted,
        _ => Status::Unknown,
    }
}

fn build_number() -> u32 {
    reg_value(r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion", "CurrentBuildNumber")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

pub fn report() -> Result<Report> {
    let build = build_number();
    let checks = vec![Check {
        id: "microphone",
        name: "Microphone",
        status: mic_status(),
        required: true,
        why: "records your own voice as the 'me' track",
        how: "Settings → Privacy & security → Microphone: turn on 'Microphone access' and \
              'Let desktop apps access your microphone'"
            .into(),
        settings_url: Some(MIC_URL),
    }, Check {
        // Not a permission: what the 'remote' track contains on Windows (audio/windows.rs).
        id: "call_audio",
        name: "Teams audio (from its output device)",
        status: Status::Granted,
        required: false,
        why: "the 'remote' track records the speakers or headset Teams plays on — other sounds \
              on that device (other apps, notifications) are recorded too; Windows can't isolate \
              Teams' audio",
        how: "keep other audio off the device Teams uses during calls".into(),
        settings_url: None,
    }];
    let os_ok = build >= 19041;
    let ready = os_ok && checks.iter().all(|c| c.status != Status::Denied);
    Ok(Report {
        platform: "windows",
        os_version: format!("build {build}"),
        os_supported: os_ok,
        os_requirement: "Windows 10 version 2004 (build 19041) or later",
        responsible_app: None,
        checks,
        ready,
    })
}
