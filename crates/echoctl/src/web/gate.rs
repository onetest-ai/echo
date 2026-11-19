//! The gate: the HTTP CONNECT proxy on 127.0.0.1 that every connection of a page rendered for a plugin goes through
//! (`ctx.web.read({render: true})`, #835). It lives only as long as the render, on a port the system picks, and answers
//! only a client that sends the render's own token (`Proxy-Authorization: Basic`, else 407).
//!
//! For each `CONNECT host:port`: port 443 only (a rendered page is https); an IPv6 address written as one is refused;
//! the name is looked up again on every connection and every address it has must be public and none of this
//! computer's own (`public::public_address_by`, `netif::addresses`); Echo then connects
//! to the address it checked, never the name, and passes the bytes through unread (TLS stays end to end, the page's
//! certificate checked by the webview against the name). Anything else — an origin-form request (something calling the
//! port directly), an absolute-form `http://` request, another method — is refused with 403. A request's head must come
//! whole within 10 s, and at most `HANDSHAKES` clients are heard at once before they show the token.
//!
//! Within `Caps`: connections in all and at once, distinct hosts, bytes both ways; and its time (`until`), when every
//! tunnel is cut and nothing more is accepted.

// Used by `echoctl render-page` (`web::render`, feature `tray`).
#![cfg_attr(not(any(test, feature = "tray")), allow(dead_code))]

