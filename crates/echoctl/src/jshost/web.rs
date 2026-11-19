//! `ctx.web` (no capability of its own: a plugin reads only what `ctx.http.fetch` may reach): `read(url, {max,
//! images})` — a web page as Markdown (`crate::web::read`), its pictures listed and, asked, saved as file handles —
//! `fetch(url, {max})` — one file as a handle — and `markdown(html, {base})` — HTML the plugin already has, read the same
//! way. Every request goes through `http::admit` and `http::get_capped`: the network path, grants and pins of
//! `ctx.http.fetch`, plain GETs, no redirect followed. Two widenings: as a browser shows a page, the pictures a public
//! page shows may come from their own public hosts (`picture_reach`); and a plugin with `uses.network.web` reads any
//! https page on the public internet, its redirects followed and each checked again (`web_reach`, `get_page`).

use std::net::IpAddr;

use super::http::{admit, get_capped, reach, Head, Reach};
use super::*;
use crate::web::public::public_address;
use crate::web::read::{self as page, Image};

/// A page: 5 MB of HTML (`max` less), 30 s (`timeout` less).
const PAGE_MAX: u64 = 5 << 20;
const PAGE_SECS: u64 = 30;
/// A page's pictures saved (`images: 'save'`): 50 at most, 10 MB each, 50 MB and 2 minutes in all, 30 s each.
const IMAGES_MOST: usize = 50;
const IMAGE_MAX: u64 = 10 << 20;
const IMAGES_TOTAL: u64 = 50 << 20;
const IMAGES_SECS: u64 = 120;
const IMAGE_SECS: u64 = 30;
/// The ports a picture is fetched from on a host the plugin wasn't granted (besides the page's own, on its own host).
const WEB_PORTS: [u16; 2] = [80, 443];
/// The redirects followed for a plugin with `uses.network.web` (each address checked again), and the answers that are one.
const HOPS: usize = 5;
const REDIRECTS: [u16; 5] = [301, 302, 303, 307, 308];
/// A file (`ctx.web.fetch`): 10 MB unless `max` says more, up to what `ctx.fs.copyIn` copies; 2 minutes.
const FILE_MAX: u64 = 10 << 20;
const FILE_SECS: u64 = 120;
/// The pictures Echo saves, by type, with the extension each gets.
const PICTURES: [(&str, &str); 6] = [("image/png", "png"), ("image/jpeg", "jpg"), ("image/gif", "gif"), ("image/webp", "webp"), ("image/avif", "avif"), ("image/svg+xml", "svg")];

/// A 2xx answer, as it is (never compressed): anything else refused before its body is read.
fn plain_ok(h: &Head, what: &str) -> Result<()> {
    if (300..400).contains(&h.status) {
        bail!("the {what} answered {} (it moved): Echo doesn't follow a redirect — read the address it moved to", h.status);
    }
    if !(200..300).contains(&h.status) {
        bail!("the {what} answered {}", h.status);
    }
    if !h.encoding.is_empty() && h.encoding != "identity" {
        bail!("the {what} came compressed ({}): Echo reads it only as it is", h.encoding.chars().take(40).collect::<String>());
    }
    Ok(())
}

fn html_only(h: &Head) -> Result<()> {
    plain_ok(h, "page")?;
    match h.kind.as_str() {
        "text/html" | "application/xhtml+xml" => Ok(()),
        "" => bail!("the page doesn't say what it is: Echo reads only HTML pages"),
        k => bail!("this is {}, not a web page: Echo reads only HTML (ctx.web.fetch gets a file)", k.chars().take(80).collect::<String>()),
    }
}

fn picture_only(h: &Head) -> Result<()> {
    plain_ok(h, "picture")?;
    if !PICTURES.iter().any(|(k, _)| *k == h.kind) {
        bail!("this is {}, not a picture Echo saves (PNG, JPEG, GIF, WebP, AVIF, SVG)", if h.kind.is_empty() { "unnamed" } else { &h.kind });
    }
    Ok(())
}

/// HTML's bytes as text, in the charset the server named, else the one the page names in its first 1 KB, else UTF-8 (a
/// byte-order mark first); bytes that aren't in it become �, never an error.
fn decode_html(bytes: &[u8], said: Option<&str>) -> String {
    let named = said.map(str::to_owned).or_else(|| {
        let start = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
        let at = start.find("charset=")? + 8;
        Some(start[at..].trim_start_matches(['"', '\'']).chars().take_while(|c| c.is_ascii_alphanumeric() || "-_:.".contains(*c)).collect())
    });
    let enc = named.and_then(|n| encoding_rs::Encoding::for_label(n.trim().as_bytes())).unwrap_or(encoding_rs::UTF_8);
    enc.decode(bytes).0.into_owned()
}

/// An option that is a number of bytes, at most `most`.
fn bytes_opt(v: &Value, default: u64, most: u64) -> Result<u64> {
    match v {
        Value::Null => Ok(default),
        v => v.as_u64().filter(|n| *n > 0).map(|n| n.min(most)).ok_or_else(|| anyhow!("max is a number of bytes")),
    }
}

/// `ctx.web.read(url, {max, timeout, images})`: the page at `url` (an address the plugin may reach, as `ctx.http.fetch`) read as
/// Markdown: {url, title, markdown, links, images, lang, byline, published, description, source, needs_js, truncated,
/// bytes}. `url` is the address read: the one asked for, or — with `uses.network.web` — where its redirects ended. `images: 'save'` also saves its pictures
/// (`save_pictures`).
pub(super) fn read(id: &str, url: &str, opts: &Value) -> Result<Value> {
    let max = bytes_opt(&opts["max"], PAGE_MAX, PAGE_MAX)?;
    let save = match &opts["images"] {
        Value::Null => false,
        Value::String(s) if s == "save" => true,
        _ => bail!("images is 'save' or nothing"),
    };
    let mut html = vec![];
    let secs = opts["timeout"].as_u64().filter(|s| *s > 0).map_or(PAGE_SECS, |s| s.min(PAGE_SECS));
    let (url, reach, head, bytes) = get_page(id, url, max, secs, "text/html,application/xhtml+xml", &html_only, &mut html)?;
    let url = url.as_str();
    let p = page::read(&decode_html(&html, head.charset.as_deref()), url);
    let mut v = p.to_json();
    if p.title.is_empty() {
        v["title"] = json!(url.split_once("://").map_or(url, |(_, r)| r).trim_end_matches('/'));
    }
    v["url"] = json!(url);
    v["bytes"] = json!(bytes);
    if save {
        v["images"] = json!(save_pictures(id, url, &reach, &p.images));
    }
    Ok(v)
}

