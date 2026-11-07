//! The window's page and its compiled-in assets (`/`, `/assets/*`).

use super::*;

pub(super) const INDEX: &str = include_str!("../../ui/index.html");
// Joined at build time from the parts in `ui/app/` and `ui/css/` (build.rs).
pub(super) const APP_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/app.js"));
pub(super) const APP_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/app.css"));
pub(super) const OCTOPUS_SVG: &str = include_str!("../../assets/octopus.svg");
/// The onetest logo (violet disc, the full octopus character), 192 px, for the window.
pub(super) const LOGO_PNG: &[u8] = include_bytes!("../../assets/onetest-logo.png");
/// xterm.js (MIT, `ui/vendor/README.md`): the terminals in the window.
pub(super) const XTERM_JS: &str = include_str!("../../ui/vendor/xterm.js");
pub(super) const XTERM_FIT_JS: &str = include_str!("../../ui/vendor/xterm-addon-fit.js");
pub(super) const XTERM_CSS: &str = include_str!("../../ui/vendor/xterm.css");
/// Sigma.js + graphology (MIT, `ui/vendor/README.md`): the Brain's graphs, drawn with WebGL.
pub(super) const SIGMA_JS: &str = include_str!("../../ui/vendor/sigma.min.js");
pub(super) const GRAPHOLOGY_JS: &str = include_str!("../../ui/vendor/graphology.umd.min.js");
pub(super) const GRAPHOLOGY_LIB_JS: &str = include_str!("../../ui/vendor/graphology-library.min.js");

/// The page with the octopus mark defined once as an SVG symbol (`<use href="#octopus">`).
pub(super) fn index_html() -> String {
    let inner = OCTOPUS_SVG
        .split_once('>')
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.rsplit_once("</svg>").map(|(body, _)| body))
        .unwrap_or("");
    let symbol = format!(
        "<svg width=\"0\" height=\"0\" style=\"position:absolute\" aria-hidden=\"true\"><symbol id=\"octopus\" viewBox=\"0 0 100 100\" fill=\"currentColor\">{inner}</symbol></svg>"
    );
    INDEX.replace("<!--OCTOPUS-->", &symbol)
}


/// The echo:// requests of this area (`respond` asks each area in turn).
pub(super) fn route(path: &str, req: &Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>> {
    let _ = req;
    Some(match path {
        "/" | "/index.html" => reply(200, "text/html; charset=utf-8", index_html().into_bytes()),
        "/assets/app.js" => reply(200, "text/javascript; charset=utf-8", APP_JS.as_bytes().to_vec()),
        "/assets/app.css" => reply(200, "text/css; charset=utf-8", APP_CSS.as_bytes().to_vec()),
        "/assets/logo.png" => reply(200, "image/png", LOGO_PNG.to_vec()),
        "/assets/vendor/xterm.js" => reply(200, "text/javascript; charset=utf-8", XTERM_JS.as_bytes().to_vec()),
        "/assets/vendor/xterm-addon-fit.js" => reply(200, "text/javascript; charset=utf-8", XTERM_FIT_JS.as_bytes().to_vec()),
        "/assets/vendor/xterm.css" => reply(200, "text/css; charset=utf-8", XTERM_CSS.as_bytes().to_vec()),
        "/assets/vendor/sigma.min.js" => reply(200, "text/javascript; charset=utf-8", SIGMA_JS.as_bytes().to_vec()),
        "/assets/vendor/graphology.umd.min.js" => reply(200, "text/javascript; charset=utf-8", GRAPHOLOGY_JS.as_bytes().to_vec()),
        "/assets/vendor/graphology-library.min.js" => reply(200, "text/javascript; charset=utf-8", GRAPHOLOGY_LIB_JS.as_bytes().to_vec()),
        _ => return None,
    })
}
