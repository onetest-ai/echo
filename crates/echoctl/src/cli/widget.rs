//! Widgets: the catalog Today picks from, and a widget's data — what the window shows, for Claude and scripts.

use anyhow::Result;
use clap::Subcommand;

use super::print_json;
use crate::{tasks, watch, widgets};

#[derive(Subcommand)]
pub enum WidgetCmd {
    /// The widgets Today can show (JSON): id, part, label, kind, sizes, height, temporal
    List,
    /// A widget's data (JSON), as Today gets it; a temporal widget's says `empty` when it has nothing
    Show { id: String },
}

pub fn widget(cmd: WidgetCmd) -> Result<()> {
    match cmd {
        WidgetCmd::List => print_json(&serde_json::json!(widgets::catalog())),
        WidgetCmd::Show { id } => {
            widgets::find(&id)?;
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let tasks = tasks::across(&watch::agenda(), chrono::Local::now().date_naive(), &now)?;
            print_json(&widgets::data(&id, &widgets::Inputs { tasks: &tasks })?);
        }
    }
    Ok(())
}
