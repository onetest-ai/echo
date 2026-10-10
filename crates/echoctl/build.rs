fn main() {
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