use std::collections::HashSet;
use std::io::{ErrorKind, Read, Write};
use std::net::{IpAddr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use base64::Engine as _;
use serde_json::{json, Value};

use super::public::{public_address_by, system_lookup, Lookup};

/// How much a render may reach through the gate.
#[derive(Clone, Copy, Debug)]
pub struct Caps {
    /// Connections in all, and open at once.
    pub conns: usize,
    pub at_once: usize,
    /// Distinct hosts asked for.
    pub hosts: usize,
    /// Bytes through every tunnel, both ways.
    pub bytes: u64,
}

/// A render's caps (design #835 § Caps).
pub const CAPS: Caps = Caps { conns: 64, at_once: 16, hosts: 32, bytes: 25 << 20 };

/// The user name the token goes with.
pub const USER: &str = "echo";
/// A request's head, in bytes at most, and how long it may take to come.
const HEAD_MAX: usize = 8 << 10;
const HEAD_SECS: u64 = 10;
/// Connecting to a checked address: 10 s at most.
const CONNECT_SECS: u64 = 10;
/// How often a tunnel looks at the gate's time and bytes.
const STEP: Duration = Duration::from_millis(200);
/// Refusals kept for the render's stats.
const REFUSALS_KEPT: usize = 32;
/// Clients heard at once before they have shown the token (others are closed unanswered).
const HANDSHAKES: usize = 32;

/// How the gate connects to an address it checked: a plain TCP connection, or a test's.
pub type Connect = dyn Fn(SocketAddr, Duration) -> std::io::Result<TcpStream> + Send + Sync;

/// The gate's way out: how it looks names up and connects. `Net::system()` but in tests.
pub struct Net {
    pub lookup: Arc<Lookup>,
    pub connect: Arc<Connect>,
    /// This computer's own addresses, read for each connection (`netif::addresses`: an error refuses it).
    pub own: Arc<dyn Fn() -> Result<Vec<IpAddr>> + Send + Sync>,
    /// The ports a page's hosts are reached on: 443 (a development build's tests add their plain-http fakes' 80).
    pub ports: &'static [u16],
}

impl Net {
    pub fn system() -> Net {
        Net { lookup: Arc::new(system_lookup), connect: Arc::new(|at, wait| TcpStream::connect_timeout(&at, wait)), own: Arc::new(crate::netif::addresses), ports: &[443] }
    }
}

struct Shared {
    caps: Caps,
    net: Net,
    /// `Proxy-Authorization`'s value, as the client must send it.
    auth: String,
    /// The 407's `Proxy-Authenticate` value.
    challenge: String,
    until: Instant,
    stop: AtomicBool,
    conns: AtomicUsize,
    open: AtomicUsize,
    handshakes: AtomicUsize,
    bytes: AtomicU64,
    hosts: Mutex<HashSet<String>>,
    last: Mutex<Instant>,
    refused: Mutex<Vec<String>>,
}

impl Shared {
    fn over(&self) -> bool {
        self.stop.load(Ordering::SeqCst) || Instant::now() >= self.until
    }

    fn refuse(&self, why: String) {
        let mut r = self.refused.lock().unwrap_or_else(|e| e.into_inner());
        if r.len() < REFUSALS_KEPT {
            r.push(why);
        }
    }
}

/// A running gate: its port and token, until it is dropped or its time is up.
pub struct Gate {
    pub port: u16,
    pub token: String,
    /// The 407's challenge, `Basic realm="echo-<random>"`: a client that answers challenges (WebView2) answers this
    /// one only, never a site's own 401.
    pub challenge: String,
    shared: Arc<Shared>,
}

impl Gate {
    /// The gate on 127.0.0.1 (a port the system picks), with a new token, until `until`.
    pub fn start(net: Net, caps: Caps, until: Instant) -> Result<Gate> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let mut rnd = [0u8; 16];
        getrandom::fill(&mut rnd).map_err(|e| anyhow!("{e}"))?;
        let token: String = rnd.iter().map(|b| format!("{b:02x}")).collect();
        let auth = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{USER}:{token}")));
        let mut realm = [0u8; 8];
        getrandom::fill(&mut realm).map_err(|e| anyhow!("{e}"))?;
        let challenge = format!("Basic realm=\"echo-{}\"", realm.iter().map(|b| format!("{b:02x}")).collect::<String>());
        let shared = Arc::new(Shared {
            caps,
            net,
            auth,
            challenge: challenge.clone(),
            until,
            stop: AtomicBool::new(false),
            conns: AtomicUsize::new(0),
            open: AtomicUsize::new(0),
            handshakes: AtomicUsize::new(0),
            bytes: AtomicU64::new(0),
            hosts: Mutex::new(HashSet::new()),
            last: Mutex::new(Instant::now()),
            refused: Mutex::new(Vec::new()),
        });
        let s = shared.clone();
        std::thread::spawn(move || accept(listener, s));
        Ok(Gate { port, token, challenge, shared })
    }

    /// When the last connection was let through (the gate's start before any).
    pub fn last_connection(&self) -> Instant {
        *self.shared.last.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// What went through: {connections, hosts, bytes, refused (the first few, why)}.
    pub fn stats(&self) -> Value {
        let s = &self.shared;
        json!({
            "connections": s.conns.load(Ordering::SeqCst).min(s.caps.conns),
            "hosts": s.hosts.lock().unwrap_or_else(|e| e.into_inner()).len(),
            "bytes": s.bytes.load(Ordering::SeqCst),
            "refused": *s.refused.lock().unwrap_or_else(|e| e.into_inner()),
        })
    }
}

impl Drop for Gate {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
    }
}

