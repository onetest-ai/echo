//! A host half's HTTP through curl (moved from `jshost.rs`): `ctx.http.fetch` — one answer, a file handle as the body,
//! a big download into its store — and `ctx.http.stream` — an answer read as it comes (SSE, chunked), each piece an
//! event (`net.rs`). Both reach only what `reach` lets through; https to an address the user listed goes with the key
//! they pinned for it (`plugins::pins`) instead of the system's trust, and a certificate refused names its key
//! (`tls::explain`).

use std::io::Read as _;

use super::*;

/// An answer kept in memory: 10 MB, 60 s. Saved into the store, or a file sent: 512 MB, 15 minutes.
const SMALL: u64 = 10 << 20;
pub(super) const BIG: u64 = 512 << 20;

/// What a URL may be reached as: `listed` an address the user listed (`network.hosts_from`), else an `https://` host
/// the manifest names (`hosts`, and the fake service a development build may send it to).
pub(super) struct Reach {
    pub listed: bool,
    pub fake: Option<String>,
}

/// Whether plugin `id` may reach `url` at all: an address the user listed (each list checked again as when it was
/// saved: the file may have been edited since), or `https://` to a host its manifest names.
pub(super) fn reach(id: &str, url: &str) -> Result<Reach> {
    let m = plugins::read_manifest(&package_dir(id)?)?;
    let hosts: Vec<String> = m.uses.get("network").and_then(|v| v["hosts"].as_array().cloned()).unwrap_or_default().iter().filter_map(|h| h.as_str().map(str::to_owned)).collect();
    let listed: Vec<String> = plugins::address_settings(&m.uses).iter().filter_map(|k| plugins::listed_addresses(id, k)).collect();
    let granted: Vec<plugins::addresses::Address> = listed.iter().filter_map(|v| plugins::addresses::parse_list(v).ok()).flatten().collect();
    if granted.iter().any(|a| a.matches(url)) {
        return Ok(Reach { listed: true, fake: None });
    }
    let named = url.strip_prefix("https://").map(|r| r.split(['/', '?', '#']).next().unwrap_or(""));
    if !named.is_some_and(|h| !h.contains('@') && !h.contains(':') && hosts.iter().any(|x| x == h)) {
        if hosts.is_empty() && listed.is_empty() && m.uses.get("network").is_some_and(|n| n["ask"] == true) {
            bail!("{id} may reach only an address you type in Echo's box when it asks, once");
        }
        bail!("{id} may reach only the hosts its manifest names ({}) and the addresses you list in its settings ({})", hosts.join(", "), listed.join(", "));
    }
    // Checked on the real host above; a development build may send it to a fake service instead (or refuse it).
    Ok(Reach { listed: false, fake: test_host(url, &hosts)? })
}

/// A file the user handed the plugin, sent as a request's body: opened by Echo, its bytes given to curl's stdin as
/// they go (never in memory), exactly `size` of them.
pub(super) struct Upload {
    file: std::fs::File,
    size: u64,
}

/// The method, headers and body of a request as curl's config lines, each checked: a NUL would end a config line
/// early; a header name is a plain token (curl reads `@file` as a file). `file`: a handle the user gave the plugin,
/// sent as the body as it is (`Upload`: `upload-file = "-"`, so curl never adds the file's name to the URL's path as it
/// does to an empty one or one ending in `/`; its length Echo's, so it isn't sent in chunks); `body` text is sent as
/// written (`data-raw`; `data-binary` would read a body starting with `@` as a file).
fn request_lines(id: &str, opts: &Value) -> Result<(String, String, Option<Upload>)> {
    let method = opts["method"].as_str().unwrap_or("GET").to_ascii_uppercase();
    if !["GET", "POST", "PUT", "PATCH", "DELETE"].contains(&method.as_str()) {
        bail!("method {method}");
    }
    let headers = opts["headers"].as_object().cloned().unwrap_or_default();
    let body = opts.get("body").filter(|b| !b.is_null()).map(|b| b.as_str().map(str::to_owned).unwrap_or_else(|| b.to_string()));
    if body.as_deref().is_some_and(|b| b.contains('\0')) || headers.iter().any(|(k, v)| k.is_empty() || !k.bytes().all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c)) || v.as_str().unwrap_or("").contains(['\0', '\n', '\r'])) {
        bail!("a URL, header or body with a NUL, a line break in a header or a header name that isn't a plain word");
    }
    let mut cfg = String::new();
    for (k, v) in &headers {
        cfg.push_str(&format!("header = {}\n", curl_quote(&format!("{k}: {}", v.as_str().unwrap_or("")))));
    }
    let file = opts.get("file").filter(|f| !f.is_null());
    if file.is_some() && headers.keys().any(|k| k.eq_ignore_ascii_case("content-length") || k.eq_ignore_ascii_case("transfer-encoding")) {
        bail!("a file's Content-Length and Transfer-Encoding Echo sets itself");
    }
    let mut upload = None;
    match (file, body) {
        (Some(_), Some(_)) => bail!("a request sends a body or a file, not both"),
        (Some(f), None) => {
            if !["POST", "PUT", "PATCH"].contains(&method.as_str()) {
                bail!("a file goes with POST, PUT or PATCH, not {method}");
            }
            let (path, _) = files::own(id, f)?;
            let file = crate::grants::open_regular(&path)?;
            let size = file.metadata()?.len();
            if size > BIG {
                bail!("the file is {} MB: a plugin sends files up to {} MB", size.div_ceil(1 << 20), BIG >> 20);
            }
            // An upload sends `Expect: 100-continue` and waits for it; most servers never say it. From stdin curl
            // doesn't know the length and would send chunks: Echo says it, and no chunks.
            cfg.push_str(&format!("upload-file = \"-\"\nheader = \"Expect:\"\nheader = \"Transfer-Encoding:\"\nheader = \"Content-Length: {size}\"\n"));
            upload = Some(Upload { file, size });
        }
        (None, Some(body)) => cfg.push_str(&format!("data-raw = {}\n", curl_quote(&body))),
        (None, None) => {}
    }
    Ok((method, cfg, upload))
}

