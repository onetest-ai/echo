//! Terminals: the window's pseudo-terminals, the shell, the clipboard.

use super::*;

/// Whether the page may open a plain shell: only `echoctl window --selftest` needs one.
pub(super) fn shell_allowed(selftest: bool) -> bool {
    selftest
}

/// The user's own shell, in the data folder (a login shell: their PATH and setup).
pub(super) fn shell_command() -> portable_pty::CommandBuilder {
    #[cfg(windows)]
    let mut c = portable_pty::CommandBuilder::new(std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into()));
    #[cfg(not(windows))]
    let mut c = {
        let mut c = portable_pty::CommandBuilder::new(std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into()));
        c.arg("-l");
        c
    };
    c.cwd(paths::home());
    c
}

// ── Settings (the window's Settings activity) ───────────────────────────────────────────────

pub(crate) fn copy_to_clipboard(text: &str) {
    use std::io::Write;
    #[cfg(target_os = "macos")]
    let cmd = std::process::Command::new("pbcopy").stdin(std::process::Stdio::piped()).spawn();
    #[cfg(windows)]
    let cmd = std::process::Command::new("clip").no_window().stdin(std::process::Stdio::piped()).spawn();
    #[cfg(not(any(target_os = "macos", windows)))]
    let cmd: std::io::Result<std::process::Child> = Err(std::io::Error::other("no clipboard"));
    if let Ok(mut child) = cmd {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

/// The clipboard's text (empty when it holds none: an image, say). macOS only: there ⌘V never
/// reaches the page (no Edit menu); on Windows Ctrl+V goes to the program, which reads it itself.
pub(super) fn clipboard_text() -> String {
    #[cfg(target_os = "macos")]
    let out = std::process::Command::new("pbpaste").output();
    #[cfg(not(target_os = "macos"))]
    let out: std::io::Result<std::process::Output> = Err(std::io::Error::other("no clipboard"));
    out.ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

/// What a paste types into a terminal, as xterm.js's own paste does: line ends as Enter, inside
/// bracketed-paste marks when the program asked for them (so Claude Code takes several lines as
/// one message). No text — an image on the clipboard — sends Ctrl+V: Claude Code reads the
/// clipboard itself then.
pub(super) fn paste_bytes(text: String, bracketed: bool) -> Vec<u8> {
    if text.is_empty() {
        return vec![0x16];
    }
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    if bracketed {
        // A pasted end mark would end the paste early and run the rest as typed keys.
        format!("\x1b[200~{}\x1b[201~", text.replace("\x1b[201~", "")).into_bytes()
    } else {
        text.into_bytes()
    }
}


/// The echo:// requests of this area (`respond` asks each area in turn).
pub(super) fn route(path: &str, req: &Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>> {
    Some(match path {
        "/api/sessions" => json_reply(json!(crate::terminal::saved_sessions())),
        "/api/ptys" => json_reply(crate::terminal::list_value()),
        // A Claude panel's conversations (#44): `?call=<name>` (this call's) or `?profile=<id>` (the Echo folder's).
        "/api/conversations" => {
            let q = req.uri().query().unwrap_or("");
            let key = match query_param(q, "call") {
                Some(name) => crate::calls::resolve_call(&name).and_then(|d| crate::terminal::call_store_key(&d)).ok(),
                None => Some(crate::terminal::folder_store_key(query_param(q, "profile").filter(|p| !p.is_empty()).as_deref())),
            };
            json_reply(key.map(|k| crate::terminal::conversations(&k)).unwrap_or_else(|| json!([])))
        }
        // A terminal's output after byte `from`: answered at once when there is some, else when
        // some comes (or after 25 s) — the page asks again right away (a long poll).
        p if p.starts_with("/api/pty/") => {
            let id = p["/api/pty/".len()..].parse::<u64>().ok();
            let from = query_param(req.uri().query().unwrap_or(""), "from").and_then(|f| f.parse::<u64>().ok()).unwrap_or(0);
            match id.and_then(|id| crate::terminal::read(id, from, std::time::Duration::from_secs(25))) {
                Some(c) => {
                    use base64::Engine as _;
                    json_reply(json!({ "from": c.from, "next": c.next, "reset": c.reset, "exited": c.exited,
                                       "data": base64::engine::general_purpose::STANDARD.encode(&c.data) }))
                }
                None => reply(404, "text/plain", b"no such terminal".to_vec()),
            }
        }
        _ => return None,
    })
}

/// The page's commands of this area (`handle` asks each area in turn).
pub(super) fn handle(v: &Value) -> Option<Handled> {
    Some(match v["cmd"].as_str().unwrap_or("") {
        // Terminals in the window (`terminal.rs`): open one, type into it, size it, end it.
        "pty_open" => {
            let (cols, rows) = (v["cols"].as_u64().unwrap_or(100) as u16, v["rows"].as_u64().unwrap_or(30) as u16);
            let kind = v["kind"].as_str().unwrap_or("");
            let key = v["key"].as_str().unwrap_or(kind).to_string();
            let opened = match kind {
                // The self-test's shell only (`window --selftest`): never in a normal window.
                "shell" if shell_allowed(SELFTEST.load(std::sync::atomic::Ordering::SeqCst)) => {
                    crate::terminal::open(&key, shell_command(), cols, rows).map(|id| (id, key.clone()))
                }
                // Claude in the data folder, on the profile picked (the default when none).
                "claude-folder" => crate::terminal::start_folder(&key, v["profile"].as_str().filter(|p| !p.is_empty()), v["session"].as_str(), cols, rows).map(|id| (id, key.clone())),
                // Claude on one call, guarded, on the call's own profile.
                "claude-call" => match call_named(v) {
                    Some(dir) => crate::terminal::start_call(&key, &dir, v["session"].as_str(), cols, rows).map(|id| (id, key.clone())),
                    None => Err(anyhow::anyhow!("that call can't be found")),
                },
                other => Err(anyhow::anyhow!("no terminal of kind {other:?}")),
            };
            let answer = match opened {
                Ok((id, key)) => json!({ "ok": true, "id": id, "key": key }),
                Err(e) => {
                    // A profile that needs a sign-in (or is being signed in): the page offers Sign in… for it.
                    let signin = e.downcast_ref::<crate::ai::SignInNeeded>().map(|s| s.profile.clone())
                        .or_else(|| e.downcast_ref::<crate::ai::SigningIn>().map(|s| s.profile.clone()));
                    json!({ "ok": false, "kind": kind, "key": key, "error": format!("{e:#}"), "signin": signin })
                }
            };
            Handled { refresh: false, reply: Some(format!("window.echo && window.echo.ptyOpened({answer})")) }
        }
        "pty_input" => {
            if let (Some(id), Some(data)) = (v["id"].as_u64(), v["data"].as_str()) {
                let _ = crate::terminal::write(id, data.as_bytes());
            }
            false.into()
        }
        "pty_paste" => {
            if let Some(id) = v["id"].as_u64() {
                let _ = crate::terminal::write(id, paste_bytes(clipboard_text(), v["bracketed"] == true).as_slice());
            }
            false.into()
        }
        "pty_resize" => {
            if let (Some(id), Some(c), Some(r)) = (v["id"].as_u64(), v["cols"].as_u64(), v["rows"].as_u64()) {
                let _ = crate::terminal::resize(id, c as u16, r as u16);
            }
            false.into()
        }
        "pty_close" => {
            if let Some(id) = v["id"].as_u64() {
                crate::terminal::close(id);
            }
            false.into()
        }
        _ => return None,
    })
}
