fn main() {
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
