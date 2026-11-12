//! Widgets: the catalog Today picks from, and a widget's data — what the window shows, for Claude and scripts.

use anyhow::Result;
use clap::Subcommand;

use super::print_json;
use crate::today_layout::{self as layout, Side};
use crate::{tasks, watch, widgets};

#[derive(Subcommand)]
pub enum WidgetCmd {
    /// The widgets Today can show (JSON): id, part, label, kind, sizes, height, heights, temporal
    List,
    /// A widget's data (JSON), as Today gets it; a temporal widget's says `empty` when it has nothing
    Show { id: String },
}

pub fn widget(cmd: WidgetCmd) -> Result<()> {
    match cmd {
        WidgetCmd::List => print_json(&serde_json::json!(widgets::catalog())),
        WidgetCmd::Show { id } => {
            widgets::find(&id)?;
            // A plugin's widget: its provider asked now, waited for (its author sees the answer it gives).
            #[cfg(feature = "js")]
            if let Some((plugin, widget)) = id.strip_prefix("plugin:").and_then(|r| r.split_once(':')) {
                crate::jshost::widget_ask_now(plugin, widget);
            }
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let tasks = tasks::across(&watch::agenda(), chrono::Local::now().date_naive(), &now)?;
            print_json(&widgets::data(&id, &widgets::Inputs { tasks: &tasks })?);
        }
    }
    Ok(())
}

#[derive(Subcommand)]
pub enum WorkCmd {
    /// The plugins' work rows (JSON), each plugin asked now: the user's (Today → My work), or a thread's (its Work tab:
    /// what the plugins give for it and the user's rows that name it); `errors`: why a plugin's rows were left out
    Show {
        /// A thread's id: its rows
        #[arg(long)]
        thread: Option<String>,
    },
}

pub fn work(cmd: WorkCmd) -> Result<()> {
    match cmd {
        WorkCmd::Show { thread } => {
            if let Some(t) = &thread {
                crate::projects::load(t)?;
            }
            #[cfg(feature = "js")]
            print_json(&crate::jshost::work::show(thread.as_deref()));
            #[cfg(not(feature = "js"))]
            anyhow::bail!("this build runs no plugins");
        }
    }
    #[allow(unreachable_code)]
    Ok(())
}

/// Where a widget goes, against one on Today (none: last, on a row of its own).
#[derive(clap::Args)]
#[group(multiple = false)]
pub struct Target {
    /// In that widget's row, before it
    #[arg(long, value_name = "WIDGET")]
    before: Option<String>,
    /// In that widget's row, after it
    #[arg(long, value_name = "WIDGET")]
    after: Option<String>,
    /// On a row of its own above that widget
    #[arg(long, value_name = "WIDGET")]
    above: Option<String>,
    /// On a row of its own below that widget
    #[arg(long, value_name = "WIDGET")]
    below: Option<String>,
}

impl Target {
    fn get(&self) -> Option<(&str, Side)> {
        [(&self.before, Side::Before), (&self.after, Side::After), (&self.above, Side::Above), (&self.below, Side::Below)]
            .into_iter()
            .find_map(|(w, side)| w.as_deref().map(|w| (w, side)))
    }
}

#[derive(Subcommand)]
pub enum TodayCmd {
    /// Today's widgets in order (JSON): widget, size (S, M, L), height (2 rows; 1 when not said), row (starts a new row)
    Show,
    /// Put a widget on Today (`echoctl widget list` names them); --size, and where
    Add {
        widget: String,
        #[arg(long)]
        size: Option<String>,
        #[command(flatten)]
        target: Target,
    },
    /// Move a widget on Today
    Move {
        widget: String,
        #[command(flatten)]
        target: Target,
    },
    /// Change a widget's size on Today: S, M or L (1, 2, 3 columns), of those it takes
    Size { widget: String, size: String },
    /// Change a widget's height on Today: 1 or 2 rows (two: its row and the next, the widgets beside it), of those it takes
    Height { widget: String, height: u8 },
    /// Set one of a widget's settings: `echoctl today set core:tasks.glance period month`
    Set { widget: String, key: String, value: String },
    /// Take a widget off Today
    Remove { widget: String },
    /// Today as it comes: Needs you now, the routines' cards, Your day beside My work
    Reset,
}

pub fn today(cmd: Option<TodayCmd>) -> Result<()> {
    let items = match cmd.unwrap_or(TodayCmd::Show) {
        TodayCmd::Show => layout::load(),
        TodayCmd::Add { widget, size, target } => layout::add(&widget, size.as_deref(), target.get())?,
        TodayCmd::Move { widget, target } => layout::move_to(&widget, target.get())?,
        TodayCmd::Size { widget, size } => layout::resize(&widget, &size)?,
        TodayCmd::Height { widget, height } => layout::set_height(&widget, height)?,
        TodayCmd::Set { widget, key, value } => layout::set_param(&widget, &key, &value)?,
        TodayCmd::Remove { widget } => layout::remove(&widget)?,
        TodayCmd::Reset => layout::reset()?,
    };
    print_json(&serde_json::json!(items));
    Ok(())
}
