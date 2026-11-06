//! Call apps Echo knows how to capture. Phase 1: Teams desktop only.

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetApp {
    Teams,
}

impl TargetApp {
    /// macOS bundle-id prefixes. New Teams (`com.microsoft.teams2`) plays audio from helper
    /// processes whose bundle ids share the prefix, so we match on prefix.
    #[allow(dead_code)]
    pub fn mac_bundle_prefixes(self) -> &'static [&'static str] {
        if let Some(test) = test_call_app() {
            return test;
        }
        match self {
            TargetApp::Teams => &["com.microsoft.teams2", "com.microsoft.teams"],
        }
    }

    /// Windows executable names (lowercase). New Teams is `ms-teams.exe`; classic is `teams.exe`.
    #[allow(dead_code)]
    pub fn windows_exe_names(self) -> &'static [&'static str] {
        match self {
            TargetApp::Teams => &["ms-teams.exe", "teams.exe"],
        }
    }
}

/// Functional tests only, in a debug build: `ECHOCTL_CALL_APP_BUNDLE=<bundle id>` makes that app the call app
/// instead of Teams (`tests/tools/testcall`, a call between two Macs on the local network), so the whole
/// recording path runs without a Teams account. A release build never reads it.
#[allow(dead_code)]
fn test_call_app() -> Option<&'static [&'static str]> {
    #[cfg(debug_assertions)]
    {
        static APP: std::sync::OnceLock<Option<Vec<&'static str>>> = std::sync::OnceLock::new();
        APP.get_or_init(|| {
            let id = std::env::var("ECHOCTL_CALL_APP_BUNDLE").ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())?;
            Some(vec![&*Box::leak(id.into_boxed_str())])
        })
        .as_deref()
    }
    #[cfg(not(debug_assertions))]
    None
}