/// `ctx.web.markdown(html, {base})`: HTML the plugin has (a mail's body, a page it fetched itself) read as `read` reads
/// a page; `base`, an http(s) address, for its relative links (without it they are text). Nothing is fetched.
pub(super) fn markdown(html: &str, opts: &Value) -> Result<Value> {
    if html.len() as u64 > PAGE_MAX {
        bail!("ctx.web.markdown reads 5 MB of HTML at most");
    }
    let base = opts["base"].as_str().unwrap_or("");
    if !base.is_empty() && page::web_link("", base).as_deref() != Some(base) {
        bail!("base is an http(s) address of 2 KB at most");
    }
    Ok(page::read(html, base).to_json())
}

/// `ctx.web.fetch(url, {max})`: one file the plugin may reach (as `ctx.http.fetch`), saved in its store's cache, as a
/// handle {file, name, size, type} to read (`ctx.files.read`) or copy into a folder it was given (`ctx.fs.copyIn`).
pub(super) fn fetch(id: &str, url: &str, opts: &Value) -> Result<Value> {
    let max = bytes_opt(&opts["max"], FILE_MAX, files::MAX_COPY)?;
    let mut rnd = [0u8; 8];
    getrandom::fill(&mut rnd).map_err(|e| anyhow!("{e}"))?;
    let dir = format!("cache/web/files/{}", rnd.iter().map(|b| format!("{b:02x}")).collect::<String>());
    let (path, kind, _) = save(id, &dir, &file_name(url), |sink| get_page(id, url, max, FILE_SECS, "*/*", &|h| plain_ok(h, "file"), sink).map(|(_, _, h, n)| (h, n)))?;
    let mut handle = files::make(id, &path)?;
    handle["type"] = json!(kind);
    Ok(handle)
}

/// The file's name from its address's last segment: letters, digits, `.`, `_`, `-` (80 at most), never starting with a
/// dot; "file" when nothing is left.
fn file_name(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or("").split_once("://").map_or("", |(_, r)| r);
    let last = path.split_once('/').map_or("", |(_, p)| p).rsplit('/').next().unwrap_or("");
    let name: String = last.chars().filter(|c| c.is_ascii_alphanumeric() || "._-".contains(*c)).take(80).collect();
    let name = name.trim_start_matches('.').to_string();
    if name.is_empty() {
        "file".into()
    } else {
        name
    }
}

/// Write what `get` reads into `dir/name` of the store (a half-written file never left); the path, type and size.
fn save(id: &str, dir: &str, name: &str, get: impl FnOnce(&mut dyn std::io::Write) -> Result<(Head, u64)>) -> Result<(PathBuf, String, u64)> {
    let to = writable_store_path(id, &format!("{dir}/{name}")).or_else(|_| writable_store_path(id, &format!("{dir}/file")))?;
    std::fs::create_dir_all(to.parent().unwrap_or(&to))?;
    let part = to.with_file_name(format!(".{}.{}.part", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let got = std::fs::OpenOptions::new().write(true).create_new(true).open(&part).map_err(anyhow::Error::from).and_then(|mut f| {
        let got = get(&mut f)?;
        std::io::Write::flush(&mut f)?;
        Ok(got)
    });
    let placed = got.and_then(|(head, size)| std::fs::rename(&part, &to).map(|_| (to, head.kind, size)).map_err(Into::into));
    let _ = std::fs::remove_file(&part);
    placed
}

/// The host and port of an http(s) address (`[…]` around an IPv6 address taken off).
fn host_port(url: &str) -> Option<(String, u16, &'static str)> {
    let (scheme, rest) = ["https", "http"].into_iter().find_map(|s| url.get(..s.len() + 3).filter(|p| p.eq_ignore_ascii_case(&format!("{s}://"))).map(|_| (s, &url[s.len() + 3..])))?;
    let auth = rest.split(['/', '?', '#']).next()?;
    if auth.is_empty() || auth.contains('@') {
        return None;
    }
    let (host, port) = match auth.strip_prefix('[') {
        Some(r) => {
            let (h, after) = r.split_once(']')?;
            (h.to_string(), after.strip_prefix(':'))
        }
        None => match auth.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), Some(p)),
            None => (auth.to_string(), None),
        },
    };
    let port = match port {
        Some(p) => p.parse().ok().filter(|p| *p > 0)?,
        None if scheme == "https" => 443,
        None => 80,
    };
    Some((host.to_ascii_lowercase(), port, scheme))
}

/// A request sent to the fake service a development build's tests map `host` to (`url`'s path kept), else `None`; a
/// release build never.
fn fake_for(host: &str, url: &str) -> Result<Option<String>> {
    test_host(&format!("https://{host}{}", url.split_once("://").and_then(|(_, r)| r.find('/').map(|i| &r[i..])).unwrap_or("")), &[host.to_string()])
}

/// How a picture of a page is reached: as the plugin may reach anything (`http::reach`: its grants, the fake service of a
/// development build), or — only for a page on the public internet (`page`: its host and port) — from its own host when
/// that is public, as a browser shows it: https (http only on an http page), port 80 or 443 (or the page's own, on the
/// page's own host), a name (or an IPv4 address), at the address Echo checked (`resolve`), all by `until`. A page on the
/// user's own network shows only pictures the plugin may reach.
fn picture_reach(id: &str, src: &str, page: Option<&(String, u16)>, page_scheme: &str, until: Instant) -> Result<(Reach, Option<String>)> {
    if let Ok(r) = reach(id, src) {
        return Ok((r, None));
    }
    let Some((page_host, page_port)) = page else {
        bail!("not an address this plugin may reach, and a page on your own network or this computer gets only those");
    };
    let (host, port, scheme) = host_port(src).ok_or_else(|| anyhow!("not an http(s) address"))?;
    // An address written as one: checked first, as it is; an IPv6 one isn't reached (only a name or an IPv4 address).
    if let Ok(ip) = host.parse::<IpAddr>() {
        public_address(&host, port, until)?;
        if ip.is_ipv6() {
            bail!("a picture at an IPv6 address written as one: Echo reaches a picture's host by its name");
        }
    }
    if !WEB_PORTS.contains(&port) && !(host == *page_host && port == *page_port) {
        bail!("a picture on port {port}: Echo fetches a page's pictures only from ports 80 and 443 (or the page's own)");
    }
    if scheme == "http" && page_scheme == "https" {
        bail!("a plain http picture on an https page");
    }
    // A development build's tests map the host to a fake service, or fail rather than reach the real one.
    if let Some(fake) = fake_for(&host, src)? {
        return Ok((Reach { listed: false, fake: Some(fake) }, None));
    }
    let ip = public_address(&host, port, until)?;
    let ip = if ip.is_ipv6() { format!("[{ip}]") } else { ip.to_string() };
    Ok((Reach { listed: false, fake: None }, Some(format!("{host}:{port}:{ip}"))))
}