fn accept(listener: TcpListener, s: Arc<Shared>) {
    while !s.over() {
        match listener.accept() {
            Ok((c, _)) => {
                let s = s.clone();
                std::thread::spawn(move || serve(c, &s));
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

/// A request's head (up to its blank line), or `None`: too long, not whole within `HEAD_SECS` in all (nor the
/// gate's time), or not text.
fn read_head(c: &mut TcpStream, s: &Shared) -> Option<String> {
    let by = (Instant::now() + Duration::from_secs(HEAD_SECS)).min(s.until);
    let mut head = Vec::with_capacity(512);
    let mut b = [0u8; 1];
    // A byte at a time: nothing past the head is taken (the client's first bytes of TLS belong to the tunnel).
    while !head.ends_with(b"\r\n\r\n") {
        let left = by.saturating_duration_since(Instant::now());
        if s.over() || left.is_zero() || head.len() >= HEAD_MAX {
            return None;
        }
        c.set_read_timeout(Some(left)).ok()?;
        if c.read(&mut b).ok()? == 0 {
            return None;
        }
        head.push(b[0]);
    }
    String::from_utf8(head).ok()
}

/// A client heard before it has shown the token: counted until dropped.
struct Handshake<'a>(&'a Shared);

impl Drop for Handshake<'_> {
    fn drop(&mut self) {
        self.0.handshakes.fetch_sub(1, Ordering::SeqCst);
    }
}

/// What a refusal keeps of what a client asked for: printable ASCII, 200 characters at most.
fn plain(text: &str) -> String {
    text.chars().filter(|c| c.is_ascii_graphic()).take(200).collect()
}

fn reply(c: &mut TcpStream, status: &str, extra: &str) {
    let _ = write!(c, "HTTP/1.1 {status}\r\n{extra}Content-Length: 0\r\nConnection: close\r\n\r\n");
}

fn serve(mut c: TcpStream, s: &Shared) {
    if s.handshakes.fetch_add(1, Ordering::SeqCst) >= HANDSHAKES {
        s.handshakes.fetch_sub(1, Ordering::SeqCst);
        return;
    }
    let shaking = Handshake(s);
    let _ = c.set_nonblocking(false);
    let Some(head) = read_head(&mut c, s) else { return };
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let (method, target) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    if method != "CONNECT" {
        // Nothing of what an unknown client sent is kept.
        s.refuse("a request other than CONNECT".into());
        return reply(&mut c, "403 Forbidden", "");
    }
    let authorized = lines.any(|l| l.split_once(':').is_some_and(|(k, v)| k.trim().eq_ignore_ascii_case("proxy-authorization") && v.trim() == s.auth));
    if !authorized {
        return reply(&mut c, "407 Proxy Authentication Required", &format!("Proxy-Authenticate: {}\r\n", s.challenge));
    }
    drop(shaking);
    let opened = open(target, s);
    match opened {
        Ok((up, _slot)) => {
            if c.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").is_ok() {
                tunnel(c, up, s);
            }
        }
        Err(e) => {
            s.refuse(plain(&format!("{target}: {e:#}")));
            reply(&mut c, "403 Forbidden", "");
        }
    }
}

/// One tunnel open: counted until dropped.
struct Slot<'a>(&'a Shared);

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.open.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The connection `CONNECT target` asks for, to the address Echo checked, within the caps.
fn open<'a>(target: &str, s: &'a Shared) -> Result<(TcpStream, Slot<'a>)> {
    let (host, port) = target.rsplit_once(':').ok_or_else(|| anyhow!("not host:port"))?;
    if host.starts_with('[') || host.parse::<Ipv6Addr>().is_ok() {
        bail!("an IPv6 address written as one: a page's hosts are reached by name or IPv4 address");
    }
    let port: u16 = port.parse().ok().filter(|p| s.net.ports.contains(p)).ok_or_else(|| anyhow!("port {port}: only {:?}", s.net.ports))?;
    let host = host.to_ascii_lowercase();
    {
        let mut hosts = s.hosts.lock().unwrap_or_else(|e| e.into_inner());
        if !hosts.contains(&host) {
            if hosts.len() >= s.caps.hosts {
                bail!("past the {} hosts a render reaches", s.caps.hosts);
            }
            hosts.insert(host.clone());
        }
    }
    if s.open.fetch_add(1, Ordering::SeqCst) >= s.caps.at_once {
        s.open.fetch_sub(1, Ordering::SeqCst);
        bail!("past the {} connections a render has open at once", s.caps.at_once);
    }
    let slot = Slot(s);
    if s.conns.fetch_add(1, Ordering::SeqCst) >= s.caps.conns {
        bail!("past the {} connections a render opens", s.caps.conns);
    }
    if s.bytes.load(Ordering::SeqCst) >= s.caps.bytes {
        bail!("past the {} MB a render moves", s.caps.bytes >> 20);
    }
    // Looked up again for every connection: a name that now points into the user's network is refused.
    let ip: IpAddr = public_address_by(&host, port, s.until, s.net.lookup.clone(), &(s.net.own)()?)?;
    let wait = s.until.saturating_duration_since(Instant::now()).min(Duration::from_secs(CONNECT_SECS));
    if wait.is_zero() {
        bail!("the render's time is up");
    }
    let up = (s.net.connect)(SocketAddr::new(ip, port), wait)?;
    *s.last.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
    Ok((up, slot))
}

