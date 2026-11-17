//! A host half's open connections: `ctx.net.websocket` (here) and `ctx.http.stream` (`http.rs`). Each runs on a thread
//! of its own and reaches the plugin's JavaScript only as events (`Msg::Net` → `__net`), one slice each, so nothing
//! waits inside the 5 s slice. Only to the addresses the user listed (a WebSocket) or what `http::reach` lets through
//! (a stream); a plugin keeps `MOST` open at once, a message is `MAX_MESSAGE` at most, and a plugin switched off,
//! updated or removed loses them all (`stop_plugin`).

use std::collections::BTreeMap;
use std::io::Read as _;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::{Receiver, TryRecvError};

use super::*;

/// Connections (WebSockets and streams) a plugin keeps open at once.
pub(super) const MOST: usize = 8;
/// The largest WebSocket message, either way.
pub(super) const MAX_MESSAGE: usize = 16 << 20;
/// Events of one connection the plugin hasn't been given yet: past this, its thread waits before reading more.
const BACKLOG: usize = 256;

pub(super) enum Out {
    Text(String),
    Binary(Vec<u8>),
    Close(u16, String),
}

type Child = Arc<Mutex<Option<std::process::Child>>>;

struct Conn {
    plugin: String,
    /// Its JavaScript still wants it: false once closed or cancelled (its thread ends it).
    open: bool,
    out: Option<Sender<Out>>,
    child: Option<Child>,
    backlog: Arc<AtomicUsize>,
}

static CONNS: Mutex<BTreeMap<u64, Conn>> = Mutex::new(BTreeMap::new());

fn conns() -> std::sync::MutexGuard<'static, BTreeMap<u64, Conn>> {
    CONNS.lock().unwrap_or_else(|e| e.into_inner())
}

/// A new connection of plugin `plugin`, when it has fewer than `MOST`.
pub(super) fn reserve(plugin: &str) -> Result<u64> {
    let mut all = conns();
    if all.values().filter(|c| c.plugin == plugin).count() >= MOST {
        bail!("{plugin} has {MOST} connections open: close one first");
    }
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    all.insert(n, Conn { plugin: plugin.into(), open: true, out: None, child: None, backlog: Arc::new(AtomicUsize::new(0)) });
    Ok(n)
}

/// The curl behind a stream, killed on `cancel` or `stop_plugin`.
pub(super) fn keep_child(n: u64, child: Child) {
    if let Some(c) = conns().get_mut(&n) {
        c.child = Some(child);
    }
}

fn kill(child: &Option<Child>) {
    if let Some(c) = child {
        if let Ok(mut g) = c.lock() {
            if let Some(p) = g.as_mut() {
                let _ = p.kill();
            }
        }
    }
}

/// Still wanted by its plugin.
pub(super) fn is_open(n: u64) -> bool {
    conns().get(&n).is_some_and(|c| c.open)
}

/// Its thread is done: it no longer counts.
pub(super) fn done(n: u64) {
    conns().remove(&n);
}

/// An event for the plugin's JavaScript (`__net(n, json)`).
pub(super) fn emit(plugin: &str, n: u64, ev: Value) {
    if let Some(b) = conns().get(&n).map(|c| c.backlog.clone()) {
        b.fetch_add(1, Ordering::Relaxed);
    }
    let _ = host().tx.send(Msg::Net(plugin.into(), n, ev.to_string()));
}

