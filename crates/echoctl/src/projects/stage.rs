//! A thread's stage and its history (moved out of `projects.rs` as it was).

use super::*;

/// Where a project stands. The group (`pursuit`, `delivery`, `closed`) decides what Echo does with
/// it; the stage is for the user and for the sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Lead,
    Proposal,
    Negotiation,
    Kickoff,
    Active,
    OnHold,
    WrapUp,
    Done,
    Lost,
    Cancelled,
}

impl Stage {
    pub const ALL: [Stage; 10] = [
        Stage::Lead,
        Stage::Proposal,
        Stage::Negotiation,
        Stage::Kickoff,
        Stage::Active,
        Stage::OnHold,
        Stage::WrapUp,
        Stage::Done,
        Stage::Lost,
        Stage::Cancelled,
    ];

    pub fn group(self) -> &'static str {
        match self {
            Stage::Lead | Stage::Proposal | Stage::Negotiation => "pursuit",
            Stage::Kickoff | Stage::Active | Stage::OnHold | Stage::WrapUp => "delivery",
            Stage::Done | Stage::Lost | Stage::Cancelled => "closed",
        }
    }

    pub fn closed(self) -> bool {
        self.group() == "closed"
    }

    pub fn id(self) -> &'static str {
        match self {
            Stage::Lead => "lead",
            Stage::Proposal => "proposal",
            Stage::Negotiation => "negotiation",
            Stage::Kickoff => "kickoff",
            Stage::Active => "active",
            Stage::OnHold => "on_hold",
            Stage::WrapUp => "wrap_up",
            Stage::Done => "done",
            Stage::Lost => "lost",
            Stage::Cancelled => "cancelled",
        }
    }

    /// "on hold", "On-Hold", "on_hold" → OnHold.
    pub fn parse(s: &str) -> Result<Stage> {
        let k: String = s.trim().to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
        Stage::ALL.into_iter().find(|st| st.id() == k).ok_or_else(|| {
            anyhow!("unknown stage {s:?} (one of: {})", Stage::ALL.map(|s| s.id()).join(", "))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageChange {
    pub stage: Stage,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Move a project to a stage (a dated event, with why).
pub fn set_stage(id: &str, stage: Stage, reason: Option<&str>) -> Result<Project> {
    let _l = lock()?;
    let mut p = load(id)?;
    if p.stage != stage {
        p.stage = stage;
        p.stages.push(StageChange { stage, at: Utc::now().to_rfc3339(), reason: reason.map(str::to_owned).filter(|r| !r.trim().is_empty()) });
        write_json(&profile_path(id)?, &p)?;
    }
    Ok(p)
}
