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
    // Windows: Echo's icon and name in echoctl.exe (the taskbar names a program by its FileDescription,
    // #420). Only when building on Windows (a host check): the Windows check from macOS has no
    // resource compiler and needs neither.
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=../../packaging/windows/echo.rc");
        println!("cargo:rerun-if-changed=../../packaging/windows/echo.ico");
        // The numeric version ("0.9.0-beta" → 0, 9, 0) for the resource's FILEVERSION.
        let version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
        let mut n = version.split('-').next().unwrap_or_default().split('.').map(|p| p.parse::<u16>().unwrap_or(0));
        let macros = ["ECHO_V1", "ECHO_V2", "ECHO_V3"].map(|m| format!("{m}={}", n.next().unwrap_or(0)));
        embed_resource::compile("../../packaging/windows/echo.rc", &macros)
            .manifest_optional()
            .expect("embedding Echo's icon and name");
    }
}

/// The parts of `dir`: files named `NN-<area>.ext`, in name order, joined as they are. Each must end with a newline
/// (a part ending in a `// comment` would otherwise swallow the next part's first line), and any other `.ext` file
/// there stops the build, so a scratch file can't reach the page. `ui/source.mjs` joins them by the same rule.
fn join_parts(dir: &str, ext: &str) -> Vec<u8> {
    let mut parts: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {dir}: {e}"))
        .map(|e| e.expect("a directory entry").path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == ext))
        .collect();
    parts.sort();
    assert!(!parts.is_empty(), "no .{ext} parts in {dir}");
    let mut out = Vec::new();
    for p in &parts {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let b = name.as_bytes();
        let stem = &name[..name.len() - ext.len() - 1];
        assert!(
            stem.len() > 3 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[2] == b'-' && stem[3..].bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'),
            "{}: a part's name is NN-<area>.{ext} (lowercase, digits, dashes)",
            p.display()
        );
        let text = std::fs::read(p).unwrap_or_else(|e| panic!("reading {}: {e}", p.display()));
        assert!(text.ends_with(b"\n"), "{} must end with a newline", p.display());
        out.extend(text);
    }
    out
}
