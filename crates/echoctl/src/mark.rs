//! The Echo mark: the onetest octopus, simplified to a silhouette that still reads at
//! menu-bar size — a round head with two big eyes, two arms waving up and four chunky tentacles
//! flaring out like the logo's skirt (no smile, spots or tuft: they vanish at 18 pt). Drawn from signed-distance
//! shapes in a unit square, so the menu-bar states (one colour, 36 px) and the app icon (white
//! on onetest violet, up to 1024 px) come from one definition. No image or SVG dependency.

/// Signed distance (negative inside) at a point of the unit square (y down).
type Sdf = Box<dyn Fn(f32, f32) -> f32>;

fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Sdf {
    Box::new(move |x, y| (((x - cx) / rx).hypot((y - cy) / ry) - 1.0) * rx.min(ry))
}

fn rounded_box(cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> Sdf {
    Box::new(move |x, y| {
        let (dx, dy) = ((x - cx).abs() - hw + r, (y - cy).abs() - hh + r);
        dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - r
    })
}

fn capsule(a: (f32, f32), b: (f32, f32), r: f32, x: f32, y: f32) -> f32 {
    let (px, py, vx, vy) = (x - a.0, y - a.1, b.0 - a.0, b.1 - a.1);
    let h = ((px * vx + py * vy) / (vx * vx + vy * vy).max(1e-9)).clamp(0.0, 1.0);
    (px - vx * h).hypot(py - vy * h) - r
}

fn bezier(p: [(f32, f32); 4], n: usize) -> Vec<(f32, f32)> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let (a, b, c, d) = ((1.0 - t).powi(3), 3.0 * t * (1.0 - t).powi(2), 3.0 * t * t * (1.0 - t), t.powi(3));
            (a * p[0].0 + b * p[1].0 + c * p[2].0 + d * p[3].0, a * p[0].1 + b * p[1].1 + c * p[2].1 + d * p[3].1)
        })
        .collect()
}

/// A stroke along `path` whose radius goes from `r0` to `r1`; also its bounding box for a
/// cheap early-out.
/// One stroke piece: from, to, radius.
type Segment = ((f32, f32), (f32, f32), f32);

struct Taper {
    segs: Vec<Segment>,
    bbox: (f32, f32, f32, f32),
}

impl Taper {
    fn new(path: Vec<(f32, f32)>, r0: f32, r1: f32) -> Self {
        let n = (path.len() - 1) as f32;
        let segs: Vec<_> = path
            .windows(2)
            .enumerate()
            .map(|(i, w)| (w[0], w[1], r0 + (r1 - r0) * (i as f32 + 0.5) / n))
            .collect();
        let pad = r0.max(r1);
        let bbox = path.iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |b, p| {
            (b.0.min(p.0 - pad), b.1.min(p.1 - pad), b.2.max(p.0 + pad), b.3.max(p.1 + pad))
        });
        Taper { segs, bbox }
    }

    fn dist(&self, x: f32, y: f32) -> f32 {
        let (x0, y0, x1, y1) = self.bbox;
        if x < x0 || x > x1 || y < y0 || y > y1 {
            // Outside the (radius-padded) box the stroke is farther than any fillet reaches:
            // "far" — a small lower bound here would make the soft join bleed along the box.
            return f32::MAX;
        }
        self.segs.iter().map(|(a, b, r)| capsule(*a, *b, *r, x, y)).fold(f32::MAX, f32::min)
    }
}

/// Polynomial smooth minimum: joins shapes with a soft fillet of size `k`.
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k / 4.0
}

/// The octopus shape. `dist(x, y) < 0` = inside.
pub struct Octopus {
    head: Sdf,
    belly: Sdf,
    eyes: [Sdf; 2],
    tentacles: Vec<Taper>,
}

impl Octopus {
    pub fn new() -> Self {
        // Outer pair sweeps out and lifts at the tip; inner pair goes down and out.
        let outer = bezier([(0.33, 0.58), (0.24, 0.70), (0.14, 0.72), (0.07, 0.64)], 28);
        let inner = bezier([(0.43, 0.62), (0.40, 0.74), (0.34, 0.82), (0.27, 0.85)], 28);
        // "Hands": thinner arms from the sides of the head, waving up, clear of the tentacles.
        let arm = bezier([(0.25, 0.50), (0.17, 0.51), (0.11, 0.46), (0.10, 0.37)], 28);
        let mirror = |p: &Vec<(f32, f32)>| p.iter().map(|(x, y)| (1.0 - x, *y)).collect::<Vec<_>>();
        let tentacles = vec![
            Taper::new(mirror(&outer), 0.085, 0.028),
            Taper::new(mirror(&inner), 0.080, 0.028),
            Taper::new(mirror(&arm), 0.055, 0.024),
            Taper::new(outer, 0.085, 0.028),
            Taper::new(inner, 0.080, 0.028),
            Taper::new(arm, 0.055, 0.024),
        ];
        Octopus {
            head: ellipse(0.5, 0.38, 0.28, 0.29),
            belly: ellipse(0.5, 0.58, 0.25, 0.11),
            eyes: [ellipse(0.40, 0.42, 0.055, 0.075), ellipse(0.60, 0.42, 0.055, 0.075)],
            tentacles,
        }
    }

