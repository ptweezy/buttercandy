//! Icons, described as a few vector primitives on a 24-unit grid and
//! rasterised on demand at exactly the pixel size they are drawn at, by
//! evaluating signed distances on the CPU.

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Icon {
    Play,
    Pause,
    Next,
    Prev,
    Volume,
    Mute,
    Mic,
    Monitor,
    Music,
    List,
    Sliders,
    Expand,
    Shrink,
    Swatch,
    Sparkles,
    Close,
    Wave,
    Plus,
    ChevronLeft,
    ChevronRight,
    Check,
    Stop,
}

enum Shape {
    /// Stroked segment.
    Line(f32, f32, f32, f32),
    /// Stroked circle: centre, radius.
    Ring(f32, f32, f32),
    /// Stroked arc: centre, radius, start and end angle in degrees
    /// (0 = right, clockwise on screen).
    Arc(f32, f32, f32, f32, f32),
    /// Filled polygon with corners rounded by the given radius.
    Poly(&'static [(f32, f32)], f32),
    /// Filled rounded rectangle: x, y, w, h, radius.
    Bar(f32, f32, f32, f32, f32),
    /// Stroked rounded rectangle.
    Frame(f32, f32, f32, f32, f32),
}

use Shape::*;

const STROKE: f32 = 1.9;

fn shapes(icon: Icon) -> &'static [Shape] {
    match icon {
        Icon::Play => &[Poly(&[(8.6, 5.6), (18.6, 12.0), (8.6, 18.4)], 1.1)],
        Icon::Pause => &[Bar(6.2, 4.8, 4.2, 14.4, 1.3), Bar(13.6, 4.8, 4.2, 14.4, 1.3)],
        Icon::Next => &[
            Poly(&[(6.4, 6.6), (14.6, 12.0), (6.4, 17.4)], 0.9),
            Line(18.4, 6.0, 18.4, 18.0),
        ],
        Icon::Prev => &[
            Poly(&[(17.6, 6.6), (9.4, 12.0), (17.6, 17.4)], 0.9),
            Line(5.6, 6.0, 5.6, 18.0),
        ],
        Icon::Volume => &[
            Poly(
                &[
                    (4.6, 10.0),
                    (7.8, 10.0),
                    (11.6, 6.6),
                    (11.6, 17.4),
                    (7.8, 14.0),
                    (4.6, 14.0),
                ],
                0.7,
            ),
            Arc(11.5, 12.0, 4.4, -48.0, 48.0),
            Arc(11.5, 12.0, 8.0, -48.0, 48.0),
        ],
        Icon::Mute => &[
            Poly(
                &[
                    (4.6, 10.0),
                    (7.8, 10.0),
                    (11.6, 6.6),
                    (11.6, 17.4),
                    (7.8, 14.0),
                    (4.6, 14.0),
                ],
                0.7,
            ),
            Line(15.8, 9.6, 20.6, 14.4),
            Line(20.6, 9.6, 15.8, 14.4),
        ],
        Icon::Mic => &[
            Frame(9.0, 3.2, 6.0, 11.0, 3.0),
            Arc(12.0, 11.2, 6.4, 0.0, 180.0),
            Line(12.0, 17.8, 12.0, 20.8),
        ],
        Icon::Monitor => &[
            Frame(3.2, 4.6, 17.6, 11.8, 2.2),
            Line(8.6, 20.0, 15.4, 20.0),
            Line(12.0, 16.6, 12.0, 19.8),
        ],
        Icon::Music => &[
            Line(9.2, 17.6, 9.2, 6.2),
            Line(9.2, 6.2, 18.8, 4.2),
            Line(18.8, 4.2, 18.8, 15.6),
            Ring(6.8, 17.8, 2.4),
            Ring(16.4, 15.8, 2.4),
        ],
        Icon::List => &[
            Line(4.4, 6.6, 19.6, 6.6),
            Line(4.4, 12.0, 19.6, 12.0),
            Line(4.4, 17.4, 13.6, 17.4),
        ],
        Icon::Sliders => &[
            Line(4.2, 7.2, 12.0, 7.2),
            Ring(14.8, 7.2, 2.3),
            Line(17.8, 7.2, 19.8, 7.2),
            Line(4.2, 16.8, 6.2, 16.8),
            Ring(9.2, 16.8, 2.3),
            Line(12.2, 16.8, 19.8, 16.8),
        ],
        Icon::Expand => &[
            Line(4.4, 9.0, 4.4, 4.6),
            Line(4.4, 4.6, 8.8, 4.6),
            Line(19.6, 9.0, 19.6, 4.6),
            Line(19.6, 4.6, 15.2, 4.6),
            Line(4.4, 15.0, 4.4, 19.4),
            Line(4.4, 19.4, 8.8, 19.4),
            Line(19.6, 15.0, 19.6, 19.4),
            Line(19.6, 19.4, 15.2, 19.4),
        ],
        Icon::Shrink => &[
            Line(8.8, 4.6, 8.8, 9.0),
            Line(8.8, 9.0, 4.4, 9.0),
            Line(15.2, 4.6, 15.2, 9.0),
            Line(15.2, 9.0, 19.6, 9.0),
            Line(8.8, 19.4, 8.8, 15.0),
            Line(8.8, 15.0, 4.4, 15.0),
            Line(15.2, 19.4, 15.2, 15.0),
            Line(15.2, 15.0, 19.6, 15.0),
        ],
        Icon::Swatch => &[Ring(9.0, 9.4, 4.6), Ring(15.0, 9.4, 4.6), Ring(12.0, 14.6, 4.6)],
        Icon::Sparkles => &[
            Poly(
                &[
                    (11.0, 3.6),
                    (12.9, 8.6),
                    (17.9, 10.5),
                    (12.9, 12.4),
                    (11.0, 17.4),
                    (9.1, 12.4),
                    (4.1, 10.5),
                    (9.1, 8.6),
                ],
                0.5,
            ),
            Poly(
                &[
                    (18.2, 14.6),
                    (19.1, 16.9),
                    (21.4, 17.8),
                    (19.1, 18.7),
                    (18.2, 21.0),
                    (17.3, 18.7),
                    (15.0, 17.8),
                    (17.3, 16.9),
                ],
                0.3,
            ),
        ],
        Icon::Close => &[Line(6.4, 6.4, 17.6, 17.6), Line(17.6, 6.4, 6.4, 17.6)],
        Icon::Wave => &[
            Line(4.0, 10.8, 4.0, 13.2),
            Line(8.0, 8.0, 8.0, 16.0),
            Line(12.0, 4.6, 12.0, 19.4),
            Line(16.0, 8.0, 16.0, 16.0),
            Line(20.0, 10.8, 20.0, 13.2),
        ],
        Icon::Plus => &[Line(12.0, 5.4, 12.0, 18.6), Line(5.4, 12.0, 18.6, 12.0)],
        Icon::ChevronLeft => &[Line(14.4, 6.0, 8.6, 12.0), Line(8.6, 12.0, 14.4, 18.0)],
        Icon::ChevronRight => &[Line(9.6, 6.0, 15.4, 12.0), Line(15.4, 12.0, 9.6, 18.0)],
        Icon::Check => &[Line(5.4, 12.6, 9.8, 17.0), Line(9.8, 17.0, 18.6, 7.4)],
        Icon::Stop => &[Bar(6.6, 6.6, 10.8, 10.8, 2.4)],
    }
}

