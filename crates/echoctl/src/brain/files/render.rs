//! LibreOffice (`soffice`, a system tool — Brain's dependency too) turns a deck or a Word file into a
//! PDF once per version of the file; its pages are then measured and rendered like any PDF's.
//! Hidden slides stay out of its PDF, as they stay out of the XML reader, so slide N is page N.

use anyhow::{anyhow, bail, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// LibreOffice, if this computer has it.
pub fn soffice() -> Option<PathBuf> {
    let mut c: Vec<PathBuf> = vec![];
    #[cfg(target_os = "macos")]
    c.extend(["/Applications/LibreOffice.app/Contents/MacOS/soffice", "/opt/homebrew/bin/soffice", "/usr/local/bin/soffice"].map(PathBuf::from));
    #[cfg(windows)]
    c.extend([r"C:\Program Files\LibreOffice\program\soffice.exe", r"C:\Program Files (x86)\LibreOffice\program\soffice.exe"].map(PathBuf::from));
    #[cfg(all(unix, not(target_os = "macos")))]
    c.extend(["/usr/bin/soffice", "/usr/local/bin/soffice"].map(PathBuf::from));
    c.into_iter().find(|p| p.exists())
}

/// `src` as a PDF at `out` (made once: an existing `out` is used as it is). Its own profile folder, so
/// a LibreOffice the user has open doesn't take the job; at most 3 minutes.
pub fn to_pdf(src: &Path, out: &Path) -> Result<PathBuf> {
    if out.exists() {
        return Ok(out.to_path_buf());
    }
    let so = soffice().ok_or_else(|| anyhow!("LibreOffice isn't installed (it lays out decks and Word files for the page reader)"))?;
    let work = out.with_extension("convert");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(work.join("profile"))?;
    // A file URL: file:///Users/… (Unix), file:///C:/… (Windows).
    let dir = work.join("profile").to_string_lossy().replace('\\', "/").replace(' ', "%20");
    let profile = format!("-env:UserInstallation=file://{}{dir}", if dir.starts_with('/') { "" } else { "/" });
    let mut child = Command::new(&so)
        .arg(&profile)
        .args(["--headless", "--norestore", "--convert-to", "pdf", "--outdir"])
        .arg(&work)
        .arg(src)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("running LibreOffice: {e}"))?;
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if start.elapsed() > Duration::from_secs(180) || crate::notes::quitting() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_dir_all(&work);
            bail!("LibreOffice took over 3 minutes");
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let made = work.join(format!("{}.pdf", src.file_stem().and_then(|s| s.to_str()).unwrap_or("doc")));
    if !status.success() || !made.exists() {
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            use std::io::Read;
            let _ = e.read_to_string(&mut err);
        }
        let _ = std::fs::remove_dir_all(&work);
        let said = err.lines().rfind(|l| l.to_lowercase().contains("error")).unwrap_or("").trim().to_string();
        bail!("LibreOffice couldn't lay it out — not a readable Office file (truncated download?){}", if said.is_empty() { String::new() } else { format!(": {said}") });
    }
    std::fs::rename(&made, out)?;
    let _ = std::fs::remove_dir_all(&work);
    Ok(out.to_path_buf())
}
