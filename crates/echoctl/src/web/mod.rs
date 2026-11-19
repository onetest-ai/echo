//! Web pages for plugins and Echo: reading one's HTML as Markdown (`read`), whether an address is public (`public`),
//! the proxy a rendered page reaches the web through (`gate`), and the child process that renders it (`render`).

pub mod gate;
mod link;
pub mod public;
pub mod read;
#[cfg(feature = "tray")]
pub mod render;