/// The host gave the plugin one of `n`'s events.
pub(super) fn delivered(n: u64) {
    if let Some(c) = conns().get(&n) {
        // Only the host thread takes away (workers add): never below zero.
        if c.backlog.load(Ordering::Relaxed) > 0 {
            c.backlog.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

/// Wait while the plugin is behind on `n`'s events (and still wants it).
pub(super) fn wait_backlog(n: u64) {
    while conns().get(&n).is_some_and(|c| c.open && c.backlog.load(Ordering::Relaxed) > BACKLOG) {
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Plugin `plugin` was switched off, updated or removed (or loads again): every connection of its goes — a WebSocket
/// closed, a stream's curl killed — and none of their events reaches it.
pub fn stop_plugin(plugin: &str) {
    let mut all = conns();
    let gone: Vec<u64> = all.iter().filter(|(_, c)| c.plugin == plugin).map(|(n, _)| *n).collect();
    for n in gone {
        if let Some(c) = all.remove(&n) {
            kill(&c.child);
        }
    }
}

/// The open connections of each plugin (tests).
#[cfg(test)]
pub(super) fn open_of(plugin: &str) -> usize {
    conns().values().filter(|c| c.plugin == plugin).count()
}

fn own(plugin: &str, n: u64) -> Result<()> {
    match conns().get(&n) {
        Some(c) if c.plugin == plugin && c.open => Ok(()),
        _ => bail!("that connection is closed"),
    }
}

/// The host's quick services for connections (`__host`): open one (its id; the rest comes as events), send, close,
/// cancel.
pub(super) fn service(id: &str, op: &str, arg: &Value) -> Result<Value> {
    match op {
        "net.websocket" => websocket(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "http.stream" => super::http::stream(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "net.send" => {
            let n = arg[0].as_u64().unwrap_or(0);
            own(id, n)?;
            let msg = if arg[1].is_object() {
                // A file the user handed it, sent as one binary message.
                let (path, _) = files::own(id, &arg[1])?;
                let mut buf = vec![];
                crate::grants::open_regular(&path)?.take(MAX_MESSAGE as u64 + 1).read_to_end(&mut buf)?;
                Out::Binary(buf)
            } else if arg[2]["binary"].as_bool() == Some(true) {
                use base64::Engine as _;
                Out::Binary(base64::engine::general_purpose::STANDARD.decode(arg[1].as_str().unwrap_or("")).map_err(|_| anyhow!("a binary message is sent as base64"))?)
            } else {
                Out::Text(arg[1].as_str().map(str::to_owned).unwrap_or_else(|| arg[1].to_string()))
            };
            let size = match &msg {
                Out::Text(t) => t.len(),
                Out::Binary(b) => b.len(),
                Out::Close(..) => 0,
            };
            if size > MAX_MESSAGE {
                bail!("a message is {} MB at most", MAX_MESSAGE >> 20);
            }
            conns().get(&n).and_then(|c| c.out.clone()).ok_or_else(|| anyhow!("that connection isn't a WebSocket"))?.send(msg).map_err(|_| anyhow!("that connection is closed"))?;
            Ok(Value::Null)
        }
        "net.close" => {
            let n = arg[0].as_u64().unwrap_or(0);
            let code = arg[1].as_u64().filter(|c| *c == 1000 || (3000..5000).contains(c)).unwrap_or(1000) as u16;
            let reason: String = arg[2].as_str().unwrap_or("").chars().take(100).collect();
            if let Some(c) = conns().get_mut(&n).filter(|c| c.plugin == id) {
                if let Some(out) = &c.out {
                    let _ = out.send(Out::Close(code, reason));
                }
            }
            Ok(Value::Null)
        }
        "http.cancel" => {
            let n = arg.as_u64().unwrap_or(0);
            let child = conns().get_mut(&n).filter(|c| c.plugin == id).map(|c| {
                c.open = false;
                c.child.clone()
            });
            if let Some(child) = child {
                kill(&child);
            }
            Ok(Value::Null)
        }
        o => bail!("Echo has no service “{o}”"),
    }
}

/// `ctx.net.websocket(url, {headers, protocols})`: `ws://` / `wss://` to an address the user listed (its http / https
/// address), checked here; connected on a thread of its own. Its id at once; then `open` {protocol} or `error`
/// {message}, `message` {data, binary?} (binary as base64), `close` {code, reason}.
fn websocket(id: &str, url: &str, opts: &Value) -> Result<Value> {
    let lower = url.to_ascii_lowercase();
    let (tls, web) = match (lower.strip_prefix("wss://"), lower.strip_prefix("ws://")) {
        (Some(_), _) => (true, format!("https://{}", &url[6..])),
        (_, Some(_)) => (false, format!("http://{}", &url[5..])),
        _ => bail!("a WebSocket's address starts with ws:// or wss://"),
    };
    if !super::http::reach(id, &web)?.listed {
        bail!("{id} opens a WebSocket only to the addresses you list in its settings");
    }
    let auth = web.split_once("://").map_or("", |(_, r)| r).split(['/', '?', '#']).next().unwrap_or("").to_string();
    let hp = plugins::pins::host_port(&format!("{}://{auth}", if tls { "https" } else { "http" }))?;
    let (host, port) = hp.rsplit_once(':').map(|(h, p)| (h.to_string(), p.parse::<u16>().unwrap_or(0))).unwrap_or_default();
    let port = if auth.contains(':') { port } else if tls { 443 } else { 80 };
    use tungstenite::client::IntoClientRequest as _;
    let mut req = url.into_client_request().map_err(|e| anyhow!("“{url}”: {e}"))?;
    let h = req.headers_mut();
    for (k, v) in opts["headers"].as_object().into_iter().flatten() {
        let lk = k.to_ascii_lowercase();
        if ["host", "upgrade", "connection", "content-length"].contains(&lk.as_str()) || lk.starts_with("sec-websocket-") {
            bail!("the header “{k}” is the WebSocket's own");
        }
        let name = tungstenite::http::HeaderName::from_bytes(k.as_bytes()).map_err(|_| anyhow!("“{k}” isn't a header name"))?;
        let value = tungstenite::http::HeaderValue::from_str(v.as_str().unwrap_or("")).map_err(|_| anyhow!("the header “{k}” has a line break or a character a header can't hold"))?;
        h.append(name, value);
    }
    let protocols: Vec<&str> = opts["protocols"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if !protocols.is_empty() {
        if !protocols.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))) {
            bail!("a WebSocket protocol is a plain word");
        }
        h.insert("sec-websocket-protocol", tungstenite::http::HeaderValue::from_str(&protocols.join(", "))?);
    }
    let n = reserve(id)?;
    let (tx, rx) = channel();
    if let Some(c) = conns().get_mut(&n) {
        c.out = Some(tx);
    }
    let plugin = id.to_string();
    std::thread::spawn(move || {
        run_ws(&plugin, n, req, &host, port, tls, rx);
        done(n);
    });
    Ok(json!(n))
}

fn run_ws(plugin: &str, n: u64, req: tungstenite::handshake::client::Request, host: &str, port: u16, tls: bool, rx: Receiver<Out>) {
    use tungstenite::protocol::frame::coding::CloseCode;
    use tungstenite::protocol::{frame::CloseFrame, WebSocketConfig};
    use tungstenite::{Error as WsError, Message};
    let emit = |ev: Value| emit(plugin, n, ev);
    let fail = |message: String| {
        emit(json!({"kind": "error", "message": message}));
        emit(json!({"kind": "close", "code": 1006, "reason": ""}));
    };
    let stream = if tls { super::tls::connect(plugin, host, port, true).map(|s| super::tls::Stream::Tls(Box::new(s))) } else { super::tls::tcp(host, port).map(super::tls::Stream::Plain) };
    let stream = match stream {
        Ok(s) => s,
        Err(e) => return fail(format!("{e:#}")),
    };
    let config = WebSocketConfig::default().max_message_size(Some(MAX_MESSAGE)).max_frame_size(Some(MAX_MESSAGE));
    let (mut ws, answer) = match tungstenite::client::client_with_config(req, stream, Some(config)) {
        Ok(x) => x,
        Err(tungstenite::HandshakeError::Failure(e)) => return fail(format!("{host}:{port} refused the WebSocket: {e}")),
        Err(tungstenite::HandshakeError::Interrupted(_)) => return fail(format!("{host}:{port} didn't answer the WebSocket in time")),
    };
    let protocol = answer.headers().get("sec-websocket-protocol").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    // Reading waits a moment at most, so what the plugin sends goes out between reads.
    let _ = ws.get_ref().tcp().set_read_timeout(Some(Duration::from_millis(30)));
    emit(json!({"kind": "open", "protocol": protocol}));
    let mut closed_by: Option<(u16, String)> = None;
    loop {
        loop {
            let m = match rx.try_recv() {
                Ok(Out::Text(t)) => Message::text(t),
                Ok(Out::Binary(b)) => Message::binary(b),
                Ok(Out::Close(code, reason)) => {
                    let _ = ws.close(Some(CloseFrame { code: CloseCode::from(code), reason: reason.into() }));
                    break;
                }
                Err(TryRecvError::Empty) => break,
                // Its plugin went: closed without a word to it.
                Err(TryRecvError::Disconnected) => {
                    let _ = ws.close(None);
                    let _ = ws.flush();
                    return;
                }
            };
            if let Err(e) = ws.send(m) {
                if !matches!(&e, WsError::Io(io) if matches!(io.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)) {
                    return fail(format!("{e}"));
                }
            }
        }
        wait_backlog(n);
        match ws.read() {
            Ok(Message::Text(t)) => emit(json!({"kind": "message", "data": t.as_str()})),
            Ok(Message::Binary(b)) => {
                use base64::Engine as _;
                emit(json!({"kind": "message", "data": base64::engine::general_purpose::STANDARD.encode(&b), "binary": true}));
            }
            Ok(Message::Close(f)) => closed_by = Some(f.map_or((1005, String::new()), |f| (u16::from(f.code), f.reason.as_str().to_string()))),
            Ok(_) => {}
            Err(WsError::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                let _ = ws.flush();
            }
            Err(WsError::ConnectionClosed | WsError::AlreadyClosed) => {
                let (code, reason) = closed_by.unwrap_or((1000, String::new()));
                return emit(json!({"kind": "close", "code": code, "reason": reason}));
            }
            Err(e) => return fail(format!("{e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::net::TcpListener;

    const CERT_A: &str = include_str!("../../../../tests/fixtures/certs/voice-a.pem");
    const KEY_A: &str = include_str!("../../../../tests/fixtures/certs/voice-a.key");
    const CERT_B: &str = include_str!("../../../../tests/fixtures/certs/voice-b.pem");
    const KEY_B: &str = include_str!("../../../../tests/fixtures/certs/voice-b.key");
    /// Certificate A's key as `openssl x509 -pubkey | openssl pkey -pubin -outform der | openssl dgst -sha256` gives it.
    const KEY_OF_A: &str = "sha256//Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc=";

    fn der(pem: &str) -> Vec<u8> {
        use base64::Engine as _;
        let b64: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
        base64::engine::general_purpose::STANDARD.decode(b64).unwrap()
    }

    fn server_config(cert: &str, key: &str) -> Arc<rustls::ServerConfig> {
        use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let c = rustls::ServerConfig::builder_with_provider(provider).with_safe_default_protocol_versions().unwrap().with_no_client_auth();
        Arc::new(c.with_single_cert(vec![CertificateDer::from(der(cert))], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(der(key)))).unwrap())
    }

    /// The certificate a test server shows now: A, or B once switched.
    type Shows = Arc<Mutex<Arc<rustls::ServerConfig>>>;

    trait Io: std::io::Read + std::io::Write + Send {}
    impl<T: std::io::Read + std::io::Write + Send> Io for T {}

    /// A server on 127.0.0.1, plain or with TLS (`shows`), each connection to `handle` on a thread of its own.
    fn serve(shows: Option<Shows>, handle: impl Fn(Box<dyn Io>) + Send + Sync + 'static) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = Arc::new(handle);
        std::thread::spawn(move || {
            for s in listener.incoming().flatten() {
                let (shows, handle) = (shows.clone(), handle.clone());
                std::thread::spawn(move || {
                    let io: Box<dyn Io> = match shows {
                        Some(cfg) => {
                            let cfg = cfg.lock().unwrap().clone();
                            Box::new(rustls::StreamOwned::new(rustls::ServerConnection::new(cfg).unwrap(), s))
                        }
                        None => Box::new(s),
                    };
                    handle(io)
                });
            }
        });
        port
    }

    /// A made-up voice server: JSON-RPC over WebSocket (each request answered with its method and params), a binary
    /// message echoed, "bye" closed with 4001; each connection and its end a line in the log.
    fn voice_server(shows: Option<Shows>) -> (u16, Arc<Mutex<Vec<String>>>) {
        let log = Arc::new(Mutex::new(vec![]));
        let l = log.clone();
        let port = serve(shows, move |io| {
            let Ok(mut ws) = tungstenite::accept(io) else { return l.lock().unwrap().push("refused".into()) };
            l.lock().unwrap().push("ws".into());
            loop {
                match ws.read() {
                    Ok(tungstenite::Message::Text(t)) if t.as_str() == "bye" => {
                        let _ = ws.close(Some(tungstenite::protocol::CloseFrame { code: 4001.into(), reason: "done".into() }));
                    }
                    Ok(tungstenite::Message::Text(t)) => {
                        let v: Value = serde_json::from_str(t.as_str()).unwrap_or_default();
                        let _ = ws.send(tungstenite::Message::text(json!({"jsonrpc": "2.0", "id": v["id"], "result": {"method": v["method"], "params": v["params"]}}).to_string()));
                    }
                    Ok(tungstenite::Message::Binary(b)) => {
                        l.lock().unwrap().push(format!("binary {}", b.len()));
                        let _ = ws.send(tungstenite::Message::Binary(b));
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            l.lock().unwrap().push("gone".into());
        });
        (port, log)
    }

    /// The request's head and, with a content-length, its body.
    fn request(io: &mut dyn Io) -> (String, Vec<u8>) {
        let mut head = vec![];
        let mut b = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") && io.read(&mut b).unwrap_or(0) == 1 {
            head.push(b[0]);
        }
        let head = String::from_utf8_lossy(&head).into_owned();
        let len = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok())).unwrap_or(0);
        let mut body = vec![0u8; len];
        let _ = io.read_exact(&mut body);
        (head, body)
    }

    /// A made-up web server: `/events` an event stream in three pieces (one cut inside a Cyrillic letter), `/forever`
    /// a tick every 50 ms until the client goes (then "gone" in the log), `/big` 12 MB, `/upload` the body's size and
    /// sha256, anything else "secure".
    fn web_server(shows: Option<Shows>) -> (u16, Arc<Mutex<Vec<String>>>) {
        let log = Arc::new(Mutex::new(vec![]));
        let l = log.clone();
        let port = serve(shows, move |mut io| {
            let (head, body) = request(&mut *io);
            let path = head.split_whitespace().nth(1).unwrap_or("").to_string();
            let chunk = |io: &mut Box<dyn Io>, b: &[u8]| {
                write!(io, "{:x}\r\n", b.len()).and_then(|_| io.write_all(b)).and_then(|_| io.write_all(b"\r\n")).and_then(|_| io.flush())
            };
            match path.as_str() {
                "/events" => {
                    let _ = write!(io, "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n");
                    let text = "data: one\n\ndata: два\n\n".as_bytes();
                    let cut = text.len() - 4; // inside "а"
                    for piece in [&text[..cut], &text[cut..], b"data: three\n\n".as_slice()] {
                        let _ = chunk(&mut io, piece);
                        std::thread::sleep(Duration::from_millis(60));
                    }
                    let _ = io.write_all(b"0\r\n\r\n");
                }
                "/forever" => {
                    let _ = write!(io, "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n");
                    while chunk(&mut io, b"data: tick\n\n").is_ok() {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    l.lock().unwrap().push("gone".into());
                }
                "/big" => {
                    let big = vec![b'x'; 12 << 20];
                    let _ = write!(io, "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", big.len());
                    let _ = io.write_all(&big);
                }
                p => {
                    use sha2::Digest as _;
                    let said = if p == "/upload" { format!("{} {:x}", body.len(), sha2::Sha256::digest(&body)) } else { "secure".into() };
                    if !head.is_empty() {
                        l.lock().unwrap().push(format!("{} {p}", head.split_whitespace().next().unwrap_or("")));
                    }
                    let _ = write!(io, "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{said}", said.len());
                }
            }
            let _ = io.flush();
        });
        (port, log)
    }

    const MAIN: &str = r#"
    export function apply(ctx) {
      ctx.command('ping', () => 'pong')
      let ws = null
      const got = []
      ctx.command('open', async (a) => {
        ws = await ctx.net.websocket(a.url, a.opts || {})
        ws.onMessage = (d, o) => got.push(o.binary ? 'binary ' + d : d)
        ws.onClose = (c) => got.push('close ' + c.code + ' ' + c.reason)
        return ws.protocol
      })
      ctx.command('try', async (a) => { try { await ctx.net.websocket(a.url); return 'open' } catch (e) { return e.message } })
      ctx.command('send', (a) => { ws.send(a.data, a.opts); return true })
      ctx.command('sendFile', (a, { files }) => { ws.send(files[0]); return true })
      ctx.command('got', () => got.splice(0))
      ctx.command('close', () => { ws.close(4000, 'bye'); return true })
      ctx.command('stream', (a) => new Promise((res) => {
        const parts = []
        const s = ctx.http.stream(a.url, a.opts || {}, (t) => { parts.push(t); if (a.cancelAfter && parts.length === a.cancelAfter) s.cancel() })
        s.done.then((d) => res({ status: s.status, type: s.headers['content-type'], text: parts.join(''), done: d }), (e) => res({ error: e.message }))
      }))
      ctx.command('get', async (a) => await ctx.http.fetch(a.url, a.opts || {}))
      ctx.command('upload', async (a, { files }) => await ctx.http.fetch(a.url, { method: 'PUT', file: files[0] }))
      ctx.command('pinIt', () => ctx.store.write('pins.json', '{"pins":{"127.0.0.1:1":{"key":"sha256//x"}}}'))
    }
    "#;

    /// Plugin `netty` installed and on, its setting `voice` listing what the user enters.
    fn install(root: &Path) {
        let mk = root.join("market");
        let d = mk.join("plugins/netty");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("echo-plugin.json"),
            json!({"id": "netty", "name": "Netty", "version": "0.1.0", "main": "main.js",
                   "uses": {"network": {"hosts": ["api.acme.example"], "hosts_from": ["voice"]}},
                   "settings": [{"key": "voice", "label": "Voice server", "type": "string"}]})
            .to_string(),
        )
        .unwrap();
        std::fs::write(d.join("main.js"), MAIN).unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "netty", "source": "./plugins/netty"}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        let agreed = plugins::review("netty", Some(&name)).unwrap().uses;
        plugins::install("netty", Some(&name), &agreed).unwrap();
        sync();
        paths::testenv::eventually("netty loads", || call("netty", "ping", &json!({})).is_ok());
    }

    fn cmd(c: &str, args: Value) -> Value {
        call("netty", c, &args).unwrap_or_else(|e| panic!("{c}: {e:#}"))
    }

    /// What the plugin heard, once it holds `n` lines.
    fn heard(n: usize) -> Vec<String> {
        let mut all = vec![];
        let t = Instant::now();
        while all.len() < n && t.elapsed() < Duration::from_secs(10) {
            all.extend(cmd("got", json!({})).as_array().unwrap().iter().map(|v| v.as_str().unwrap_or("").to_string()));
            std::thread::sleep(Duration::from_millis(20));
        }
        all
    }

    #[test]
    fn a_certificates_key_is_read_as_curl_and_openssl_pin_it() {
        assert_eq!(super::super::tls::key_of(&rustls::pki_types::CertificateDer::from(der(CERT_A))).as_deref(), Some(KEY_OF_A));
        assert_ne!(super::super::tls::key_of(&rustls::pki_types::CertificateDer::from(der(CERT_B))).as_deref(), Some(KEY_OF_A));
    }

    #[test]
    fn a_plugin_talks_json_rpc_over_a_websocket_only_to_an_address_the_user_listed_and_loses_it_when_switched_off() {
        let (_env, root) = paths::testenv::folders("jshost-ws");
        install(&root);
        let (port, log) = voice_server(None);
        let url = format!("ws://127.0.0.1:{port}/rpc");
        // Nothing listed: refused before anything is sent; nor to a host the manifest names (https only, no WebSocket).
        assert!(cmd("try", json!({"url": url})).as_str().unwrap().contains("may reach only"));
        assert!(cmd("try", json!({"url": "wss://api.acme.example/rpc"})).as_str().unwrap().contains("only to the addresses you list"));
        assert!(cmd("try", json!({"url": format!("http://127.0.0.1:{port}/")})).as_str().unwrap().contains("ws:// or wss://"));
        assert!(log.lock().unwrap().is_empty(), "nothing reached the server");
        plugins::set_setting("netty", "voice", &json!(format!("http://127.0.0.1:{port}"))).unwrap();
        assert!(cmd("try", json!({"url": format!("ws://127.0.0.1:{}/rpc", port.wrapping_add(1))})).as_str().unwrap().contains("may reach only"), "another port");
        // Listed: open, a request answered, a binary message both ways, the server's close heard with its code.
        cmd("open", json!({"url": url}));
        cmd("send", json!({"data": json!({"jsonrpc": "2.0", "id": 7, "method": "speak", "params": {"text": "привіт"}}).to_string()}));
        let answer: Value = serde_json::from_str(&heard(1)[0]).unwrap();
        assert_eq!(answer, json!({"jsonrpc": "2.0", "id": 7, "result": {"method": "speak", "params": {"text": "привіт"}}}));
        cmd("send", json!({"data": "AAEC/w==", "opts": {"binary": true}}));
        assert_eq!(heard(1), ["binary AAEC/w=="]);
        // A file the user handed it goes as one binary message.
        let f = root.join("clip.bin");
        std::fs::write(&f, [1u8, 2, 3, 4, 5]).unwrap();
        let h = files::make("netty", &f).unwrap();
        call_with("netty", "sendFile", &json!({}), &[h], CALL_TIMEOUT).unwrap();
        assert_eq!(heard(1), ["binary AQIDBAU="]);
        // Too big a message is refused at send, nothing sent.
        let big = root.join("big.bin");
        std::fs::write(&big, vec![0u8; MAX_MESSAGE + 1]).unwrap();
        let e = call_with("netty", "sendFile", &json!({}), &[files::make("netty", &big).unwrap()], CALL_TIMEOUT).unwrap_err().to_string();
        assert!(e.contains("16 MB at most"), "{e}");
        cmd("send", json!({"data": "bye"}));
        assert_eq!(heard(1), ["close 4001 done"]);
        assert!(call("netty", "send", &json!({"data": "late"})).unwrap_err().to_string().contains("closed"));
        // At most 8 open at once; switched off, every one goes.
        for _ in 0..MOST {
            cmd("open", json!({"url": url}));
        }
        assert!(cmd("try", json!({"url": url})).as_str().unwrap().contains("has 8 connections open"));
        assert_eq!(open_of("netty"), MOST);
        let before = log.lock().unwrap().iter().filter(|l| *l == "gone").count();
        plugins::set_enabled("netty", false).unwrap();
        sync();
        paths::testenv::eventually("its connections closed", || open_of("netty") == 0 && log.lock().unwrap().iter().filter(|l| *l == "gone").count() == before + MOST);
        plugins::remove("netty", false).unwrap();
        sync();
    }

    #[test]
    fn a_certificate_the_system_doesnt_trust_is_reached_only_once_the_user_trusts_its_key_and_refused_when_it_changes() {
        let (_env, root) = paths::testenv::folders("jshost-pins");
        install(&root);
        let shows: Shows = Arc::new(Mutex::new(server_config(CERT_A, KEY_A)));
        let (wss, ws_log) = voice_server(Some(shows.clone()));
        let (https, web_log) = web_server(Some(shows.clone()));
        plugins::set_setting("netty", "voice", &json!(format!("https://127.0.0.1:{wss}, https://127.0.0.1:{https}"))).unwrap();
        let ws_url = format!("wss://127.0.0.1:{wss}/rpc");
        let web_url = format!("https://127.0.0.1:{https}/hello");
        // First time: refused, naming the key; Echo keeps it as met, for the user.
        let first = cmd("try", json!({"url": ws_url}));
        assert!(first.as_str().unwrap().contains("isn't one this computer trusts") && first.as_str().unwrap().contains(KEY_OF_A), "{first}");
        let e = call("netty", "get", &json!({"url": web_url})).unwrap_err().to_string();
        assert!(e.contains("isn't one this computer trusts") && e.contains(KEY_OF_A), "{e}");
        assert!(ws_log.lock().unwrap().iter().all(|l| l != "ws") && web_log.lock().unwrap().is_empty(), "nothing went over");
        let met = plugins::pins::view("netty");
        assert_eq!(met.len(), 2);
        assert!(met.iter().all(|c| c["seen"] == KEY_OF_A && c["pinned"].is_null()), "{met:?}");
        // The plugin can't pin: its store's files aren't where pins are kept.
        let _ = call("netty", "pinIt", &json!({}));
        assert!(cmd("try", json!({"url": ws_url})).as_str().unwrap().contains("isn't one this computer trusts"));
        // Only an https address the user listed takes a pin.
        assert!(plugins::pins::trust("netty", "127.0.0.1:1", None).unwrap_err().to_string().contains("isn't an https address you listed"));
        assert!(plugins::pins::trust("netty", &format!("127.0.0.1:{wss}"), Some("sha256//short")).unwrap_err().to_string().contains("isn't a key's fingerprint"));
        // The user trusts what was met: both go through, with that key alone.
        assert_eq!(plugins::pins::trust("netty", &format!("127.0.0.1:{wss}"), None).unwrap(), KEY_OF_A);
        plugins::pins::trust("netty", &format!("https://127.0.0.1:{https}"), None).unwrap();
        cmd("open", json!({"url": ws_url}));
        cmd("send", json!({"data": json!({"jsonrpc": "2.0", "id": 1, "method": "ping", "params": {}}).to_string()}));
        assert!(heard(1)[0].contains("\"method\":\"ping\""));
        assert_eq!(cmd("get", json!({"url": web_url})), json!({"status": 200, "body": "secure"}));
        let s = cmd("stream", json!({"url": format!("https://127.0.0.1:{https}/events")}));
        assert_eq!(s["text"], "data: one\n\ndata: два\n\ndata: three\n\n");
        // The server's certificate changes: refused until the user trusts the new key.
        *shows.lock().unwrap() = server_config(CERT_B, KEY_B);
        let changed = cmd("try", json!({"url": ws_url}));
        assert!(changed.as_str().unwrap().contains("changed since you trusted it") && changed.as_str().unwrap().contains(KEY_OF_A), "{changed}");
        let e = call("netty", "get", &json!({"url": web_url})).unwrap_err().to_string();
        assert!(e.contains("changed since you trusted it"), "{e}");
        let now = plugins::pins::view("netty");
        assert!(now.iter().all(|c| c["pinned"] == KEY_OF_A && c["changed"] == true), "{now:?}");
        plugins::pins::trust("netty", &format!("127.0.0.1:{wss}"), None).unwrap();
        assert_eq!(cmd("try", json!({"url": ws_url})), json!("open"));
        // Forgotten: refused again.
        assert!(plugins::pins::forget("netty", &format!("127.0.0.1:{wss}")).unwrap());
        assert!(cmd("try", json!({"url": ws_url})).as_str().unwrap().contains("isn't one this computer trusts"));
        // Its pins go with its data.
        plugins::remove("netty", false).unwrap();
        sync();
        assert!(plugins::pins::view("netty").is_empty());
    }

    #[test]
    fn a_plugin_streams_an_answer_as_it_comes_cancels_it_and_sends_and_saves_big_files() {
        let (_env, root) = paths::testenv::folders("jshost-stream");
        install(&root);
        let (port, log) = web_server(None);
        let at = |p: &str| format!("http://127.0.0.1:{port}{p}");
        assert!(call("netty", "stream", &json!({"url": at("/events")})).unwrap_err().to_string().contains("may reach only"));
        plugins::set_setting("netty", "voice", &json!(format!("http://127.0.0.1:{port}"))).unwrap();
        // Pieces as they come, whole letters only (one was cut inside "а"), then its end.
        let s = cmd("stream", json!({"url": at("/events")}));
        assert_eq!(s["status"], 200);
        assert_eq!(s["type"], "text/event-stream");
        assert_eq!(s["text"], "data: one\n\ndata: два\n\ndata: three\n\n");
        assert_eq!(s["done"]["cancelled"], false);
        // An endless stream ends when the plugin cancels it: the server sees the client go.
        let c = cmd("stream", json!({"url": at("/forever"), "cancelAfter": 2}));
        assert_eq!(c["done"]["cancelled"], true, "{c}");
        paths::testenv::eventually("the server saw it go", || log.lock().unwrap().iter().any(|l| l == "gone"));
        paths::testenv::eventually("no connection left", || open_of("netty") == 0);
        // A file the user handed it is sent as the body, byte for byte.
        let f = root.join("note.bin");
        let bytes: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&f, &bytes).unwrap();
        let h = files::make("netty", &f).unwrap();
        let up = call_with("netty", "upload", &json!({"url": at("/upload")}), std::slice::from_ref(&h), CALL_TIMEOUT).unwrap();
        use sha2::Digest as _;
        assert_eq!(up["body"], format!("300000 {:x}", sha2::Sha256::digest(&bytes)));
        assert!(log.lock().unwrap().iter().any(|l| l == "PUT /upload"));
        assert!(call_with("netty", "get", &json!({"url": at("/upload"), "opts": {"file": h["file"], "body": "x", "method": "PUT"}}), &[], CALL_TIMEOUT).unwrap_err().to_string().contains("not both"));
        assert!(call("netty", "get", &json!({"url": at("/upload"), "opts": {"file": "0123", "method": "PUT"}})).unwrap_err().to_string().contains("isn't a file handed to netty"));
        // Over 10 MB: refused into memory, saved into its store.
        assert!(call("netty", "get", &json!({"url": at("/big")})).is_err());
        let saved = cmd("get", json!({"url": at("/big"), "opts": {"save": "cache/big.bin"}}));
        assert_eq!(saved["bytes"], 12 << 20);
        assert_eq!(std::fs::metadata(plugins::data_dir("netty").join("cache/big.bin")).unwrap().len(), 12 << 20);
        plugins::remove("netty", false).unwrap();
        sync();
    }
}