    pub fn dist(&self, x: f32, y: f32) -> f32 {
        // Tentacles meet the body with a soft fillet (one moulded body) but each other plainly —
        // a fillet between the inner pair would bridge the gap under the belly.
        let tentacles = self.tentacles.iter().map(|t| t.dist(x, y)).fold(f32::MAX, f32::min);
        let body = smin(smin((self.head)(x, y), (self.belly)(x, y), 0.06), tentacles, 0.06);
        self.eyes.iter().fold(body, |d, e| d.max(-e(x, y)))
    }

    /// Coverage (0–1) of pixel (px, py) in a `size`² image with `pad` margin, supersampled.
    fn coverage(&self, px: u32, py: u32, size: u32, pad: f32, ss: u32) -> f32 {
        let mut hits = 0;
        for sy in 0..ss {
            for sx in 0..ss {
                let x = (px as f32 + (sx as f32 + 0.5) / ss as f32) / size as f32;
                let y = (py as f32 + (sy as f32 + 0.5) / ss as f32) / size as f32;
                hits += (self.dist(pad + x * (1.0 - 2.0 * pad), pad + y * (1.0 - 2.0 * pad)) < 0.0) as u32;
            }
        }
        hits as f32 / (ss * ss) as f32
    }
}

/// The mark alone in `color`, RGBA `size`², transparent background (menu-bar / tray icon).
pub fn mark_rgba(size: u32, color: [u8; 3]) -> Vec<u8> {
    let o = Octopus::new();
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let a = o.coverage(x, y, size, 0.02, 6);
            let i = ((y * size + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[color[0], color[1], color[2], (a * 255.0).round() as u8]);
        }
    }
    rgba
}

/// The app icon, RGBA `size`²: macOS-style rounded square in onetest violet, faint concentric
/// "echo" ripples behind the head, the white octopus.
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    // onetest violet (#8B62FF), a touch lighter at the top.
    const TOP: [f32; 3] = [0x9C as f32, 0x78 as f32, 0xFF as f32];
    const BOTTOM: [f32; 3] = [0x7B as f32, 0x50 as f32, 0xF2 as f32];
    // Faint "echo" ripples behind the head.
    const RINGS: [(f32, f32, f32); 3] = [(0.25, 0.006, 0.10), (0.32, 0.005, 0.07), (0.39, 0.004, 0.045)];
    let (ox, oy, scale) = (0.5, 0.52, 0.62);
    let o = Octopus::new();
    let body = rounded_box(0.5, 0.5, 0.41, 0.41, 0.18);
    let ss = 4;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for py in 0..size {
        for px in 0..size {
            let (mut acc, mut n) = ([0f32; 3], 0u32);
            for sy in 0..ss {
                for sx in 0..ss {
                    let x = (px as f32 + (sx as f32 + 0.5) / ss as f32) / size as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / ss as f32) / size as f32;
                    let inside = body(x, y);
                    if inside >= 0.0 {
                        continue;
                    }
                    let mut c: [f32; 3] = std::array::from_fn(|i| TOP[i] * (1.0 - y) + BOTTOM[i] * y);
                    let r = (x - ox).hypot(y - (oy - 0.07));
                    for (radius, width, alpha) in RINGS {
                        if (r - radius).abs() < width && inside < -0.01 {
                            c = c.map(|v| v * (1.0 - alpha) + 255.0 * alpha);
                        }
                    }
                    if o.dist((x - ox) / scale + 0.5, (y - oy) / scale + 0.5) < 0.0 {
                        c = [255.0; 3];
                    }
                    for i in 0..3 {
                        acc[i] += c[i];
                    }
                    n += 1;
                }
            }
            let i = ((py * size + px) * 4) as usize;
            if n > 0 {
                let a = n as f32 / (ss * ss) as f32;
                rgba[i..i + 4].copy_from_slice(&[(acc[0] / n as f32) as u8, (acc[1] / n as f32) as u8, (acc[2] / n as f32) as u8, (a * 255.0) as u8]);
            }
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn octopus_has_a_body_two_eyes_and_separate_tentacles() {
        let o = Octopus::new();
        let inside = |x, y| o.dist(x, y) < 0.0;
        assert!(inside(0.5, 0.25), "head");
        assert!(!inside(0.40, 0.42) && !inside(0.60, 0.42), "eyes are holes");
        assert!(!inside(0.5, 0.05), "above the head");
        assert!(!inside(0.5, 0.86), "gap between the inner tentacles");
        assert!(inside(0.19, 0.685) && inside(0.81, 0.685), "outer tentacles, mirrored");
        assert!(inside(0.11, 0.42) && inside(0.89, 0.42), "arms");
        assert!(!inside(0.10, 0.55), "a gap between each arm and the tentacle below it");
    }
}
