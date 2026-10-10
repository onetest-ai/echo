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