/// Whether plugin `id`'s sheet has `uses.network.web`: it reads any https page on the public internet.
fn reads_web(id: &str) -> bool {
    package_dir(id).and_then(|d| plugins::read_manifest(&d)).is_ok_and(|m| m.uses.get("network").is_some_and(|n| n["web"] == true))
}

/// How a page of the public internet is reached for a plugin with `uses.network.web`: https, port 80 or 443, a host name
/// or an IPv4 address (an IPv6 one written as one refused, as for a picture) whose every address is public, at the
/// address Echo checked and never through a proxy (`resolve`), looked up by `until`.
fn web_reach(url: &str, until: Instant) -> Result<(Reach, Option<String>)> {
    let (host, port, scheme) = host_port(url).ok_or_else(|| anyhow!("“{}” isn't an https address", url.chars().take(200).collect::<String>()))?;
    if scheme != "https" {
        bail!("{host} over plain http: Echo reads a page on the public internet only over https");
    }
    if !WEB_PORTS.contains(&port) {
        bail!("{host} on port {port}: Echo reads a page on the public internet only on ports 80 and 443");
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        public_address(&host, port, until)?;
        if ip.is_ipv6() {
            bail!("a page at an IPv6 address written as one: Echo reaches a page's host by its name");
        }
    }
    // A development build's tests map the host to a fake service, or fail rather than reach the real one.
    if let Some(fake) = fake_for(&host, url)? {
        return Ok((Reach { listed: false, fake: Some(fake) }, None));
    }
    let ip = public_address(&host, port, until)?;
    Ok((Reach { listed: false, fake: None }, Some(format!("{host}:{port}:{ip}"))))
}

/// How plugin `id` reaches `url`: as `ctx.http.fetch` may (`first`: the address the user typed too, once), else — with
/// `web` — as a page on the public internet (`web_reach`); `public_only` (a redirect from a public page): only as that,
/// never at an address of the plugin's grants. Whether it was reached as a public page.
fn page_reach(id: &str, url: &str, web: bool, first: bool, public_only: bool, until: Instant) -> Result<(Reach, Option<String>, bool)> {
    let granted = if public_only {
        Err(anyhow!("a public page moved"))
    } else if first {
        admit(id, url, true)
    } else {
        reach(id, url)
    };
    match granted {
        Ok(r) => Ok((r, None, false)),
        Err(_) if web => {
            let (r, resolve) = web_reach(url, until).map_err(|e| anyhow!("{id} reads only pages on the public internet and the addresses it may reach: {e:#}"))?;
            Ok((r, resolve, true))
        }
        Err(e) => Err(e),
    }
}

/// A redirect `get_page` follows: where to.
#[derive(Debug)]
struct Moved(String);

impl std::fmt::Display for Moved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "moved to {}", self.0)
    }
}

impl std::error::Error for Moved {}

/// A plain GET of `url` through `get_capped`, `secs` in all. Without `uses.network.web` exactly that: only what the
/// plugin may reach, no redirect followed. With it, a public page is reached too (`web_reach`), and a redirect (301,
/// 302, 303, 307, 308) is followed up to `HOPS` times — its `Location` read against the address it came from, and
/// that address checked again before anything is asked of it: as the first was (never the typed one) when the first
/// was one of the plugin's grants, else — a page reached as public — only as a public page (never into the addresses
/// its grants name, the user's own network). Stopped at `secs`. The address read last, how it was reached, its head
/// and size.
#[allow(clippy::too_many_arguments)]
fn get_page(id: &str, url: &str, max: u64, secs: u64, accept_types: &str, accept: &dyn Fn(&Head) -> Result<()>, sink: &mut dyn std::io::Write) -> Result<(String, Reach, Head, u64)> {
    let web = reads_web(id);
    let until = Instant::now() + Duration::from_secs(secs);
    let (mut reach, mut resolve, mut public_only) = page_reach(id, url, web, true, false, until)?;
    let mut at = url.to_string();
    let mut hops = 0;
    loop {
        let follow = |h: &Head| match &h.location {
            Some(to) if web && REDIRECTS.contains(&h.status) => Err(Moved(to.clone()).into()),
            _ => accept(h),
        };
        let rest = until.saturating_duration_since(Instant::now());
        if rest.is_zero() {
            bail!("the answer took longer than {secs} s");
        }
        let left = (rest.as_millis().div_ceil(1000) as u64).min(secs);
        let err = match get_capped(id, &at, &reach, max, left, accept_types, resolve.as_deref(), &follow, sink) {
            Ok((head, n)) => return Ok((at, reach, head, n)),
            Err(e) => e,
        };
        let Some(Moved(to)) = err.downcast_ref::<Moved>() else { return Err(err) };
        if hops == HOPS {
            bail!("the page moved more than {HOPS} times: Echo follows {HOPS} redirects at most");
        }
        hops += 1;
        let next = page::web_link(&at, to).ok_or_else(|| anyhow!("the page moved to “{}”, which isn't a web address Echo reads", to.chars().take(200).collect::<String>()))?;
        let was_public;
        (reach, resolve, was_public) = page_reach(id, &next, web, false, public_only, until).map_err(|e| anyhow!("the page moved to {next}, which Echo doesn't read: {e:#}"))?;
        // Once any page on the way was a public one, every later address must be public too.
        public_only |= was_public;
        at = next;
    }
}

