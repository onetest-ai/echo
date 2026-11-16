//! A thread's stage and its history; an Echo plugin's stage writes (`ctx.threads.setStage`, uses.echo.write: thread_stage),
//! with its name and reason, until the user changes the stage away from the plugin's.

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
    /// The Echo plugin that set it (`ctx.threads.setStage`), with its name; none: the user, or Echo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by_name: Option<String>,
}

/// Move a project to a stage (a dated event, with why).
pub fn set_stage(id: &str, stage: Stage, reason: Option<&str>) -> Result<Project> {
    let _l = lock()?;
    let mut p = load(id)?;
    if p.stage != stage {
        p.stage = stage;
        p.stages.push(StageChange { stage, at: Utc::now().to_rfc3339(), reason: reason.map(str::to_owned).filter(|r| !r.trim().is_empty()), by: None, by_name: None });
        write_json(&profile_path(id)?, &p)?;
    }
    Ok(p)
}

/// Whether the user changed the thread's stage away from what `plugin` set last, and hasn't let it set the stage
/// again since: then the plugin's stage writes are ignored. A stage set before the plugin's first write doesn't count.
pub fn stage_held_from(p: &Project, plugin: &str) -> bool {
    let Some(last) = p.stages.iter().rposition(|c| c.by.as_deref() == Some(plugin)) else { return false };
    let handed = p.stage_handed.get(plugin).map(String::as_str).unwrap_or("");
    p.stages[last + 1..].iter().any(|c| c.by.is_none() && c.at.as_str() > handed)
}

/// An Echo plugin sets a thread's stage (`ctx.threads.setStage`), its name and reason in the history: `{applied, why?}`
/// — not applied once the user changed the stage away from the plugin's (`stage_held_from`).
pub fn plugin_set_stage(id: &str, plugin: &str, name: &str, stage: Stage, reason: Option<&str>) -> Result<serde_json::Value> {
    let _l = lock()?;
    let mut p = load(id)?;
    if stage_held_from(&p, plugin) {
        return Ok(serde_json::json!({"applied": false, "why": "the user set this thread's stage themselves"}));
    }
    if p.stage != stage {
        p.stage = stage;
        let reason = reason.map(str::trim).filter(|r| !r.is_empty()).map(|r| r.chars().take(300).collect());
        p.stages.push(StageChange { stage, at: Utc::now().to_rfc3339(), reason, by: Some(plugin.into()), by_name: Some(name.into()) });
        write_json(&profile_path(id)?, &p)?;
    }
    Ok(serde_json::json!({"applied": true}))
}

/// The user lets `plugin` set the thread's stage again after changing it themselves.
pub fn hand_stage_back(id: &str, plugin: &str) -> Result<Project> {
    let _l = lock()?;
    let mut p = load(id)?;
    if !p.stages.iter().any(|c| c.by.as_deref() == Some(plugin)) {
        bail!("no plugin {plugin} set this thread's stage");
    }
    p.stage_handed.insert(plugin.into(), Utc::now().to_rfc3339());
    write_json(&profile_path(id)?, &p)?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugin_moves_the_stage_until_the_user_overrides_it_and_again_once_they_hand_it_back() {
        let (_env, _root) = crate::paths::testenv::folders("projects-plugin-stage");
        let p = create("Aurora", None, None).unwrap();
        // The user's own stage before the plugin's first write holds nothing.
        set_stage(&p.id, Stage::Lead, None).unwrap();
        assert_eq!(plugin_set_stage(&p.id, "tracker", "Acme Tracker", Stage::Proposal, Some("moved in the tracker")).unwrap()["applied"], true);
        let q = load(&p.id).unwrap();
        let c = q.stages.last().unwrap();
        assert_eq!((q.stage, c.by.as_deref(), c.by_name.as_deref(), c.reason.as_deref()), (Stage::Proposal, Some("tracker"), Some("Acme Tracker"), Some("moved in the tracker")));
        // The same stage again: nothing new in the history.
        plugin_set_stage(&p.id, "tracker", "Acme Tracker", Stage::Proposal, None).unwrap();
        assert_eq!(load(&p.id).unwrap().stages.len(), q.stages.len());
        // The user overrides it: the plugin's writes are ignored, another plugin's aren't.
        set_stage(&p.id, Stage::Negotiation, Some("call went well")).unwrap();
        let r = plugin_set_stage(&p.id, "tracker", "Acme Tracker", Stage::Lost, None).unwrap();
        assert_eq!((r["applied"].as_bool(), r["why"].as_str()), (Some(false), Some("the user set this thread's stage themselves")));
        assert!(stage_held_from(&load(&p.id).unwrap(), "tracker") && !stage_held_from(&load(&p.id).unwrap(), "board"));
        assert_eq!(load(&p.id).unwrap().stage, Stage::Negotiation);
        assert!(hand_stage_back(&p.id, "board").is_err(), "a plugin that never set it");
        // Handed back: it moves the stage again, until the user overrides it again.
        hand_stage_back(&p.id, "tracker").unwrap();
        assert_eq!(plugin_set_stage(&p.id, "tracker", "Acme Tracker", Stage::Lost, Some("lost in the tracker")).unwrap()["applied"], true);
        assert_eq!(load(&p.id).unwrap().stage, Stage::Lost);
        update(&p.id, |p| {
            p.stage = Stage::Negotiation;
            Ok(())
        })
        .unwrap();
        assert_eq!(plugin_set_stage(&p.id, "tracker", "Acme Tracker", Stage::Done, None).unwrap()["applied"], false);
    }
}
