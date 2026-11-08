//! Microsoft 365 on Echo's sign-in (`echoctl m365 …`) and sending things to yourself (`echoctl deliver …`).

use anyhow::Result;
use clap::Subcommand;

use super::print_json;
#[cfg(feature = "js")]
use crate::jshost;
use crate::{deliver, m365};

#[derive(Subcommand)]
pub enum M365Cmd {
    /// Search mail, Teams chats and files (OneDrive, SharePoint) at once, most relevant first (KQL allowed)
    Search {
        query: String,
        /// What to search: mail, chats, files (comma-separated)
        #[arg(long, default_value = "mail,chats,files")]
        r#in: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// Only mail and chat messages from this person (name or address)
        #[arg(long)]
        from: Option<String>,
        /// Only the last N days
        #[arg(long)]
        days: Option<i64>,
    },
    /// Mail: read, send, reply, draft
    Mail {
        #[command(subcommand)]
        cmd: M365Mail,
    },
    /// Teams: chats, read, send, teams and channels, post
    Teams {
        #[command(subcommand)]
        cmd: M365Teams,
    },
    /// People by name or address: their addresses, title, department, company
    People {
        query: String,
        #[arg(long, default_value_t = 8)]
        limit: usize,
    },
    /// OneDrive: list, download, upload
    Drive {
        #[command(subcommand)]
        cmd: M365Drive,
    },
    /// Any Microsoft Graph GET (a path after /v1.0), read only
    Get { path: String },
}

