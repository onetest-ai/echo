//! Web pages for plugins and Echo: reading one's HTML as Markdown (`read`), whether an address is public (`public`),
//! and the proxy a rendered page reaches the web through (`gate`).

pub mod gate;
mod link;
pub mod public;
pub mod read;