/// curl's config in a file of its own (only the user can read it: it holds the headers), gone once dropped — kept
/// until curl ends. For a request whose body comes on stdin.
struct CfgFile(PathBuf);

impl Drop for CfgFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// curl with config `cfg`, its answer on stdout, its words on stderr. `-q` first: the user's own ~/.curlrc (a
/// `location` that follows redirects) isn't read. The config on stdin; with an upload, the file's bytes go there and
/// the config in a `CfgFile` (keep it until curl ends).
fn curl(cfg: &str, upload: Option<Upload>) -> Result<(std::process::Child, Option<CfgFile>)> {
    use crate::app::NoWindow;
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    let Some(Upload { file, size }) = upload else {
        let mut child = Command::new("curl").no_window().args(["-q", "-sS", "-K", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        child.stdin.take().unwrap().write_all(cfg.as_bytes())?;
        return Ok((child, None));
    };
    let at = CfgFile(std::env::temp_dir().join(format!("echo-curl-{}-{}.cfg", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    o.open(&at.0)?.write_all(cfg.as_bytes())?;
    let mut child = Command::new("curl").no_window().args(["-q", "-sS", "-K"]).arg(&at.0).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let mut to = child.stdin.take().unwrap();
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut file.take(size), &mut to);
    });
    Ok((child, Some(at)))
}

/// The lines that pin the key the user trusts for an https address they listed: curl checks that key alone, not the
/// system's trust or the name (`pinnedpubkey` holds even with `insecure`). `None`: not such an address, or no pin.
fn pin_lines(id: &str, url: &str, reach: &Reach) -> Option<String> {
    let hp = (reach.listed && url.get(..8).is_some_and(|s| s.eq_ignore_ascii_case("https://"))).then(|| plugins::pins::host_port(url).ok()).flatten()?;
    let key = plugins::pins::pinned(id, &hp).filter(|k| plugins::pins::is_key(k))?;
    Some(format!("insecure\npinnedpubkey = {}\n", curl_quote(&key)))
}

/// curl failed on an https address the user listed with a certificate problem: what the key there is and how to trust
/// it (or that it changed), when Echo can tell; else curl's own words.
fn failed(id: &str, url: &str, reach: &Reach, code: Option<i32>, stderr: &str) -> anyhow::Error {
    let host = url.split_once("://").map_or("", |(_, r)| r).split(['/', '?', '#']).next().unwrap_or("");
    // 35 TLS handshake, 51 / 60 / 83 the certificate, 58 / 77 / 91 its files and status, 90 not the pinned key.
    let tls = matches!(code, Some(35 | 51 | 58 | 60 | 77 | 83 | 90 | 91));
    if tls && reach.listed && url.get(..8).is_some_and(|s| s.eq_ignore_ascii_case("https://")) {
        if let Some(e) = plugins::pins::host_port(url).ok().and_then(|hp| super::tls::probe(id, &hp)) {
            return e;
        }
    }
    anyhow!("{host} can't be reached: {}", stderr.trim())
}

/// `ctx.http.fetch`: HTTPS to a host the plugin's manifest names, or to an address the user listed in a setting its
/// manifest names in `network.hosts_from` (`plugins::addresses`: plain http too, to any listed host), or once to the
/// address the user just typed in Echo's box (`typed`, `uses.network.ask`: a plain GET, no headers or body), through
/// curl (its config on stdin, so headers and body never show on a command line). {status, body}; at most 10 MB, 60 s.
/// With `save: <path>` (relative, in the plugin's store) a 2xx answer's bytes go to that file — up to 512 MB in 15
/// minutes — and the answer is {status, body: "", saved, bytes}: the body is text, so audio and images can't come
/// through it. With `file: <handle>` that file is the body (up to 512 MB, 15 minutes).
pub(super) fn fetch(id: &str, url: &str, opts: &Value) -> Result<Value> {
    let reach = match reach(id, url) {
        Ok(r) => r,
        // An address the user just typed in Echo's box: once, a plain GET (`typed`).
        Err(e) => {
            let get = opts["method"].as_str().unwrap_or("GET").eq_ignore_ascii_case("GET");
            if !get || opts.get("body").is_some_and(|b| !b.is_null()) || opts["headers"].as_object().is_some_and(|h| !h.is_empty()) || !super::typed::take(id, url) {
                return Err(e);
            }
            Reach { listed: false, fake: None }
        }
    };
    let url = reach.fake.as_deref().unwrap_or(url);
    if url.contains('\0') {
        bail!("a URL, header or body with a NUL, a line break in a header or a header name that isn't a plain word");
    }
    /// The half-written download: its own name per fetch, gone however the fetch ends unless renamed into place.
    struct Part(PathBuf);
    impl Drop for Part {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let save = match opts["save"].as_str().filter(|s| !s.is_empty()) {
        Some(rel) => {
            let to = writable_store_path(id, rel)?;
            std::fs::create_dir_all(to.parent().unwrap_or(&to))?;
            let mut part = to.clone().into_os_string();
            part.push(format!(".{}-{}.part", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            Some((rel, to, Part(PathBuf::from(part))))
        }
        None => None,
    };
    let (method, lines, upload) = request_lines(id, opts)?;
    let big = save.is_some() || upload.is_some();
    let mut cfg = curl_head(url, &method, reach.fake.is_some());
    if big {
        cfg = cfg.replace("max-time = 60\n", "max-time = 900\n").replace(&format!("max-filesize = {SMALL}\n"), &format!("max-filesize = {BIG}\n"));
    }
    cfg.push_str(&pin_lines(id, url, &reach).unwrap_or_default());
    if let Some((_, _, part)) = &save {
        cfg.push_str(&format!("output = {}\n", curl_quote(&part.0.to_string_lossy())));
    }
    cfg.push_str(&lines);
    let (child, _cfg) = curl(&cfg, upload)?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(failed(id, url, &reach, out.status.code(), &String::from_utf8_lossy(&out.stderr)));
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", &text));
    let status = code.trim().parse::<u16>().unwrap_or(0);
    let Some((rel, to, part)) = save else { return Ok(json!({"status": status, "body": body})) };
    if (200..300).contains(&status) {
        // An empty answer may leave curl's file unmade.
        if !part.0.exists() {
            std::fs::write(&part.0, b"")?;
        }
        let bytes = std::fs::metadata(&part.0).map(|m| m.len()).unwrap_or(0);
        std::fs::rename(&part.0, &to)?;
        return Ok(json!({"status": status, "body": "", "saved": rel, "bytes": bytes}));
    }
    // Not a success: what the server said (text, cut), not a file.
    let said = std::fs::read(&part.0).map(|b| String::from_utf8_lossy(&b[..b.len().min(4096)]).into_owned()).unwrap_or_default();
    Ok(json!({"status": status, "body": said}))
}

/// A value in curl's config: quoted, its backslashes, quotes and line breaks escaped.
pub(super) fn curl_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r"))
}

/// The first lines of curl's config for `ctx.http.fetch`. Only http(s), never a redirect elsewhere: what the plugin may
/// reach is checked on this URL alone. A request sent to a fake service never goes through a proxy (`http_proxy`,
/// `all_proxy`): it stays on this computer, its headers with it.
pub(super) fn curl_head(url: &str, method: &str, to_fake: bool) -> String {
    let mut cfg = format!("url = {}\nrequest = \"{method}\"\nproto = \"=http,https\"\ngloboff\nmax-time = 60\nmax-filesize = {SMALL}\nwrite-out = \"\\n%{{http_code}}\"\n", curl_quote(url));
    if to_fake {
        cfg.push_str("noproxy = \"*\"\n");
    }
    cfg
}

/// `ctx.http.stream(url, opts, onChunk)`: the same checks as `fetch`, the answer read as it comes on a thread of its
/// own — `head` {status, headers} once, then `chunk` {text} pieces (UTF-8 never cut inside a character), then `end`
/// {bytes, cancelled?} or `error` {message}. No time limit (an event stream stays open), 512 MB at most; `cancel` ends
/// it. Counted with the plugin's WebSockets (`net::MOST`).
pub(super) fn stream(id: &str, url: &str, opts: &Value) -> Result<Value> {
    let reach = reach(id, url)?;
    let target = reach.fake.clone().unwrap_or_else(|| url.to_string());
    if target.contains('\0') {
        bail!("a URL, header or body with a NUL, a line break in a header or a header name that isn't a plain word");
    }
    let (method, lines, upload) = request_lines(id, opts)?;
    let mut cfg = curl_head(&target, &method, reach.fake.is_some()).replace("max-time = 60\n", "").replace("write-out = \"\\n%{http_code}\"\n", "");
    cfg = cfg.replace(&format!("max-filesize = {SMALL}\n"), &format!("max-filesize = {BIG}\nconnect-timeout = 15\ninclude\nno-buffer\n"));
    cfg.push_str(&pin_lines(id, &target, &reach).unwrap_or_default());
    cfg.push_str(&lines);
    let conn = super::net::reserve(id)?;
    let (mut child, cfg_file) = match curl(&cfg, upload) {
        Ok(c) => c,
        Err(e) => {
            super::net::done(conn);
            return Err(e);
        }
    };
    let (mut out, mut err) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
    let child = Arc::new(Mutex::new(Some(child)));
    if !super::net::keep_child(conn, child.clone()) {
        // Its plugin went meanwhile: killed by `keep_child`, reaped here.
        let _ = child.lock().ok().and_then(|mut c| c.take()).map(|mut c| c.wait());
        super::net::done(conn);
        bail!("that connection is closed");
    }
    let (id, url) = (id.to_string(), url.to_string());
    std::thread::spawn(move || {
        let _cfg_file = cfg_file;
        let said = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = err.read_to_string(&mut s);
            s
        });
        let emit = |ev: Value| super::net::emit(&id, conn, ev);
        let mut head: Vec<u8> = vec![];
        let mut rest: Vec<u8> = vec![];
        let mut headed = false;
        let mut bytes: u64 = 0;
        let mut too_big = false;
        let mut buf = vec![0u8; 64 << 10];
        loop {
            let n = match out.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            if !headed {
                head.extend_from_slice(&buf[..n]);
                // Header blocks until the answer's own: a 1xx, or a proxy's "Connection established", comes first.
                while let Some(end) = head.windows(4).position(|w| w == b"\r\n\r\n") {
                    let block = String::from_utf8_lossy(&head[..end]).into_owned();
                    let body = head.split_off(end + 4);
                    head = body;
                    let status = block.split_whitespace().nth(1).and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
                    if (100..200).contains(&status) || block.lines().next().is_some_and(|l| l.to_ascii_lowercase().contains("connection established")) {
                        continue;
                    }
                    let headers: serde_json::Map<String, Value> = block.lines().skip(1).filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_ascii_lowercase(), json!(v.trim()))).collect();
                    emit(json!({"kind": "head", "status": status, "headers": headers}));
                    headed = true;
                    break;
                }
                if !headed {
                    continue;
                }
                rest = std::mem::take(&mut head);
            } else {
                rest.extend_from_slice(&buf[..n]);
            }
            // Whole characters only: an incomplete one at the end waits for the next piece.
            let cut = match std::str::from_utf8(&rest) {
                Ok(_) => rest.len(),
                Err(e) if e.error_len().is_none() => e.valid_up_to(),
                Err(_) => rest.len(),
            };
            let tail = rest.split_off(cut);
            if !rest.is_empty() {
                bytes += rest.len() as u64;
                emit(json!({"kind": "chunk", "text": String::from_utf8_lossy(&rest)}));
            }
            rest = tail;
            if bytes > BIG {
                too_big = true;
                if let Some(c) = child.lock().ok().as_mut().and_then(|c| c.as_mut()) {
                    let _ = c.kill();
                }
                break;
            }
            super::net::wait_backlog(conn);
        }
        let cancelled = !too_big && !super::net::is_open(conn);
        let status = child.lock().ok().and_then(|mut c| c.take()).and_then(|mut c| c.wait().ok());
        let said = said.join().unwrap_or_default();
        if too_big {
            emit(json!({"kind": "error", "message": format!("the answer passed {} MB: a stream is {} MB at most", BIG >> 20, BIG >> 20)}));
        } else if cancelled || status.is_some_and(|s| s.success()) {
            emit(json!({"kind": "end", "bytes": bytes, "cancelled": cancelled}));
        } else {
            let e = failed(&id, &url, &reach, status.and_then(|s| s.code()), &said);
            emit(json!({"kind": "error", "message": format!("{e:#}")}));
        }
        super::net::done(conn);
    });
    Ok(json!(conn))
}