/// `images: 'save'`: each picture of the page (`page::Image`) with what became of it — `saved` a handle {file, name,
/// size, type} (in the store's `cache/web/<page>/`, swept with the cache) or `error`, why not. Within the caps above;
/// GET only, picture types only, every one reached by `picture_reach`.
fn save_pictures(id: &str, page_url: &str, page_reach: &Reach, images: &[Image]) -> Vec<Value> {
    let until = Instant::now() + Duration::from_secs(IMAGES_SECS);
    let page = host_port(page_url).filter(|(h, p, _)| page_reach.fake.is_some() || public_address(h, *p, until).is_ok()).map(|(h, p, _)| (h, p));
    let page_scheme = if page_url.get(..8).is_some_and(|s| s.eq_ignore_ascii_case("https://")) { "https" } else { "http" };
    let dir = format!("cache/web/{}", &folders::sha_hex(page_url.as_bytes())[..16]);
    let mut total = 0u64;
    images
        .iter()
        .enumerate()
        .map(|(n, img)| {
            let mut v = json!({"src": img.src, "alt": img.alt, "caption": img.caption, "width": img.width, "height": img.height});
            let saved = if n >= IMAGES_MOST {
                Err(anyhow!("past the {IMAGES_MOST} pictures Echo saves for a page"))
            } else if total >= IMAGES_TOTAL {
                Err(anyhow!("past the {} MB Echo saves for a page", IMAGES_TOTAL >> 20))
            } else if Instant::now() > until {
                Err(anyhow!("past the {IMAGES_SECS} s Echo takes for a page's pictures"))
            } else {
                picture_reach(id, &img.src, page.as_ref(), page_scheme, until).and_then(|(r, resolve)| {
                    let max = IMAGE_MAX.min(IMAGES_TOTAL - total);
                    // Each picture within what is left of the pictures' time, not only checked between them.
                    let secs = until.saturating_duration_since(Instant::now()).as_secs().clamp(1, IMAGE_SECS);
                    let (path, kind, size) = save(id, &dir, &format!("{:02}", n + 1), |sink| get_capped(id, &img.src, &r, max, secs, "image/avif,image/webp,image/png,image/jpeg,image/gif,image/svg+xml", resolve.as_deref(), &picture_only, sink))?;
                    total += size;
                    let ext = PICTURES.iter().find(|(k, _)| *k == kind).map_or("img", |(_, e)| e);
                    let named = path.with_extension(ext);
                    std::fs::rename(&path, &named)?;
                    let mut h = files::make(id, &named)?;
                    h["type"] = json!(kind);
                    Ok(h)
                })
            };
            match saved {
                Ok(h) => v["saved"] = h,
                Err(e) => v["error"] = json!(format!("{e:#}")),
            }
            v
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::public::all_public;
    use std::io::{Read as _, Write as _};
    use std::sync::atomic::AtomicUsize;

    /// What the made-up site saw: requests by path, and how much of the endless page it managed to send.
    #[derive(Default)]
    struct Seen {
        paths: Mutex<Vec<String>>,
        big_sent: AtomicU64,
        conns: AtomicUsize,
    }

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n made-up picture";

    /// A made-up site on 127.0.0.1 (`other`: the port of a second one the plugin isn't given).
    fn site(other: u16) -> (u16, Arc<Seen>) {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let seen = Arc::new(Seen::default());
        let s2 = seen.clone();
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let seen = s2.clone();
                seen.conns.fetch_add(1, Ordering::SeqCst);
                std::thread::spawn(move || answer(s, other, &seen));
            }
        });
        (port, seen)
    }

    fn answer(mut s: std::net::TcpStream, other: u16, seen: &Seen) {
        let mut req = vec![];
        let mut buf = [0u8; 2048];
        while !req.windows(4).any(|w| w == b"\r\n\r\n") {
            match s.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => req.extend_from_slice(&buf[..n]),
            }
        }
        let req = String::from_utf8_lossy(&req).into_owned();
        let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
        seen.paths.lock().unwrap().push(path.clone());
        let mut reply = |status: &str, head: &str, body: &[u8]| {
            let _ = write!(s, "HTTP/1.1 {status}\r\n{head}Content-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = s.write_all(body);
        };
        let filler = "The pilot moves to November; three stores go first and the rest follow in two waves after the holidays. ";
        match path.split('?').next().unwrap_or("") {
            "/page" => {
                let html = format!(
                    "<html lang=\"en\"><head><title>Pilot notes</title><script>alert(1)</script></head><body><article><h1>Pilot notes</h1><p>{filler}</p>\
                     <figure><img src=\"/img/a.png\" alt=\"Waves\" width=\"640\" height=\"360\"><figcaption>The three waves</figcaption></figure>\
                     <img src=\"/img/pixel.gif\" width=\"1\" height=\"1\"><img src=\"/img/notpic\" alt=\"not one\"><img src=\"/img/huge\" alt=\"huge\">\
                     <img src=\"http://127.0.0.1:{other}/img/a.png\" alt=\"elsewhere\"><p>See <a href=\"/plan\">the plan</a>.</p></article></body></html>"
                );
                reply("200 OK", "Content-Type: text/html; charset=utf-8\r\n", html.as_bytes());
            }
            "/many" => {
                let imgs: String = (0..60).map(|i| format!("<img src=\"/img/a.png?n={i}\" alt=\"{i}\">")).collect();
                reply("200 OK", "Content-Type: text/html\r\n", format!("<main><p>{filler}</p>{imgs}</main>").as_bytes());
            }
            "/public" => {
                let html = format!("<article><p>{filler}</p><img src=\"http://127.0.0.1:{other}/img/a.png\" alt=\"local\"><img src=\"https://cdn.acme.example/img/a.png\" alt=\"cdn\"></article>");
                reply("200 OK", "Content-Type: text/html\r\n", html.as_bytes());
            }
            "/img/a.png" => reply("200 OK", "Content-Type: image/png\r\n", PNG),
            "/img/notpic" => reply("200 OK", "Content-Type: text/html\r\n", b"<p>no</p>"),
            "/img/huge" => reply("200 OK", "Content-Type: image/png\r\n", &vec![0u8; 11 << 20]),
            "/pdf" | "/files/Plan%20v2.pdf" => reply("200 OK", "Content-Type: application/pdf\r\n", b"%PDF-1.4 made up"),
            "/gz" => reply("200 OK", "Content-Type: text/html\r\nContent-Encoding: gzip\r\n", &[0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3]),
            "/moved" => reply("302 Found", "Location: /target\r\n", b""),
            // A chain of redirects: /hop/N moves N more times, the last one to the page (relative, with dots).
            p if p.starts_with("/hop/") => match p[5..].parse::<u32>() {
                Ok(0) => reply("301 Moved Permanently", "Location: ../page\r\n", b""),
                Ok(n) => reply("307 Temporary Redirect", &format!("Location: /hop/{}\r\n", n - 1), b""),
                Err(_) => reply("404 Not Found", "", b""),
            },
            // A public page that moves somewhere it may not go (or to another public host).
            p if p.starts_with("/away/") => {
                let to = match &p[6..] {
                    "loopback" => "https://127.0.0.1/page".to_string(),
                    "ten" => "https://10.0.0.1/page".into(),
                    "metadata" => "https://169.254.169.254/latest/meta-data/".into(),
                    "localhost" => "https://localhost/page".into(),
                    "v6" => "https://[::1]/page".into(),
                    "http" => "http://news.acme.example/page".into(),
                    "port" => format!("https://news.acme.example:{other}/page"),
                    "file" => "file:///etc/hosts".into(),
                    "listed" => format!("http://127.0.0.1:{other}/page"),
                    // From the user's own site out to a public page that moves back in.
                    "bounce" => "https://news.acme.example/away/listed".into(),
                    _ => "https://cdn.acme.example/page".into(),
                };
                reply("302 Found", &format!("Location: {to}\r\n"), b"")
            }
            "/cp1251" => {
                let mut body = format!("<main><p>{filler}</p><p>").into_bytes();
                body.extend_from_slice(&[0xcf, 0xf0, 0xe8, 0xe2, 0xe5, 0xf2]);
                reply("200 OK", "Content-Type: text/html; charset=windows-1251\r\n", &body);
            }
            "/slow" => {
                std::thread::sleep(Duration::from_secs(3));
                reply("200 OK", "Content-Type: text/html\r\n", b"<p>late</p>");
            }
            "/big" => {
                // 50 MB of a page with no length said: written until the reader goes.
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n");
                let chunk = vec![b'a'; 64 << 10];
                for _ in 0..800 {
                    if s.write_all(&chunk).is_err() {
                        return;
                    }
                    seen.big_sent.fetch_add(chunk.len() as u64, Ordering::SeqCst);
                }
            }
            _ => reply("404 Not Found", "Content-Type: text/html\r\n", b"<p>no</p>"),
        }
    }

    // A public page's pictures below come from a development build's fake hosts: a release build would reach for the real ones.
    #[cfg(debug_assertions)]
    #[test]
    fn a_plugin_reads_a_page_it_may_reach_within_its_caps_and_saves_its_pictures() {
        let (_env, root) = paths::testenv::folders("jshost-web");
        let (other, other_seen) = site(1);
        let (port, seen) = site(other);
        let mk = root.join("market");
        let d = mk.join("plugins/reader");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("echo-plugin.json"),
            json!({"id": "reader", "name": "Reader", "version": "0.1.0", "main": "main.js",
                   "uses": {"network": {"hosts": ["docs.acme.example"], "hosts_from": ["site"]}},
                   "settings": [{"key": "site", "label": "Site", "type": "string"}]})
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            d.join("main.js"),
            r#"
            export function apply(ctx) {
              ctx.command('ping', () => 'pong')
              ctx.command('read', async (a) => await ctx.web.read(a.url, a.opts || {}))
              ctx.command('fetch', async (a) => await ctx.web.fetch(a.url, a.opts || {}))
              ctx.command('md', async (a) => await ctx.web.markdown(a.html, a.opts || {}))
              ctx.command('text', (a) => ctx.files.read(a.file))
            }
            "#,
        )
        .unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "reader", "source": "./plugins/reader"}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        let sheet = plugins::review("reader", Some(&name)).unwrap().uses;
        plugins::install("reader", Some(&name), &sheet).unwrap();
        plugins::set_setting("reader", "site", &json!(format!("http://127.0.0.1:{port}"))).unwrap();
        sync();
        let up = (0..50).find_map(|_| call("reader", "ping", &json!({})).ok().or_else(|| {
            std::thread::sleep(Duration::from_millis(50));
            None
        }));
        assert_eq!(up, Some(json!("pong")));
        let at = |p: &str| format!("http://127.0.0.1:{port}{p}");
        let read = |url: &str, opts: Value| call("reader", "read", &json!({"url": url, "opts": opts}));
        let refused = |r: Result<Value>| r.unwrap_err().to_string();

        // Only what ctx.http.fetch may reach, refused before anything is asked.
        let asked = other_seen.conns.load(Ordering::SeqCst);
        assert!(refused(read(&format!("http://127.0.0.1:{other}/page"), json!({}))).contains("may reach only"));
        assert!(refused(read("https://news.example/", json!({}))).contains("may reach only"));
        assert!(refused(read("file:///etc/hosts", json!({}))).contains("may reach only"));
        assert_eq!(other_seen.conns.load(Ordering::SeqCst), asked, "nothing asked of a site not granted");

        // A page: Markdown, links, pictures listed (no tracking pixel), its own address back.
        let p = read(&at("/page"), json!({})).unwrap();
        assert_eq!(p["url"], json!(at("/page")));
        assert_eq!(p["title"], json!("Pilot notes"));
        assert_eq!(p["lang"], json!("en"));
        let md = p["markdown"].as_str().unwrap();
        assert!(md.contains(&format!("![Waves](http://127.0.0.1:{port}/img/a.png)\n\nThe three waves")), "{md}");
        assert!(md.contains(&format!("See [the plan](http://127.0.0.1:{port}/plan).")) && !md.contains("alert"), "{md}");
        assert_eq!(p["links"], json!([{"text": "the plan", "href": at("/plan")}]));
        let srcs: Vec<&str> = p["images"].as_array().unwrap().iter().map(|i| i["src"].as_str().unwrap()).collect();
        assert_eq!(srcs, [at("/img/a.png"), at("/img/notpic"), at("/img/huge"), format!("http://127.0.0.1:{other}/img/a.png")]);
        assert!(p["images"][0].get("saved").is_none(), "pictures are saved only when asked");
        assert!(!seen.paths.lock().unwrap().iter().any(|p| p.starts_with("/img")), "nothing but the page fetched");
        assert!(p["bytes"].as_u64().unwrap() > 200);

        // `images: 'save'`: each picture a handle, or why not.
        let p = read(&at("/page"), json!({"images": "save"})).unwrap();
        let imgs = p["images"].as_array().unwrap();
        let saved = &imgs[0]["saved"];
        assert_eq!((saved["name"].as_str(), saved["size"].as_u64(), saved["type"].as_str()), (Some("01.png"), Some(PNG.len() as u64), Some("image/png")), "{imgs:?}");
        assert_eq!(imgs[0]["caption"], json!("The three waves"));
        let file = plugins::data_dir("reader").join(format!("cache/web/{}/01.png", &folders::sha_hex(at("/page").as_bytes())[..16]));
        assert_eq!(std::fs::read(&file).unwrap(), PNG, "in the store's cache");
        assert!(imgs[1]["error"].as_str().unwrap().contains("text/html, not a picture"), "{}", imgs[1]);
        assert!(imgs[2]["error"].as_str().unwrap().contains("over 10 MB"), "{}", imgs[2]);
        assert!(imgs[3]["error"].as_str().unwrap().contains("your own network"), "a page on the user's network shows only what the plugin may reach: {}", imgs[3]);
        assert_eq!(other_seen.conns.load(Ordering::SeqCst), asked, "the unlisted site never asked");
        let left: Vec<String> = std::fs::read_dir(file.parent().unwrap()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, ["01.png"], "nothing half-written, nothing refused kept");
        // 50 pictures at most for a page.
        let p = read(&at("/many"), json!({"images": "save"})).unwrap();
        let imgs = p["images"].as_array().unwrap();
        assert_eq!(imgs.len(), 60);
        assert_eq!(imgs.iter().filter(|i| i.get("saved").is_some()).count(), 50);
        assert!(imgs[50]["error"].as_str().unwrap().contains("past the 50 pictures"), "{}", imgs[50]);

        // HTML only; nothing compressed; no redirect followed; the cap, the time, the charset.
        assert!(refused(read(&at("/pdf"), json!({}))).contains("this is application/pdf, not a web page"));
        assert!(refused(read(&at("/gz"), json!({}))).contains("came compressed (gzip)"));
        assert!(refused(read(&at("/moved"), json!({}))).contains("answered 302 (it moved)"));
        assert!(!seen.paths.lock().unwrap().iter().any(|p| p == "/target"), "a redirect is never followed");
        assert!(refused(read(&at("/missing"), json!({}))).contains("answered 404"));
        let e = refused(read(&at("/big"), json!({})));
        assert!(e.contains("over 5 MB: Echo stopped reading it"), "{e}");
        let sent = seen.big_sent.load(Ordering::SeqCst);
        assert!(sent < 30 << 20, "stopped as it came, not after the 50 MB: {sent}");
        assert!(refused(read(&at("/big"), json!({"max": 100_000}))).contains("over 97 KB"));
        assert!(refused(read(&at("/slow"), json!({"timeout": 1}))).contains("longer than 1 s"));
        assert!(read(&at("/cp1251"), json!({})).unwrap()["markdown"].as_str().unwrap().ends_with("Привет"));

        // A public page's pictures from their own public hosts (a development build's fake stands for them); never from this
        // computer or the user's network.
        let _map = EnvHosts::set(&_env, &format!("docs.acme.example=http://127.0.0.1:{port},cdn.acme.example=http://127.0.0.1:{port}"));
        let p = read("https://docs.acme.example/public", json!({"images": "save"})).unwrap();
        let imgs = p["images"].as_array().unwrap();
        assert_eq!(imgs[0]["saved"], Value::Null, "{imgs:?}");
        assert!(imgs[0]["error"].as_str().unwrap().contains("on this computer or your own network"), "{}", imgs[0]);
        assert_eq!(imgs[1]["saved"]["name"], json!("02.png"), "{imgs:?}");
        drop(_map);

        // One file as a handle; HTML it has read the same way.
        let f = call("reader", "fetch", &json!({"url": at("/files/Plan%20v2.pdf")})).unwrap();
        assert_eq!((f["name"].as_str(), f["type"].as_str(), f["size"].as_u64()), (Some("Plan20v2.pdf"), Some("application/pdf"), Some(16)));
        assert_eq!(call("reader", "text", &json!({"file": f["file"]})).unwrap(), json!("%PDF-1.4 made up"));
        assert!(refused(call("reader", "fetch", &json!({"url": "https://news.example/a.pdf"}))).contains("may reach only"));
        let m = call("reader", "md", &json!({"html": "<p>Hi <a href=\"/x\">there</a> <img src=\"a.png\" alt=\"pic\"></p>", "opts": {"base": "https://acme.example/d/"}})).unwrap();
        assert_eq!(m["markdown"], json!("Hi [there](https://acme.example/x)\n\n![pic](https://acme.example/d/a.png)"));
        assert!(refused(call("reader", "md", &json!({"html": "<p>x</p>", "opts": {"base": "javascript:alert(1)"}}))).contains("base is an http(s) address"));
        plugins::remove("reader", false).unwrap();
    }

    // The public hosts below are a development build's fakes: a release build would reach for the real ones.
    #[cfg(debug_assertions)]
    #[test]
    fn a_plugin_with_network_web_reads_any_public_https_page_following_redirects_each_checked_again() {
        let (_env, root) = paths::testenv::folders("jshost-web-public");
        let (other, other_seen) = site(1);
        let (port, seen) = site(other);
        let mk = root.join("market");
        let d = mk.join("plugins/crawler");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("echo-plugin.json"),
            json!({"id": "crawler", "name": "Crawler", "version": "0.1.0", "main": "main.js", "uses": {"network": {"web": true, "hosts_from": ["site"]}},
                   "settings": [{"key": "site", "label": "Site", "type": "string"}]})
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            d.join("main.js"),
            r#"
            export function apply(ctx) {
              ctx.command('ping', () => 'pong')
              ctx.command('read', async (a) => await ctx.web.read(a.url, a.opts || {}))
              ctx.command('fetch', async (a) => await ctx.web.fetch(a.url, a.opts || {}))
              ctx.command('http', async (a) => await ctx.http.fetch(a.url, {}))
            }
            "#,
        )
        .unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "crawler", "source": "./plugins/crawler"}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        plugins::refresh(&name).unwrap();
        let sheet = plugins::review("crawler", Some(&name)).unwrap().uses;
        assert_eq!(sheet.len(), 3, "{sheet:?}");
        assert_eq!(sheet[2], "Reads web pages on the public internet (https), and can send what it knows to any site through the addresses it opens", "a line of its own");
        plugins::install("crawler", Some(&name), &sheet).unwrap();
        // The user lists the other site (on this computer): the plugin's own grant.
        plugins::set_setting("crawler", "site", &json!(format!("http://127.0.0.1:{other}"))).unwrap();
        sync();
        let up = (0..50).find_map(|_| call("crawler", "ping", &json!({})).ok().or_else(|| {
            std::thread::sleep(Duration::from_millis(50));
            None
        }));
        assert_eq!(up, Some(json!("pong")));
        let _map = EnvHosts::set(&_env, &format!("news.acme.example=http://127.0.0.1:{port},cdn.acme.example=http://127.0.0.1:{port}"));
        let read = |url: &str| call("crawler", "read", &json!({"url": url}));
        let refused = |r: Result<Value>| r.unwrap_err().to_string();

        // A public page, and one five redirects away: read where it ended, its links against that address.
        let p = read("https://news.acme.example/page").unwrap();
        assert_eq!((p["url"].as_str(), p["title"].as_str()), (Some("https://news.acme.example/page"), Some("Pilot notes")));
        let p = read("https://news.acme.example/hop/4").unwrap();
        assert_eq!(p["url"], json!("https://news.acme.example/page"), "five redirects followed, the last one relative");
        assert_eq!(p["links"], json!([{"text": "the plan", "href": "https://news.acme.example/plan"}]));
        let p = read("https://news.acme.example/away/cdn").unwrap();
        assert_eq!(p["url"], json!("https://cdn.acme.example/page"), "to another public host");
        let f = call("crawler", "fetch", &json!({"url": "https://news.acme.example/hop/1"})).unwrap();
        assert_eq!(f["size"].as_u64(), Some(p["bytes"].as_u64().unwrap()), "ctx.web.fetch follows them too");
        // A sixth is refused, and never asked.
        let asked = |p: &str| seen.paths.lock().unwrap().iter().filter(|x| *x == p).count();
        let before = asked("/page");
        assert!(refused(read("https://news.acme.example/hop/5")).contains("moved more than 5 times"));
        assert_eq!(asked("/page"), before, "the page past the fifth redirect never asked");

        // A redirect anywhere but a public https page on port 80 or 443 is refused before it is asked.
        let conns = other_seen.conns.load(Ordering::SeqCst);
        let other_port = format!("on port {other}");
        for (to, why) in [
            ("loopback", "on this computer or your own network"),
            ("ten", "on this computer or your own network"),
            ("metadata", "on this computer or your own network"),
            ("v6", "on this computer or your own network"),
            ("http", "only over https"),
            ("port", other_port.as_str()),
            ("file", "isn't a web address Echo reads"),
        ] {
            let e = refused(read(&format!("https://news.acme.example/away/{to}")));
            assert!(e.contains("the page moved to") && e.contains(why), "{to}: {e}");
        }
        assert_eq!(other_seen.conns.load(Ordering::SeqCst), conns, "the other port never asked");

        // The first address the same way: this computer, the user's network, plain http, another port.
        for url in [
            "https://127.0.0.1/page",
            "https://10.1.2.3/page",
            "https://172.16.0.1/page",
            "https://192.168.68.1/page",
            "https://169.254.169.254/latest/meta-data/",
            "https://[::1]/page",
            "https://[fd00::1]/page",
            "https://[::ffff:192.168.1.1]/page",
        ] {
            let e = refused(read(url));
            assert!(e.contains("reads only pages on the public internet") && e.contains("your own network"), "{url}: {e}");
        }
        assert!(refused(read("http://news.acme.example/page")).contains("only over https"));
        assert!(refused(read(&format!("https://news.acme.example:{port}/page"))).contains(&format!("on port {port}")));
        assert!(refused(read(&format!("http://127.0.0.1:{port}/page"))).contains("only over https"), "not listed");
        // A public page never moves into an address the user listed for the plugin (their own network).
        let e = refused(read("https://news.acme.example/away/listed"));
        assert!(e.contains(&format!("the page moved to http://127.0.0.1:{other}/page")) && e.contains("only over https"), "{e}");
        assert_eq!(other_seen.conns.load(Ordering::SeqCst), conns, "nothing asked of this computer");
        // A page at the listed address keeps its grant's rules, its own redirects followed there.
        let p = read(&format!("http://127.0.0.1:{other}/hop/1")).unwrap();
        assert_eq!(p["url"], json!(format!("http://127.0.0.1:{other}/page")));
        // Listed → public → listed: once a public page is on the way, never back into the user's network.
        let asked_page = |s: &Seen| s.paths.lock().unwrap().iter().filter(|x| *x == "/page").count();
        let before = asked_page(&other_seen);
        let e = refused(read(&format!("http://127.0.0.1:{other}/away/bounce")));
        assert!(e.contains(&format!("the page moved to http://127.0.0.1:{other}/page")) && e.contains("only over https"), "{e}");
        assert_eq!(asked_page(&other_seen), before, "the listed site's page never asked again");
        // ctx.http.fetch isn't widened.
        assert!(refused(call("crawler", "http", &json!({"url": "https://news.acme.example/page"}))).contains("may reach only"));
        drop(_map);
        plugins::remove("crawler", false).unwrap();
    }

    #[test]
    fn a_public_page_is_reached_at_the_public_address_echo_checked() {
        let (_env, _root) = paths::testenv::folders("jshost-web-reach");
        let until = Instant::now() + Duration::from_secs(10);
        let (r, resolve) = web_reach("https://8.8.8.8/news", until).unwrap();
        assert!(r.fake.is_none() && !r.listed);
        assert_eq!(resolve.as_deref(), Some("8.8.8.8:443:8.8.8.8"), "pinned: curl connects there and to no proxy");
        assert_eq!(web_reach("https://1.1.1.1:80/", until).unwrap().1.as_deref(), Some("1.1.1.1:80:1.1.1.1"));
        let refused = |u: &str| web_reach(u, until).err().expect("refused").to_string();
        for u in ["https://127.0.0.1/", "https://10.0.0.1/", "https://172.16.5.4/", "https://192.168.1.1/", "https://169.254.169.254/", "https://[::1]/", "https://[fd00::1]/", "https://[::ffff:10.0.0.1]/", "https://localhost/"] {
            assert!(refused(u).contains("your own network"), "{u}");
        }
        assert!(refused("https://[2606:4700:4700::1111]/").contains("IPv6"));
        assert!(refused("http://8.8.8.8/").contains("only over https"));
        assert!(refused("https://8.8.8.8:8443/").contains("port 8443"));
        assert!(refused("https://user@8.8.8.8/").contains("isn't an https address"));
        assert!(refused("ftp://8.8.8.8/").contains("isn't an https address"));
        assert!(refused("https://2130706433/").contains("isn't a host name"), "an address written as one number");
        // A name: every address it has must be public; one into the user's network refuses it all.
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        assert_eq!(all_public("news.acme.example", &[ip("93.184.215.14"), ip("2606:2800:21f:cb07:6820:80da:af6b:8b2c")]).unwrap(), ip("93.184.215.14"));
        assert!(all_public("intranet.acme.example", &[ip("10.0.0.7")]).unwrap_err().to_string().contains("your own network"));
        assert!(all_public("mixed.acme.example", &[ip("93.184.215.14"), ip("192.168.1.10")]).unwrap_err().to_string().contains("your own network"));
        assert!(all_public("v6.acme.example", &[ip("fd00::1")]).is_err());
        // A name inside the user's network and one not found read the same: no name can be probed.
        let said = |h: &str, ips: &[IpAddr]| all_public(h, ips).unwrap_err().to_string().replace(h, "<host>");
        assert_eq!(said("none.acme.example", &[]), said("intranet.acme.example", &[ip("10.0.0.7")]));
        assert_eq!(said("none.acme.example", &[]), said("mixed.acme.example", &[ip("93.184.215.14"), ip("192.168.1.10")]));    }

    /// `ECHOCTL_TEST_HOSTS` until dropped, set only while the test holds the test environment (its lock: the variable is
    /// process-wide).
    #[cfg(debug_assertions)]
    struct EnvHosts<'a>(std::marker::PhantomData<&'a paths::testenv::Env>);
    #[cfg(debug_assertions)]
    impl<'a> EnvHosts<'a> {
        fn set(_env: &'a paths::testenv::Env, v: &str) -> Self {
            std::env::set_var("ECHOCTL_TEST_HOSTS", v);
            EnvHosts(std::marker::PhantomData)
        }
    }
    #[cfg(debug_assertions)]
    impl Drop for EnvHosts<'_> {
        fn drop(&mut self) {
            std::env::remove_var("ECHOCTL_TEST_HOSTS");
        }
    }

    #[test]
    fn a_public_pages_pictures_come_only_from_web_ports_and_never_an_ipv6_literal() {
        let (_env, _root) = paths::testenv::folders("jshost-web-ports");
        let until = Instant::now() + Duration::from_secs(10);
        let page = ("1.1.1.1".to_string(), 8443);
        let pic = |src: &str| picture_reach("nobody", src, Some(&page), "https", until);
        let refused = |src: &str| pic(src).err().expect("refused").to_string();
        assert!(refused("https://8.8.8.8:22/a.png").contains("port 22"));
        assert!(refused("https://8.8.8.8:8443/a.png").contains("port 8443"), "the page's port is its own host's only");
        assert!(refused("https://cdn.acme.example:6379/a.png").contains("port 6379"), "refused before any lookup");
        assert!(refused("https://[2606:4700:4700::1111]/a.png").contains("IPv6"));
        assert!(refused("https://[::1]/a.png").contains("your own network"));
        assert!(refused("http://8.8.8.8/a.png").contains("plain http"));
        assert_eq!(pic("https://8.8.8.8/a.png").unwrap().1.as_deref(), Some("8.8.8.8:443:8.8.8.8"));
        assert_eq!(pic("https://1.1.1.1:8443/a.png").unwrap().1.as_deref(), Some("1.1.1.1:8443:1.1.1.1"), "the page's own host and port");
        assert!(picture_reach("nobody", "https://8.8.8.8/a.png", None, "https", until).err().expect("refused").to_string().contains("your own network"), "a page that isn't public");
    }

    #[test]
    fn a_file_keeps_a_plain_name_of_its_address() {
        assert_eq!(file_name("https://acme.example/docs/Plan%20v2.pdf?x=1"), "Plan20v2.pdf");
        assert_eq!(file_name("https://acme.example/"), "file");
        assert_eq!(file_name("https://acme.example/../.hidden"), "hidden");
        assert_eq!(host_port("https://[::1]:8443/x"), Some(("::1".into(), 8443, "https")));
        assert_eq!(host_port("http://Acme.Example/x"), Some(("acme.example".into(), 80, "http")));
        assert_eq!(host_port("https://u@acme.example/"), None);
    }

    #[test]
    fn a_page_is_read_in_the_charset_it_names() {
        // "Привет" in windows-1251, named by the server or by the page itself; bad bytes never fail.
        let cp1251 = [0xcf, 0xf0, 0xe8, 0xe2, 0xe5, 0xf2];
        assert_eq!(decode_html(&cp1251, Some("windows-1251")), "Привет");
        let mut page = b"<meta charset=\"windows-1251\"><p>".to_vec();
        page.extend_from_slice(&cp1251);
        assert!(decode_html(&page, None).ends_with("Привет"));
        assert_eq!(decode_html(&[0xff, 0xfe, 0x41, 0x00], None), "A", "a byte-order mark");
        assert_eq!(decode_html(&[0x41, 0xff, 0x42], Some("no-such-charset")), "A\u{fffd}B");
    }
}