/// The bytes both ways until either side ends, the bytes are spent or the time is up.
fn tunnel(c: TcpStream, up: TcpStream, s: &Shared) {
    // A side that stops reading holds a write only a step at a time (`send`), never past the gate's time.
    let _ = (c.set_write_timeout(Some(STEP)), up.set_write_timeout(Some(STEP)));
    let (Ok(c2), Ok(up2)) = (c.try_clone(), up.try_clone()) else { return };
    std::thread::scope(|scope| {
        scope.spawn(|| pump(c, up2, s));
        pump(up, c2, s);
    });
}

fn pump(mut from: TcpStream, mut to: TcpStream, s: &Shared) {
    let _ = from.set_read_timeout(Some(STEP));
    let mut buf = vec![0u8; 16 << 10];
    loop {
        if s.over() || s.bytes.load(Ordering::SeqCst) >= s.caps.bytes {
            break;
        }
        match from.read(&mut buf) {
            Ok(0) => {
                // One side done sending: the other may still answer.
                let _ = to.shutdown(Shutdown::Write);
                return;
            }
            Ok(n) => {
                if s.bytes.fetch_add(n as u64, Ordering::SeqCst) + n as u64 > s.caps.bytes || !send(&mut to, &buf[..n], s) {
                    break;
                }
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => {}
            Err(_) => break,
        }
    }
    let _ = from.shutdown(Shutdown::Both);
    let _ = to.shutdown(Shutdown::Both);
}

/// All of `bytes` to `to`, waiting a step at a time while the gate's time and bytes allow; whether they went.
fn send(to: &mut TcpStream, mut bytes: &[u8], s: &Shared) -> bool {
    while !bytes.is_empty() {
        if s.over() || s.bytes.load(Ordering::SeqCst) > s.caps.bytes {
            return false;
        }
        match to.write(bytes) {
            Ok(0) => return false,
            Ok(n) => bytes = &bytes[n..],
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => {}
            Err(_) => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A made-up internet: names and their addresses, every lookup counted, and every address the gate connected to
    /// recorded — each reaching `up`, a local made-up server, whatever address it was asked for.
    struct Fake {
        lookups: Arc<AtomicUsize>,
        connected: Arc<Mutex<Vec<SocketAddr>>>,
    }

    fn fake(names: &[(&str, &[&str])], up: SocketAddr) -> (Net, Fake) {
        let map: HashMap<String, Vec<IpAddr>> = names.iter().map(|(n, ips)| (n.to_string(), ips.iter().map(|i| i.parse().unwrap()).collect())).collect();
        let map = Arc::new(Mutex::new(map));
        let lookups = Arc::new(AtomicUsize::new(0));
        let connected = Arc::new(Mutex::new(vec![]));
        let (l, c) = (lookups.clone(), connected.clone());
        let m = map.clone();
        let net = Net {
            own: Arc::new(|| Ok(vec!["127.0.0.1".parse().unwrap(), "93.184.215.99".parse().unwrap(), "2a00:1450:4001:80b::5".parse().unwrap()])),
            ports: &[443],
            lookup: Arc::new(move |host, _| {
                l.fetch_add(1, Ordering::SeqCst);
                m.lock().unwrap().get(host).cloned().ok_or_else(|| std::io::Error::new(ErrorKind::NotFound, "no such name"))
            }),
            connect: Arc::new(move |at, wait| {
                c.lock().unwrap().push(at);
                TcpStream::connect_timeout(&up, wait)
            }),
        };
        (net, Fake { lookups, connected })
    }

    /// A made-up server on 127.0.0.1 that answers every line it gets with "pong", or (`flood`) sends that many bytes.
    fn upstream(flood: usize) -> SocketAddr {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let at = l.local_addr().unwrap();
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                std::thread::spawn(move || {
                    if flood > 0 {
                        let _ = s.write_all(&vec![b'x'; flood]);
                        return;
                    }
                    let mut b = [0u8; 64];
                    while let Ok(n) = s.read(&mut b) {
                        if n == 0 || s.write_all(b"pong").is_err() {
                            break;
                        }
                    }
                });
            }
        });
        at
    }

    fn soon() -> Instant {
        Instant::now() + Duration::from_secs(20)
    }

    /// `CONNECT target` to the gate (with its token, a wrong one, or none): the answer's status and the connection.
    fn ask(g: &Gate, target: &str, token: Option<&str>) -> (u16, TcpStream) {
        let mut c = TcpStream::connect(("127.0.0.1", g.port)).unwrap();
        c.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
        let auth = token.map_or(String::new(), |t| format!("Proxy-Authorization: Basic {}\r\n", base64::engine::general_purpose::STANDARD.encode(format!("{USER}:{t}"))));
        write!(c, "CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n{auth}\r\n").unwrap();
        (status(&mut c), c)
    }

    fn status(c: &mut TcpStream) -> u16 {
        let mut head = vec![];
        let mut b = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            match c.read(&mut b) {
                Ok(1) => head.push(b[0]),
                _ => return 0,
            }
        }
        String::from_utf8_lossy(&head).split(' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0)
    }

    fn ping(c: &mut TcpStream) -> String {
        c.write_all(b"ping").unwrap();
        let mut b = [0u8; 4];
        c.read_exact(&mut b).map(|_| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
    }

    #[test]
    fn nothing_on_this_computer_or_the_users_network_is_reached() {
        let names: &[(&str, &[&str])] = &[
            ("loop.acme.example", &["127.0.0.1"]),
            ("ten.acme.example", &["10.0.0.7"]),
            ("home.acme.example", &["192.168.1.10"]),
            ("metadata.acme.example", &["169.254.169.254"]),
            ("v6.acme.example", &["::1"]),
            ("ula.acme.example", &["fd00::7"]),
            ("mapped.acme.example", &["::ffff:192.168.1.10"]),
            ("mixed.acme.example", &["93.184.215.14", "192.168.1.10"]),
            ("mixed6.acme.example", &["2606:2800:21f:cb07:6820:80da:af6b:8b2c", "fd00::1"]),
            ("localhost", &["127.0.0.1"]),
            // This computer's own public addresses (injected above): its IPv4, and its IPv6 /64.
            ("self.acme.example", &["93.184.215.99"]),
            ("self6.acme.example", &["2a00:1450:4001:80b::abcd"]),
        ];
        let (net, seen) = fake(names, upstream(0));
        let g = Gate::start(net, CAPS, soon()).unwrap();
        let t = g.token.clone();
        let mut refused: Vec<String> = [
            "127.0.0.1:443", "10.1.2.3:443", "192.168.68.1:443", "172.16.0.1:443", "169.254.169.254:80", "100.64.0.1:443", "0.0.0.0:443",
            "[::1]:443", "[fd00::1]:443", "[::ffff:127.0.0.1]:443", "[::ffff:10.0.0.1]:443", "::1:443",
            "2130706433:443", "0x7f.1:443", "127.1:443", "nobody.acme.example:443", "93.184.215.99:443",
        ]
        .map(String::from)
        .to_vec();
        refused.extend(names.iter().map(|(n, _)| format!("{n}:443")));
        for target in &refused {
            assert_eq!(ask(&g, target, Some(&t)).0, 403, "{target}");
        }
        assert!(seen.connected.lock().unwrap().is_empty(), "nothing was connected to: {:?}", seen.connected.lock().unwrap());
        assert!(g.stats()["refused"].as_array().unwrap().iter().any(|r| r.as_str().unwrap().contains("mixed.acme.example")));
    }

    #[test]
    fn when_this_computers_addresses_cant_be_read_nothing_is_reached() {
        let (net, seen) = fake(&[("news.acme.example", &["93.184.215.14"])], upstream(0));
        let net = Net { own: Arc::new(|| anyhow::bail!("can't list")), ..net };
        let g = Gate::start(net, CAPS, soon()).unwrap();
        assert_eq!(ask(&g, "news.acme.example:443", Some(&g.token.clone())).0, 403);
        assert_eq!(ask(&g, "8.8.8.8:443", Some(&g.token.clone())).0, 403);
        assert!(seen.connected.lock().unwrap().is_empty());
    }

    #[test]
    fn only_port_443_is_reached() {
        let (net, seen) = fake(&[("news.acme.example", &["93.184.215.14"])], upstream(0));
        let g = Gate::start(net, CAPS, soon()).unwrap();
        for target in ["news.acme.example:80", "news.acme.example:22", "news.acme.example:8443", "news.acme.example:0", "news.acme.example:x", "news.acme.example"] {
            assert_eq!(ask(&g, target, Some(&g.token.clone())).0, 403, "{target}");
        }
        assert_eq!(seen.lookups.load(Ordering::SeqCst), 0, "refused before any lookup");
        assert_eq!(ask(&g, "news.acme.example:443", Some(&g.token.clone())).0, 200);
    }

    #[test]
    fn a_slow_or_crowding_client_gets_nothing_before_the_token() {
        let (net, seen) = fake(&[("news.acme.example", &["93.184.215.14"])], upstream(0));
        let g = Gate::start(net, CAPS, soon()).unwrap();
        // HANDSHAKES clients that never finish their head hold every place; the next is closed unanswered.
        let mut idle: Vec<TcpStream> = (0..HANDSHAKES).map(|_| TcpStream::connect(("127.0.0.1", g.port)).unwrap()).collect();
        for c in &mut idle {
            c.write_all(b"CONNECT news.acme.example:443 HTTP/1.1\r\n").unwrap();
        }
        let held = Instant::now();
        while g.shared.handshakes.load(Ordering::SeqCst) < HANDSHAKES && held.elapsed() < Duration::from_secs(10) {
            std::thread::yield_now();
        }
        let mut late = TcpStream::connect(("127.0.0.1", g.port)).unwrap();
        late.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let _ = write!(late, "CONNECT news.acme.example:443 HTTP/1.1\r\n\r\n");
        assert_eq!(status(&mut late), 0, "closed with no answer");
        drop(idle);
        // A head that trickles in a byte at a time still ends at its 10 s in all, unanswered.
        let mut slow = TcpStream::connect(("127.0.0.1", g.port)).unwrap();
        let at = Instant::now();
        for b in b"CONNECT news.acme.example:443 HTTP/1.1\r\nX-Pad: ".iter().chain([b'a'; 64].iter()) {
            if slow.write_all(&[*b]).is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
            if at.elapsed() > Duration::from_secs(13) {
                break;
            }
        }
        let _ = slow.set_read_timeout(Some(Duration::from_secs(2)));
        assert_eq!(status(&mut slow), 0);
        assert!(at.elapsed() < Duration::from_secs(14), "{:?}", at.elapsed());
        assert!(seen.connected.lock().unwrap().is_empty());
        // What an unknown client sent is never kept.
        let mut c = TcpStream::connect(("127.0.0.1", g.port)).unwrap();
        c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        c.write_all(b"GET /<script>alert(1)</script> HTTP/1.1\r\n\r\n").unwrap();
        assert_eq!(status(&mut c), 403);
        assert!(!g.stats().to_string().contains("script"), "{}", g.stats());
    }

    #[test]
    fn only_connect_with_the_renders_token_is_answered() {
        let (net, seen) = fake(&[("news.acme.example", &["93.184.215.14"])], upstream(0));
        let g = Gate::start(net, CAPS, soon()).unwrap();
        assert_eq!(ask(&g, "news.acme.example:443", None).0, 407);
        assert_eq!(ask(&g, "news.acme.example:443", Some("not-the-token")).0, 407);
        for req in ["GET / HTTP/1.1\r\nHost: x\r\n\r\n", "GET http://news.acme.example/ HTTP/1.1\r\nHost: news.acme.example\r\n\r\n", "POST http://news.acme.example/ HTTP/1.1\r\n\r\n", "connect news.acme.example:443 HTTP/1.1\r\n\r\n"] {
            let mut c = TcpStream::connect(("127.0.0.1", g.port)).unwrap();
            c.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
            c.write_all(req.as_bytes()).unwrap();
            assert_eq!(status(&mut c), 403, "{req}");
        }
        assert!(seen.connected.lock().unwrap().is_empty());
    }

    #[test]
    fn a_public_name_is_tunnelled_to_the_address_echo_checked() {
        let (net, seen) = fake(&[("news.acme.example", &["93.184.215.14", "2606:2800:21f:cb07:6820:80da:af6b:8b2c"])], upstream(0));
        let g = Gate::start(net, CAPS, soon()).unwrap();
        let (code, mut c) = ask(&g, "News.Acme.Example:443", Some(&g.token.clone()));
        assert_eq!(code, 200);
        assert_eq!(ping(&mut c), "pong");
        assert_eq!(*seen.connected.lock().unwrap(), vec!["93.184.215.14:443".parse::<SocketAddr>().unwrap()]);
        // A public address written as one is reached as it is.
        assert_eq!(ask(&g, "8.8.8.8:443", Some(&g.token.clone())).0, 200);
        assert_eq!(seen.connected.lock().unwrap()[1], "8.8.8.8:443".parse::<SocketAddr>().unwrap());
        let stats = g.stats();
        assert_eq!((stats["connections"].as_u64(), stats["hosts"].as_u64()), (Some(2), Some(2)));
        assert!(stats["bytes"].as_u64().unwrap() >= 8);
        assert_eq!(ask(&g, "93.184.215.99:443", Some(&g.token.clone())).0, 403, "this computer's own public address");
    }

    #[test]
    fn every_connection_looks_the_name_up_again() {
        let up = upstream(0);
        let (net, seen) = fake(&[("news.acme.example", &["93.184.215.14"])], up);
        // The name moves into the user's network after its first lookup (DNS rebinding).
        let lookup = net.lookup.clone();
        let n = Arc::new(AtomicUsize::new(0));
        let net = Net {
            lookup: Arc::new(move |h, p| if n.fetch_add(1, Ordering::SeqCst) == 0 { lookup(h, p) } else { Ok(vec!["127.0.0.1".parse().unwrap()]) }),
            ..net
        };
        let g = Gate::start(net, CAPS, soon()).unwrap();
        assert_eq!(ask(&g, "news.acme.example:443", Some(&g.token.clone())).0, 200);
        assert_eq!(ask(&g, "news.acme.example:443", Some(&g.token.clone())).0, 403);
        assert_eq!(seen.connected.lock().unwrap().len(), 1);
    }

    #[test]
    fn connections_and_hosts_stop_at_their_caps() {
        let names: &[(&str, &[&str])] = &[("a.acme.example", &["93.184.215.14"]), ("b.acme.example", &["93.184.215.15"]), ("c.acme.example", &["93.184.215.16"])];
        let (net, _) = fake(names, upstream(0));
        let g = Gate::start(net, Caps { conns: 3, at_once: 2, hosts: 2, bytes: CAPS.bytes }, soon()).unwrap();
        let t = g.token.clone();
        let (code, mut one) = ask(&g, "a.acme.example:443", Some(&t));
        assert_eq!(code, 200);
        let (code, mut two) = ask(&g, "b.acme.example:443", Some(&t));
        assert_eq!(code, 200);
        assert_eq!(ask(&g, "c.acme.example:443", Some(&t)).0, 403, "a third host");
        assert_eq!(ask(&g, "a.acme.example:443", Some(&t)).0, 403, "a third open at once");
        // One closed: room for another, up to the connections in all.
        assert_eq!(ping(&mut one), "pong");
        drop(one);
        let closed = Instant::now();
        let mut third = loop {
            match ask(&g, "a.acme.example:443", Some(&t)) {
                (200, c) => break c,
                _ if closed.elapsed() < Duration::from_secs(10) => std::thread::yield_now(),
                (code, _) => panic!("a closed tunnel's slot never came back: {code}"),
            }
        };
        assert_eq!(ping(&mut third), "pong");
        assert_eq!(ping(&mut two), "pong");
        drop((two, third));
        let s = g.stats();
        assert!(s["refused"].to_string().contains("connections"), "{s}");
        assert!(s["refused"].to_string().contains("hosts"), "{s}");
    }

    #[test]
    fn the_connections_in_all_are_capped() {
        let (net, seen) = fake(&[("a.acme.example", &["93.184.215.14"])], upstream(0));
        let g = Gate::start(net, Caps { conns: 2, at_once: 16, hosts: 32, bytes: CAPS.bytes }, soon()).unwrap();
        let t = g.token.clone();
        assert_eq!(ask(&g, "a.acme.example:443", Some(&t)).0, 200);
        assert_eq!(ask(&g, "a.acme.example:443", Some(&t)).0, 200);
        assert_eq!(ask(&g, "a.acme.example:443", Some(&t)).0, 403);
        assert_eq!(seen.connected.lock().unwrap().len(), 2);
    }

    #[test]
    fn a_tunnel_is_cut_at_the_bytes_cap() {
        let (net, _) = fake(&[("big.acme.example", &["93.184.215.14"])], upstream(1 << 20));
        let g = Gate::start(net, Caps { bytes: 64 << 10, ..CAPS }, soon()).unwrap();
        let t = g.token.clone();
        let (code, mut c) = ask(&g, "big.acme.example:443", Some(&t));
        assert_eq!(code, 200);
        let mut got = vec![];
        let _ = c.read_to_end(&mut got);
        assert!(got.len() <= 64 << 10, "{} bytes through a 64 KB cap", got.len());
        assert_eq!(ask(&g, "big.acme.example:443", Some(&t)).0, 403, "nothing more once spent");
    }

    #[test]
    fn at_its_time_every_tunnel_is_cut_and_nothing_more_is_answered() {
        let (net, _) = fake(&[("news.acme.example", &["93.184.215.14"])], upstream(0));
        let g = Gate::start(net, CAPS, Instant::now() + Duration::from_millis(1500)).unwrap();
        let (code, mut c) = ask(&g, "news.acme.example:443", Some(&g.token.clone()));
        assert_eq!(code, 200);
        assert_eq!(ping(&mut c), "pong");
        let mut rest = vec![];
        let at = Instant::now();
        let _ = c.read_to_end(&mut rest);
        assert!(at.elapsed() < Duration::from_secs(5), "the tunnel was cut at the gate's time");
        let gone = TcpStream::connect(("127.0.0.1", g.port)).and_then(|mut c| {
            c.set_read_timeout(Some(Duration::from_secs(2)))?;
            let _ = c.write_all(b"CONNECT news.acme.example:443 HTTP/1.1\r\n\r\n");
            Ok(status(&mut c))
        });
        assert!(!matches!(gone, Ok(200) | Ok(407) | Ok(403)), "{gone:?}");
    }
}
