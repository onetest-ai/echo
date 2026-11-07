//! Hidden diagnostics and build helpers (`#[command(hide = true)]` in `Cmd`).

use anyhow::Result;

#[cfg(any(target_os = "macos", windows))]
use crate::{ax, graph, watch};
#[cfg(feature = "tray")]
use crate::window;

#[cfg(any(target_os = "macos", windows))]
pub fn call_seen() -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&watch::call_seen_json())?);
    Ok(())
}

#[cfg(any(target_os = "macos", windows))]
pub fn call_people(name: Option<String>) -> Result<()> {
    let name = match name {
        Some(n) => n,
        None => {
            let title = ax::call_title().ok_or_else(|| anyhow::anyhow!("no Teams call window (or Accessibility not granted)"))?;
            println!("call window: {title}");
            ax::call_name(&title).ok_or_else(|| anyhow::anyhow!("the title names nobody"))?
        }
    };
    println!("name: {name} (looks like a person: {})", ax::is_person_name(&name));
    match graph::chat_named(&name)? {
        Some((_, topic, members)) => {
            println!("chat: {}", topic.as_deref().unwrap_or("(1:1)"));
            for m in members {
                println!("  {}{}", m.name, m.email.map(|e| format!(" <{e}>")).unwrap_or_default());
            }
        }
        None => println!("no recent chat with that name"),
    }
    Ok(())
}

#[cfg(feature = "tray")]
pub fn window(selftest: bool, export: Option<std::path::PathBuf>, serve: Option<u16>) -> Result<()> {
    match export {
        _ if serve.is_some() => window::serve(serve.unwrap_or(0)),
        Some(dir) => window::export(&dir),
        None => {
            anyhow::ensure!(selftest, "the window opens from the menu-bar icon (use --selftest to check it)");
            window::selftest()
        }
    }
}