#[derive(Subcommand)]
pub enum M365Mail {
    /// One mail (--thread: its whole conversation)
    Read {
        id: String,
        #[arg(long)]
        thread: bool,
    },
    /// Send a mail from your account; its text a Markdown file. Without --yes: what would be sent
    Send {
        #[arg(long, required = true)]
        to: Vec<String>,
        #[arg(long)]
        cc: Vec<String>,
        #[arg(long)]
        subject: String,
        #[arg(long)]
        file: std::path::PathBuf,
        /// A model wrote or edited the text (its name): the message says so
        #[arg(long)]
        ai: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    /// Reply to a mail (--all: to everyone on it). Without --yes: what would be sent
    Reply {
        id: String,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        file: std::path::PathBuf,
        /// A model wrote or edited the text (its name): the message says so
        #[arg(long)]
        ai: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    /// A draft in your Drafts (nothing sent): its Outlook link
    Draft {
        #[arg(long)]
        to: Vec<String>,
        #[arg(long)]
        cc: Vec<String>,
        #[arg(long)]
        subject: String,
        #[arg(long)]
        file: std::path::PathBuf,
        /// A model wrote or edited the text (its name): the message says so
        #[arg(long)]
        ai: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum M365Teams {
    /// Your chats, latest first (--with: a person's name or address, or words of the topic)
    Chats {
        #[arg(long)]
        with: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// A chat's messages
    Read {
        chat: String,
        #[arg(long, default_value_t = 7)]
        days: i64,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// A message to a chat (--chat) or a person (--to: their 1:1 chat). Without --yes: what would be sent
    Send {
        #[arg(long)]
        chat: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        file: std::path::PathBuf,
        /// A model wrote or edited the text (its name): the message says so
        #[arg(long)]
        ai: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    /// Your teams, or (with a team id) its channels
    List { team: Option<String> },
    /// A channel's posts with their replies
    Channel {
        team: String,
        channel: String,
        #[arg(long, default_value_t = 7)]
        days: i64,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// A new post in a channel. Without --yes: what would be posted
    Post {
        team: String,
        channel: String,
        #[arg(long)]
        subject: Option<String>,
        #[arg(long)]
        file: std::path::PathBuf,
        /// A model wrote or edited the text (its name): the message says so
        #[arg(long)]
        ai: Option<String>,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum M365Drive {
    /// A folder's items (a path from the root; none: the root)
    List { path: Option<String> },
    /// A file into a local file or folder: a search hit's ref, a OneDrive path, or an item id
    Download {
        item: String,
        #[arg(long, default_value = ".")]
        out: std::path::PathBuf,
    },
    /// A local file into OneDrive at a path. Without --yes: what would be uploaded
    Upload {
        file: std::path::PathBuf,
        path: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum DeliverCmd {
    /// What can be sent now (JSON)
    Status,
    /// Send a test message: teams | mail | a way a plugin adds (telegram…)
    Test { kind: String },
    /// Send a Markdown file to yourself: teams | mail | a way a plugin adds
    Send {
        kind: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        file: std::path::PathBuf,
        /// One line on where it comes from, under the title
        #[arg(long)]
        about: Option<String>,
        /// A model wrote it (its name): the message says so
        #[arg(long)]
        ai: Option<String>,
        /// Mail to these addresses instead of you (comma-separated)
        #[arg(long, default_value = "")]
        to: String,
    },
}

pub fn run(cmd: M365Cmd) -> Result<()> {
    let v = match cmd {
        M365Cmd::Search { query, r#in, limit, from, days } => {
            m365::search(&query, &r#in.split(',').map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect::<Vec<_>>(), limit, from.as_deref(), days)?
        }
        M365Cmd::Mail { cmd } => match cmd {
            M365Mail::Read { id, thread } => m365::mail_read(&id, thread)?,
            M365Mail::Send { to, cc, subject, file, ai, yes } => m365::mail_send(&to, &cc, &subject, &file, ai.as_deref(), yes)?,
            M365Mail::Reply { id, all, file, ai, yes } => m365::mail_reply(&id, all, &file, ai.as_deref(), yes)?,
            M365Mail::Draft { to, cc, subject, file, ai } => m365::mail_draft(&to, &cc, &subject, &file, ai.as_deref())?,
        },
        M365Cmd::Teams { cmd } => match cmd {
            M365Teams::Chats { with, limit } => m365::chats(with.as_deref(), limit)?,
            M365Teams::Read { chat, days, limit } => m365::chat_read(&chat, days, limit)?,
            M365Teams::Send { chat, to, file, ai, yes } => m365::chat_send(chat.as_deref(), to.as_deref(), &file, ai.as_deref(), yes)?,
            M365Teams::List { team } => m365::teams(team.as_deref())?,
            M365Teams::Channel { team, channel, days, limit } => m365::channel_read(&team, &channel, days, limit)?,
            M365Teams::Post { team, channel, subject, file, ai, yes } => m365::channel_post(&team, &channel, subject.as_deref(), &file, ai.as_deref(), yes)?,
        },
        M365Cmd::People { query, limit } => m365::people(&query, limit)?,
        M365Cmd::Drive { cmd } => match cmd {
            M365Drive::List { path } => m365::drive_list(path.as_deref())?,
            M365Drive::Download { item, out } => m365::drive_download(&item, &out)?,
            M365Drive::Upload { file, path, yes } => m365::drive_upload(&file, &path, yes)?,
        },
        M365Cmd::Get { path } => m365::get(&path)?,
    };
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

/// What a delivery test sends: the shapes a routine's answer has (headings, a list, a table).
const TEST_MESSAGE: &str = "This is how Echo's messages reach you.\n\n## Today\n\n1. **Send the deck** to Acme — due today\n2. Review `T012` before the 15:00 call\n\n## Meetings\n\n| Time | Meeting | Prepare |\n|---|---|---|\n| 10:00 | Weekly sync | Open asks |\n| 15:00 | Acme review | The deck |\n\n> Nothing here is real: it's a test.";

pub fn deliver_command(cmd: DeliverCmd) -> Result<()> {
    // The ways plugins add live in their host halves: load them (commands only, no timers).
    #[cfg(feature = "js")]
    {
        jshost::no_timers();
        jshost::sync();
        std::thread::sleep(std::time::Duration::from_millis(600));
    }
    match cmd {
        DeliverCmd::Status => print_json(&deliver::status()),
        DeliverCmd::Send { kind, title, file, about, ai, to } => {
            deliver::ready(&kind)?;
            let to: Vec<String> = to.split(',').map(str::trim).filter(|a| !a.is_empty()).map(str::to_owned).collect();
            let markdown = std::fs::read_to_string(file)?;
            deliver::send(&kind, &deliver::Msg { title: &title, markdown: &markdown, about: about.as_deref(), ai: ai.as_deref(), mail_to: &to })?;
            println!("Sent");
        }
        DeliverCmd::Test { kind } => {
            deliver::ready(&kind)?;
            deliver::send(&kind, &deliver::Msg { title: "Echo test", markdown: TEST_MESSAGE, about: Some("A test from Settings"), ..Default::default() })?;
            println!("Sent");
        }
    }
    Ok(())
}