pub(super) fn segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (pax, pay, bax, bay) = (px - ax, py - ay, bx - ax, by - ay);
    let len2 = bax * bax + bay * bay;
    let h = if len2 > 0.0 {
        ((pax * bax + pay * bay) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (pax - bax * h).hypot(pay - bay * h)
}

pub(super) fn round_rect(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let qx = (px - (x + w * 0.5)).abs() - w * 0.5 + r;
    let qy = (py - (y + h * 0.5)).abs() - h * 0.5 + r;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

/// Signed distance to a polygon (negative inside).
pub(super) fn polygon(px: f32, py: f32, pts: &[(f32, f32)]) -> f32 {
    let n = pts.len();
    let mut d = (px - pts[0].0).powi(2) + (py - pts[0].1).powi(2);
    let mut sign = 1.0;
    let mut j = n - 1;
    for i in 0..n {
        let (ex, ey) = (pts[j].0 - pts[i].0, pts[j].1 - pts[i].1);
        let (wx, wy) = (px - pts[i].0, py - pts[i].1);
        let t = ((wx * ex + wy * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
        let (bx, by) = (wx - ex * t, wy - ey * t);
        d = d.min(bx * bx + by * by);
        let c = [py >= pts[i].1, py < pts[j].1, ex * wy > ey * wx];
        if c.iter().all(|&b| b) || c.iter().all(|&b| !b) {
            sign = -sign;
        }
        j = i;
    }
    sign * d.sqrt()
}

fn distance(shape: &Shape, x: f32, y: f32) -> f32 {
    let half = STROKE * 0.5;
    match *shape {
        Line(ax, ay, bx, by) => segment(x, y, ax, ay, bx, by) - half,
        Ring(cx, cy, r) => ((x - cx).hypot(y - cy) - r).abs() - half,
        Arc(cx, cy, r, a0, a1) => {
            // Flatten to a polyline; plenty at icon sizes.
            const STEPS: usize = 16;
            let point = |i: usize| {
                let a = (a0 + (a1 - a0) * i as f32 / STEPS as f32).to_radians();
                (cx + r * a.cos(), cy + r * a.sin())
            };
            let mut d = f32::MAX;
            for i in 0..STEPS {
                let (a, b) = (point(i), point(i + 1));
                d = d.min(segment(x, y, a.0, a.1, b.0, b.1));
            }
            d - half
        }
        Poly(pts, r) => polygon(x, y, pts) - r,
        Bar(bx, by, w, h, r) => round_rect(x, y, bx, by, w, h, r),
        Frame(bx, by, w, h, r) => round_rect(x, y, bx, by, w, h, r).abs() - half,
    }
}

/// Rasterise `icon` into a `px x px` RGBA coverage mask.
pub fn rasterize(icon: Icon, px: u32) -> Vec<u8> {
    let scale = px as f32 / 24.0;
    let list = shapes(icon);
    let mut out = Vec::with_capacity((px * px * 4) as usize);
    for y in 0..px {
        for x in 0..px {
            let (ux, uy) = ((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
            let d = list.iter().map(|s| distance(s, ux, uy)).fold(f32::MAX, f32::min);
            let a = (0.5 - d * scale).clamp(0.0, 1.0);
            out.extend_from_slice(&[255, 255, 255, (a * 255.0 + 0.5) as u8]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_distance_is_signed() {
        let tri = [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)];
        assert!(polygon(2.0, 2.0, &tri) < 0.0);
        assert!(polygon(8.0, 8.0, &tri) > 0.0);
        assert!((polygon(5.0, -3.0, &tri) - 3.0).abs() < 1e-4);
    }

    #[test]
    fn every_icon_rasterises_with_ink_and_margin() {
        use Icon::*;
        for icon in [
            Play,
            Pause,
            Next,
            Prev,
            Volume,
            Mute,
            Mic,
            Monitor,
            Music,
            List,
            Sliders,
            Expand,
            Shrink,
            Swatch,
            Sparkles,
            Close,
            Wave,
            Plus,
            ChevronLeft,
            ChevronRight,
            Check,
            Stop,
        ] {
            let px = 48;
            let img = rasterize(icon, px);
            let alpha = |x: u32, y: u32| img[((y * px + x) * 4 + 3) as usize];
            let ink = (0..px * px).filter(|i| img[(*i * 4 + 3) as usize] > 128).count();
            assert!(ink > 60, "{icon:?} is nearly empty ({ink} px)");
            // Nothing should touch the very edge of the cell.
            for i in 0..px {
                assert!(
                    alpha(i, 0) == 0 && alpha(i, px - 1) == 0 && alpha(0, i) == 0 && alpha(px - 1, i) == 0,
                    "{icon:?} clips"
                );
            }
        }
    }
}
