//! A page rendered for a plugin (`ctx.web.read(url, {render: true})`, #835): the host's side. The page renders in the child
//! process `echoctl render-page` (`web::render`), apart from Echo's window, its IPC and the plugin's `ctx`:
//! - no stdin, and an environment of only what a webview needs (no secrets, no `CLAUDE*` / `ANTHROPIC*`);
//! - its own process group, stopped whole when its time is up;
//! - its answer read back as data: the DOM's HTML and the address it ended at, which `web::read` reads as any page.
//!
//! One render at a time per plugin, two in all.

// Only macOS renders yet (Windows: the next step of #835).
#![cfg_attr(not(all(feature = "tray", target_os = "macos")), allow(dead_code))]

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;

/// A rendered page: its HTML, the address it ended at, whether its HTML was cut.
pub(super) struct Rendered {
    pub html: String,
    pub url: String,
    pub truncated: bool,
}

/// Renders running in this process, by plugin.
static RENDERING: Mutex<Vec<String>> = Mutex::new(Vec::new());
const AT_ONCE: usize = 2;
/// The child's answer read back, in bytes at most: 5 MB of HTML escaped as JSON (at most 6 bytes for one, a control
/// character as `\u00XX`) and the rest of the line; and why it failed.
const ANSWER_MAX: u64 = 6 * (5 << 20) + (1 << 20);
const WHY_MAX: u64 = 4 << 10;
/// Past the render's own time, how long the child has to start and answer before its group is stopped.
const GRACE: Duration = Duration::from_secs(5);

/// A render's place, given back when dropped.
struct Place(String);

impl Place {
    fn take(id: &str) -> Result<Place> {
        let mut all = RENDERING.lock().unwrap_or_else(|e| e.into_inner());
        if all.iter().any(|p| p == id) {
            bail!("{id} is already rendering a page: one at a time");
        }
        if all.len() >= AT_ONCE {
            bail!("Echo is rendering {AT_ONCE} pages already: try again in a moment");
        }
        all.push(id.to_string());
        Ok(Place(id.to_string()))
    }
}

impl Drop for Place {
    fn drop(&mut self) {
        let mut all = RENDERING.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = all.iter().position(|p| *p == self.0) {
            all.remove(i);
        }
    }
}

/// Plugin `id`'s page at `url` (already checked: a public https page), rendered within `secs` (30 at most). `test`: a
/// development build's test maps its host to a fake service (`ECHOCTL_TEST_HOSTS`, passed on to the child).
#[cfg(all(feature = "tray", target_os = "macos"))]
pub(super) fn page(id: &str, url: &str, test: bool, secs: u64) -> Result<Rendered> {
    let _place = Place::take(id)?;
    let mut cmd = Command::new(super::files::reader_exe()?);
    cmd.args(["render-page", "--url", url, "--secs", &secs.to_string()]);
    cmd.env_clear().stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for k in ["HOME", "TMPDIR", "USER", "LOGNAME", "LANG"] {
        if let Some(v) = std::env::var_os(k) {
            cmd.env(k, v);
        }
    }
    cmd.env("PATH", "/usr/bin:/bin");
    #[cfg(debug_assertions)]
    if test {
        if let Some(v) = std::env::var_os("ECHOCTL_TEST_HOSTS") {
            cmd.env("ECHOCTL_TEST_HOSTS", v);
        }
    }
    #[cfg(not(debug_assertions))]
    let _ = test;
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().context("the page's renderer couldn't start")?;
    let drain = |s: Option<Box<dyn Read + Send>>, most: u64| {
        std::thread::spawn(move || {
            let mut b = vec![];
            if let Some(s) = s {
                let _ = s.take(most).read_to_end(&mut b);
            }
            b
        })
    };
    let out = drain(child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>), ANSWER_MAX);
    let err = drain(child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>), WHY_MAX);
    let limit = Duration::from_secs(secs) + GRACE;
    let started = Instant::now();
    let status = loop {
        match child.try_wait()? {
            Some(s) => break s,
            None if started.elapsed() > limit => {
                crate::ai::stop_group(child.id());
                let _ = child.kill();
                let _ = child.wait();
                bail!("the page took over {secs} s to render");
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    // The child is reaped: its group id may already be another's, so nothing is sent to it.
    let (out, err) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
    if !status.success() {
        let why = String::from_utf8_lossy(&err);
        bail!("{}", why.lines().next().map(str::trim).filter(|l| !l.is_empty()).unwrap_or("the page couldn't be rendered"));
    }
    answer(&out)
}

#[cfg(not(all(feature = "tray", target_os = "macos")))]
pub(super) fn page(id: &str, url: &str, test: bool, secs: u64) -> Result<Rendered> {
    let _ = (id, url, test, secs);
    if cfg!(windows) {
        // WebView2 is the next step of #835 (design § Request interception per OS).
        bail!("rendering a page isn't on Windows yet: read it without render");
    }
    bail!("this build of Echo can't render pages: read it without render")
}

/// The child's answer: `{html, url, truncated}`, its address an http(s) one.
fn answer(out: &[u8]) -> Result<Rendered> {
    let v: Value = serde_json::from_slice(out).map_err(|_| anyhow!("the page's renderer answered nothing readable"))?;
    let url = v["url"].as_str().unwrap_or("").to_string();
    if crate::web::read::web_link("", &url).as_deref() != Some(url.as_str()) {
        bail!("the page's renderer answered nothing readable");
    }
    Ok(Rendered { html: v["html"].as_str().unwrap_or("").to_string(), url, truncated: v["truncated"] == true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_render_per_plugin_and_two_in_all() {
        let a = Place::take("render-a").unwrap();
        assert!(Place::take("render-a").err().unwrap().to_string().contains("one at a time"));
        let b = Place::take("render-b").unwrap();
        assert!(Place::take("render-c").err().unwrap().to_string().contains("2 pages already"));
        drop((a, b));
        drop(Place::take("render-c").unwrap());
    }

    #[test]
    fn only_an_http_address_comes_back() {
        assert_eq!(answer(br#"{"html":"<p>x</p>","url":"https://news.acme.example/a","truncated":false}"#).unwrap().url, "https://news.acme.example/a");
        for bad in [&br#"{"html":"","url":"file:///etc/hosts"}"#[..], br#"{"html":""}"#, b"not json"] {
            assert!(answer(bad).is_err());
        }
    }
}
