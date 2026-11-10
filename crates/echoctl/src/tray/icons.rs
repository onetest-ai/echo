//! The tray: its icons and the app's icon files (split out of `tray.rs`, moved as it was).

use super::*;

pub(super) const SIZE: u32 = 36;

/// Recording: warm orange; armed: green; idle: black as a template (follows the menu bar),
/// white on a dark Windows taskbar.
pub(super) const ORANGE: [u8; 3] = [0xD9, 0x77, 0x57];
pub(super) const BLACK: [u8; 3] = [0, 0, 0];
pub(super) const WHITE: [u8; 3] = [0xFF, 0xFF, 0xFF];
pub(super) const GREEN: [u8; 3] = [52, 199, 89];

/// The octopus mark (`mark.rs`) in `color`, SIZE×SIZE.
pub(super) fn logo(color: [u8; 3]) -> Icon {
    Icon::from_rgba(crate::mark::mark_rgba(SIZE, color), SIZE, SIZE).expect("valid icon")
}

/// The app icon (Dock-style, for Echo.app), RGBA `size`×`size`.
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    crate::mark::app_icon_rgba(size)
}

/// Minimal PNG encoder (stored deflate blocks) — enough for build-time icons, no dependency.
pub fn png(rgba: &[u8], size: u32) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend(data);
        out.extend(&body);
        out.extend(crc32(&body).to_be_bytes());
    }
    let mut raw = Vec::with_capacity(rgba.len() + size as usize);
    for row in rgba.chunks(size as usize * 4) {
        raw.push(0); // filter: none
        raw.extend(row);
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, b) in blocks.iter().enumerate() {
        z.push((i + 1 == blocks.len()) as u8);
        z.extend((b.len() as u16).to_le_bytes());
        z.extend((!(b.len() as u16)).to_le_bytes());
        z.extend(*b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = vec![];
    ihdr.extend(size.to_be_bytes());
    ihdr.extend(size.to_be_bytes());
    ihdr.extend([8, 6, 0, 0, 0]); // 8-bit RGBA
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

pub(super) const ICONSET: [(&str, u32); 10] = [
    ("icon_16x16", 16), ("icon_16x16@2x", 32), ("icon_32x32", 32), ("icon_32x32@2x", 64),
    ("icon_128x128", 128), ("icon_128x128@2x", 256), ("icon_256x256", 256), ("icon_256x256@2x", 512),
    ("icon_512x512", 512), ("icon_512x512@2x", 1024),
];

/// `echoctl dev-icon <file>.ico [--from logo.png]`: the Windows app icon (embedded in
/// echoctl.exe by build.rs) at 16…256 px, each a PNG entry (Windows Vista+ reads those). With
/// `--from` (the onetest logo) it is the macOS icon's artwork — the octopus on the logo's purple
/// rounded square — without Apple's margin, so it fills the tile like other Windows icons;
/// otherwise the drawn mark.
pub(super) fn write_ico(path: &std::path::Path, from: Option<&std::path::Path>) -> anyhow::Result<()> {
    const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];
    use image::ImageEncoder;
    let art = match from {
        Some(src) => {
            let (_, art) = logo_icon(&image::open(src)?.to_rgba8(), 1024, true);
            let m = (1024 - 824) / 2; // logo_icon draws macOS's 824/1024 body
            Some(image::imageops::crop_imm(&art, m, m, 824, 824).to_image())
        }
        None => None,
    };
    let pngs = SIZES
        .iter()
        .map(|&s| {
            let rgba = match &art {
                Some(a) => image::imageops::resize(a, s, s, image::imageops::FilterType::Lanczos3).into_raw(),
                None => app_icon_rgba(s),
            };
            let mut buf = vec![];
            image::codecs::png::PngEncoder::new(&mut buf).write_image(&rgba, s, s, image::ExtendedColorType::Rgba8)?;
            Ok(buf)
        })
        .collect::<anyhow::Result<Vec<Vec<u8>>>>()?;
    let mut out = vec![0, 0, 1, 0];
    out.extend((SIZES.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * SIZES.len() as u32;
    for (&s, data) in SIZES.iter().zip(&pngs) {
        let side = if s >= 256 { 0 } else { s as u8 }; // 0 means 256
        out.extend([side, side, 0, 0]);
        out.extend(1u16.to_le_bytes()); // colour planes
        out.extend(32u16.to_le_bytes()); // bits per pixel
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for data in pngs {
        out.extend(data);
    }
    std::fs::write(path, out)?;
    Ok(())
}

/// `echoctl dev-icon <dir> [--from logo.png]`: write an .iconset (for `iconutil`), or with a
/// `<dir>.icon` path an Icon Composer document (for `actool`, macOS 26's native icon). With
/// `--from`, the icon is built from that image (the onetest logo); otherwise the drawn mark.
pub fn write_iconset(dir: &std::path::Path, from: Option<&std::path::Path>) -> anyhow::Result<()> {
    if dir.extension().is_some_and(|e| e == "ico") {
        return write_ico(dir, from);
    }
    std::fs::create_dir_all(dir)?;
    #[cfg(target_os = "macos")]
    if let Some(src) = from {
        let logo = image::open(src)?.to_rgba8();
        if dir.extension().is_some_and(|e| e == "icon") {
            // The purple fill comes from icon.json; the octopus is the one layer, and macOS
            // draws the shape, margins and edge itself.
            let (purple, layer) = logo_icon(&logo, 1024, false);
            std::fs::create_dir_all(dir.join("Assets"))?;
            layer.save(dir.join("Assets/octopus.png"))?;
            let c = |i: usize| format!("{:.5}", purple[i] as f32 / 255.0);
            let doc = serde_json::json!({
                "fill": { "solid": format!("srgb:{},{},{},1.00000", c(0), c(1), c(2)) },
                "groups": [{
                    "layers": [{ "image-name": "octopus.png", "name": "octopus", "glass": false }],
                    "shadow": { "kind": "neutral", "opacity": 0.5 },
                    "translucency": { "enabled": false, "value": 0.5 }
                }],
                "supported-platforms": { "squares": ["macOS"] }
            });
            std::fs::write(dir.join("icon.json"), serde_json::to_vec_pretty(&doc)?)?;
            return Ok(());
        }
        let (_, art) = logo_icon(&logo, 1024, true);
        for (name, px) in ICONSET {
            let img = image::imageops::resize(&art, px, px, image::imageops::FilterType::Lanczos3);
            img.save(dir.join(format!("{name}.png")))?;
        }
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    let _ = from;
    for (name, px) in ICONSET {
        std::fs::write(dir.join(format!("{name}.png")), png(&app_icon_rgba(px), px))?;
    }
    Ok(())
}

/// The app icon from the round onetest logo (white octopus on a purple disc), with the octopus
/// as large as fits. `squircle`: the whole icon, the octopus on a purple rounded square in
/// Apple's 824/1024 grid (the .icns for macOS before 26). Otherwise only the octopus, white on
/// transparent, for an Icon Composer layer. Returns the logo's purple too.
#[cfg(any(target_os = "macos", windows))]
pub(super) fn logo_icon(logo: &image::RgbaImage, px: u32, squircle: bool) -> (image::Rgba<u8>, image::RgbaImage) {
    let (w, h) = logo.dimensions();
    let purple = *logo.get_pixel(w / 2, h / 40); // inside the disc, above the octopus
    // How white a logo pixel is (the octopus), 0..1, from the green channel (purple is low).
    let base = purple[1] as f32;
    let white = |x: u32, y: u32| {
        let p = logo.get_pixel(x, y);
        (((p[1] as f32 - base) / (255.0 - base)).clamp(0.0, 1.0)) * (p[3] as f32 / 255.0)
    };
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if white(x, y) > 0.5 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    // The body is the icon's shape: 824/1024 of the .icns, the whole Icon Composer canvas.
    let body = if squircle { px as f32 * 824.0 / 1024.0 } else { px as f32 };
    let margin = (px as f32 - body) / 2.0;
    let (ow, oh) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
    let scale = body * if squircle { 0.84 } else { 0.80 } / ow.max(oh);
    let (ox, oy) = ((px as f32 - ow * scale) / 2.0, (px as f32 - oh * scale) / 2.0);
    let mut out = image::RgbaImage::new(px, px);
    const SS: u32 = 4; // supersampling for smooth edges
    for y in 0..px {
        for x in 0..px {
            let (mut cover, mut wsum) = (0.0f32, 0.0f32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let fx = x as f32 + (sx as f32 + 0.5) / SS as f32;
                    let fy = y as f32 + (sy as f32 + 0.5) / SS as f32;
                    // Superellipse (n = 5), close to Apple's continuous-corner shape.
                    let u = ((fx - px as f32 / 2.0) / (body / 2.0)).abs();
                    let v = ((fy - px as f32 / 2.0) / (body / 2.0)).abs();
                    if squircle && (u.powi(5) + v.powi(5) > 1.0 || fx < margin || fy < margin) {
                        continue;
                    }
                    cover += 1.0;
                    let lx = x0 as f32 + (fx - ox) / scale;
                    let ly = y0 as f32 + (fy - oy) / scale;
                    if lx >= x0 as f32 && ly >= y0 as f32 && lx < (x1 + 1) as f32 && ly < (y1 + 1) as f32 {
                        wsum += white(lx as u32, ly as u32);
                    }
                }
            }
            let n = (SS * SS) as f32;
            if !squircle {
                if wsum > 0.0 {
                    out.put_pixel(x, y, image::Rgba([255, 255, 255, (wsum / n * 255.0).round() as u8]));
                }
            } else if cover > 0.0 {
                let t = wsum / cover;
                let mix = |c: u8| (c as f32 * (1.0 - t) + 255.0 * t).round() as u8;
                let a = (cover / n * 255.0).round() as u8;
                out.put_pixel(x, y, image::Rgba([mix(purple[0]), mix(purple[1]), mix(purple[2]), a]));
            }
        }
    }
    (purple, out)
}
