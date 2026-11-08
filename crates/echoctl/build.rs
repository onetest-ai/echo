fn main() {
    // The window's script and styles: parts per area in `ui/app/*.js` and `ui/css/*.css`, joined in name order into
    // one `app.js` and one `app.css` (window.rs compiles them in). One classic script, as before: functions still hoist
    // across the parts, and the page, its CSP and `index.html` don't change.
    println!("cargo:rerun-if-changed=build.rs");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    for (dir, ext, name) in [("ui/app", "js", "app.js"), ("ui/css", "css", "app.css")] {
        println!("cargo:rerun-if-changed={dir}");
        std::fs::write(out.join(name), join_parts(dir, ext)).expect("writing the window's assets");
    }
    // Windows gives the main thread 1 MB of stack (macOS 8 MB): clap's parse of the command tree and
    // the app's start overflowed it in 0.4.1-beta (`thread 'main' has overflowed its stack`). 8 MB, as
    // on macOS; reserved address space, committed only as used.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    }
    // Windows: Echo's icon in echoctl.exe. Only when building on Windows (a host check): the
    // Windows check from macOS has no resource compiler and needs no icon.
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=../../packaging/windows/echo.rc");
        println!("cargo:rerun-if-changed=../../packaging/windows/echo.ico");
        embed_resource::compile("../../packaging/windows/echo.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("embedding Echo's icon");
    }
}

/// The files of `dir` ending in `.ext`, in name order, joined as they are (no separator).
fn join_parts(dir: &str, ext: &str) -> Vec<u8> {
    let mut parts: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {dir}: {e}"))
        .map(|e| e.expect("a directory entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == ext))
        .collect();
    parts.sort();
    assert!(!parts.is_empty(), "no .{ext} parts in {dir}");
    parts.iter().flat_map(|p| std::fs::read(p).unwrap_or_else(|e| panic!("reading {}: {e}", p.display()))).collect()
}
