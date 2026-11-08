//! The CLI's command groups by area: each module holds its subcommand enums and what they run. The top-level
//! `Cmd` and its dispatch stay in `main.rs`.

pub mod app;
#[cfg(feature = "brain")]
pub mod brain;
pub mod calendar;
pub mod calls;
pub mod capture;
pub mod dev;
pub mod m365;
pub mod plugin;
pub mod project;

pub fn print_json(v: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}
