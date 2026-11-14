//! The buttercandy logo: a wrapped sweet in the shape of a stick of butter.
//!
//! It borrows the look of a supermarket butter stick: soft butter yellow, a
//! single royal-blue ink, and heavy condensed capitals. The twisted wrapper
//! ends make it a candy. Like the icons, it is described as signed distances
//! and rasterised on the CPU at exactly the size it is drawn, so the lettering
//! stays crisp from a 16-pixel icon to a splash screen. The letterforms are
//! drawn here rather than typed, so the logo does not depend on which fonts a
//! machine happens to have.

use super::icons::{polygon, round_rect};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Logo {
    /// The whole label: "BUTTER" over "CANDY", with the net weight in fine
    /// print in the corner. For large sizes.
    Full,
    /// Just "BUTTER". For the top bar, where the small print would not read.
    Compact,
    /// A single "B". For icons.
    Mark,
}

const fn rgb(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 255) as f32 / 255.0,
        ((hex >> 8) & 255) as f32 / 255.0,
        (hex & 255) as f32 / 255.0,
    ]
}

const BODY_TOP: [f32; 3] = rgb(0xFBE57C);
const BODY_BOTTOM: [f32; 3] = rgb(0xF2CD50);
const BODY_EDGE: [f32; 3] = rgb(0xE2B43C);
const SHEEN: [f32; 3] = rgb(0xFFF4B8);
// The wrapper's folds: facing the light, turned from it, and deep in a crease.
const WRAP: [f32; 3] = rgb(0xFCE88A);
const WRAP_SHADE: [f32; 3] = rgb(0xEFC656);
const WRAP_DEEP: [f32; 3] = rgb(0xD39E30);
const INK: [f32; 3] = rgb(0x2358B2);

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ---- lettering -------------------------------------------------------------

/// One step in building a letter. Coordinates are in cap heights: y runs from
/// 0 (cap line) down to 1 (baseline). Steps apply in order.
#[derive(Clone, Copy)]
enum Op {
    /// Add a rectangle: x0, y0, x1, y1.
    Rect(f32, f32, f32, f32),
    /// Add a polygon.
    Poly(&'static [(f32, f32)]),
    /// Cut a rectangle out of everything so far.
    CutRect(f32, f32, f32, f32),
    /// Add a bowl, a box whose right side is a single curve from its top
    /// edge round to its bottom edge: x0, y0, x1, y1, how far before x1 the
    /// curve begins.
    Bowl(f32, f32, f32, f32, f32),
    /// Cut a bowl out of everything so far.
    CutBowl(f32, f32, f32, f32, f32),
    /// Add a cup, a bowl turned to curve along its bottom: x0, y0, x1, y1,
    /// how far above y1 the curve begins.
    Cup(f32, f32, f32, f32, f32),
    /// Cut a cup out of everything so far.
    CutCup(f32, f32, f32, f32, f32),
    /// Add a lobe, a bowl with a straight stretch down its side: x0, y0, x1,
    /// y1, how far before x1 and how far from the top and bottom the curves
    /// begin.
    Lobe(f32, f32, f32, f32, f32, f32),
    /// Cut a lobe out of everything so far.
    CutLobe(f32, f32, f32, f32, f32, f32),
    /// Add an oval with straight sides, the curve of a cup at both ends: x0,
    /// y0, x1, y1, how far from the top and bottom the curves begin.
    Oval(f32, f32, f32, f32, f32),
    /// Cut an oval out of everything so far.
    CutOval(f32, f32, f32, f32, f32),
}

use Op::*;

/// Heavy condensed grotesque capitals: (advance width, construction).
/// Stems are 0.21 wide, horizontal strokes 0.175 thick.
/// Strokes that join overlap rather than touch: a union of shapes that only
/// touch has zero distance along the seam, which would show as a hairline.
fn glyph(c: char) -> (f32, &'static [Op]) {
    match c {
        // B and R are a stem carrying bowls. A bowl swells to 0.215 at its
        // widest, a touch more than the stem, because a round stroke looks
        // lighter than a straight one of the same width. The B's waist sits
        // just above the middle, level with the arm of the E, and its upper
        // bowl stops short of the lower, so that it stands firmly.
        'B' => (
            0.64,
            &[
                Bowl(0.0, 0.0, 0.605, 0.575, 0.315),
                Bowl(0.0, 0.40, 0.64, 1.0, 0.33),
                CutBowl(0.21, 0.175, 0.39, 0.40, 0.105),
                CutBowl(0.21, 0.575, 0.425, 0.825, 0.12),
            ],
        ),
        // The curve of the bowls, turned to sit on the baseline. The counter
        // is cut from above the cap line: a cut that stopped at the line
        // would leave a hairline across the mouth.
        'U' => (
            0.60,
            &[Cup(0.0, 0.0, 0.60, 1.0, 0.31), CutCup(0.21, -0.1, 0.39, 0.825, 0.13)],
        ),
        // Short arms, to keep the holes beneath them small.
        'T' => (0.52, &[Rect(0.0, 0.0, 0.52, 0.175), Rect(0.155, 0.0, 0.365, 1.0)]),
        // The arms step back from the foot: the top a little, the middle,
        // which sits just above half way, a little more.
        'E' => (
            0.52,
            &[
                Rect(0.0, 0.0, 0.21, 1.0),
                Rect(0.0, 0.0, 0.50, 0.175),
                Rect(0.0, 0.40, 0.455, 0.575),
                Rect(0.0, 0.825, 0.52, 1.0),
            ],
        ),
        // The bowl hangs a little lower than the B's, over a short, sturdy
        // leg. The leg is straight and steep, as condensed grotesques draw
        // it, and starts inside the bowl's lower stroke, so that it grows out
        // of the curve instead of being hung beneath it. Measured across the
        // stroke it is the stem's weight, no heavier. Its foot ends just
        // past the bowl, where a square corner looks level with a curve.
        'R' => (
            0.64,
            &[
                Rect(0.0, 0.0, 0.21, 1.0),
                Bowl(0.0, 0.0, 0.62, 0.605, 0.33),
                CutBowl(0.21, 0.175, 0.405, 0.43, 0.115),
                Poly(&[(0.319, 0.47), (0.534, 0.47), (0.64, 1.0), (0.425, 1.0)]),
            ],
        ),
        // C, O and D are built on the U: straight sides that turn through its
        // curve at each end, which is how a condensed grotesque keeps its
        // rounds as narrow as its straights. The C is an O opened on the
        // right. Its arms are cut level, like every other terminal, just
        // past the turn, which leaves a gap that stays open at small sizes.
        'C' => (
            0.60,
            &[
                Oval(0.0, 0.0, 0.60, 1.0, 0.31),
                CutOval(0.21, 0.175, 0.39, 0.825, 0.13),
                CutRect(0.30, 0.355, 0.70, 0.645),
            ],
        ),
        'O' => (
            0.60,
            &[Oval(0.0, 0.0, 0.60, 1.0, 0.31), CutOval(0.21, 0.175, 0.39, 0.825, 0.13)],
        ),
        // The corners of the B's lower bowl, drawn apart to the full height.
        'D' => (
            0.63,
            &[
                Lobe(0.0, 0.0, 0.63, 1.0, 0.33, 0.31),
                CutLobe(0.21, 0.175, 0.42, 0.825, 0.12, 0.13),
            ],
        ),
        // The apex is flat and a stem wide. The legs are a stem wide at the
        // foot and narrow on the inside as they climb, and the crossbar sits
        // low and is a little lighter than the other bars: both make room for
        // the counter, which is the first thing to fill in at small sizes.
        'A' => (
            0.69,
            &[
                Poly(&[
                    (0.0, 1.0),
                    (0.24, 0.0),
                    (0.45, 0.0),
                    (0.69, 1.0),
                    (0.475, 1.0),
                    (0.345, 0.30),
                    (0.215, 1.0),
                ]),
                Rect(0.20, 0.655, 0.49, 0.815),
            ],
        ),
        // The diagonal runs from corner to corner, just lighter across the
        // stroke than the stems, and the letter is wide enough for each
        // counter to reach past the middle. Any narrower and the three
        // strokes merge into a solid band where they cross.
        'N' => (
            0.68,
            &[
                Rect(0.0, 0.0, 0.21, 1.0),
                Rect(0.47, 0.0, 0.68, 1.0),
                Poly(&[(0.0, 0.0), (0.20, 0.0), (0.68, 1.0), (0.48, 1.0)]),
            ],
        ),
        // The arms are a stem's weight at the cap line and narrow towards the
        // fork, like the legs of the A, so that the notch between them runs
        // deep. They meet the stem just below half way.
        'Y' => (
            0.66,
            &[Poly(&[
                (0.0, 0.0),
                (0.225, 0.0),
                (0.33, 0.37),
                (0.435, 0.0),
                (0.66, 0.0),
                (0.435, 0.56),
                (0.435, 1.0),
                (0.225, 1.0),
                (0.225, 0.56),
            ])],
        ),
        // Z, 4 and the full stop appear only in the fine print. The diagonal
        // of the Z leaves each bar exactly at its end, so there is no step
        // under the corner.
        'Z' => (
            0.56,
            &[
                Rect(0.0, 0.0, 0.56, 0.175),
                Rect(0.0, 0.825, 0.56, 1.0),
                Poly(&[
                    (0.385, 0.10),
                    (0.56, 0.10),
                    (0.56, 0.175),
                    (0.175, 0.90),
                    (0.0, 0.90),
                    (0.0, 0.825),
                ]),
            ],
        ),
        // The diagonal is lighter than the stem, which keeps the counter
        // large enough to show at a few pixels.
        '4' => (
            0.67,
            &[
                Rect(0.35, 0.0, 0.56, 1.0),
                Rect(0.0, 0.62, 0.67, 0.78),
                Poly(&[(0.0, 0.62), (0.33, 0.0), (0.505, 0.0), (0.1324, 0.70), (0.0, 0.70)]),
            ],
        ),
        '.' => (0.21, &[Rect(0.0, 0.79, 0.21, 1.0)]),
        // Four strokes have to share the room of two, so they are lighter
        // than a stem and taper towards the joins, which keeps the notches
        // deep enough to stay open at small sizes. It overhangs its advance a
        // little on both sides: a diagonal next to an upright looks further
        // off than it is.
        'W' => (
            0.92,
            &[Poly(&[
                (-0.02, 0.0),
                (0.145, 0.0),
                (0.26, 0.65),
                (0.3775, 0.0),
                (0.5425, 0.0),
                (0.66, 0.65),
                (0.775, 0.0),
                (0.94, 0.0),
                (0.75, 1.0),
                (0.57, 1.0),
                (0.46, 0.35),
                (0.35, 1.0),
                (0.17, 1.0),
            ])],
        ),
        _ => (0.30, &[]),
    }
}

/// Signed distance to a box from `x0` whose two right corners turn through
/// curves `rx` wide and `ry` tall. When `ry` is half the height the two meet,
/// and the right side is one curve from the top edge round to the bottom: a
/// bowl.
///
/// Each curve is a quarter of a superellipse (|x|^2.5 + |y|^2.5 = 1) rather
/// than a circular arc. Its curvature builds from nothing where it leaves
/// the flat edges, so no point shows where the straight ends and the curve
/// begins, and it stays full into the shoulders, the way the rounds of a
/// condensed grotesque do.
///
/// The distance is exact along the straight sides and first-order round the
/// curves: true at the outline, where antialiasing reads it, and approximate
/// far from it. The left corners are mitred, not rounded, so that thinning
/// the letters leaves the corners of a counter square.
fn bowl(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32, (rx, ry): (f32, f32)) -> f32 {
    let qx = px - (x1 - rx);
    let qy = (py - (y0 + y1) * 0.5).abs() - ((y1 - y0) * 0.5 - ry);
    let edge = if qx > 0.0 && qy > 0.0 {
        // `g` is 1 on the curve and grows evenly along any ray from the
        // curve's centre, so `g - 1` over the slope of `g` is a distance.
        let (a, b) = (qx / rx, qy / ry);
        let (a15, b15) = (a * a.sqrt(), b * b.sqrt());
        let s = a * a15 + b * b15;
        let g = s.powf(0.4);
        (g - 1.0) * s / (g * (a15 / rx).hypot(b15 / ry))
    } else {
        (qx - rx).max(qy - ry)
    };
    edge.max(x0 - px)
}

/// Signed distance to an oval filling the box `x0, y0` to `x1, y1`, its sides
/// straight between curves `ry` tall at the top and bottom: two lobes back to
/// back.
fn oval(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32, ry: f32) -> f32 {
    let (cx, rx) = ((x0 + x1) * 0.5, (x1 - x0) * 0.5);
    bowl(cx + (px - cx).abs(), py, f32::MIN, y0, x1, y1, (rx, ry))
}

/// Signed distance to a letter, in cap heights.
fn glyph_sdf(ops: &[Op], x: f32, y: f32) -> f32 {
    let mut d = f32::MAX;
    for op in ops {
        match *op {
            Rect(x0, y0, x1, y1) => d = d.min(round_rect(x, y, x0, y0, x1 - x0, y1 - y0, 0.0)),
            Poly(pts) => d = d.min(polygon(x, y, pts)),
            CutRect(x0, y0, x1, y1) => d = d.max(-round_rect(x, y, x0, y0, x1 - x0, y1 - y0, 0.0)),
            Bowl(x0, y0, x1, y1, r) => d = d.min(bowl(x, y, x0, y0, x1, y1, (r, (y1 - y0) * 0.5))),
            CutBowl(x0, y0, x1, y1, r) => d = d.max(-bowl(x, y, x0, y0, x1, y1, (r, (y1 - y0) * 0.5))),
            Cup(x0, y0, x1, y1, r) => d = d.min(bowl(y, x, y0, x0, y1, x1, (r, (x1 - x0) * 0.5))),
            CutCup(x0, y0, x1, y1, r) => d = d.max(-bowl(y, x, y0, x0, y1, x1, (r, (x1 - x0) * 0.5))),
            Lobe(x0, y0, x1, y1, rx, ry) => d = d.min(bowl(x, y, x0, y0, x1, y1, (rx, ry))),
            CutLobe(x0, y0, x1, y1, rx, ry) => d = d.max(-bowl(x, y, x0, y0, x1, y1, (rx, ry))),
            Oval(x0, y0, x1, y1, ry) => d = d.min(oval(x, y, x0, y0, x1, y1, ry)),
            CutOval(x0, y0, x1, y1, ry) => d = d.max(-oval(x, y, x0, y0, x1, y1, ry)),
        }
    }
    // Letters stop dead at the cap line and the baseline.
    d.max(-y).max(y - 1.0)
}

/// A line of lettering, positioned in logo units (the logo is 1 unit tall).
struct Run {
    text: &'static str,
    x: f32,
    top: f32,
    cap: f32,
    /// Extra space between letters, in cap heights.
    tracking: f32,
    /// How much to thin the strokes, in cap heights (a lighter weight).
    thin: f32,
    /// Horizontal scale of the letters (1 = as drawn).
    stretch: f32,
}

/// How far from a letter's box its distance is worked out, in cap heights.
/// Antialiasing reads distances only within half a pixel of the ink, so
/// letters further off are skipped rather than measured.
const REACH: f32 = 0.25;

/// Adjustment to the space between a pair of letters, in cap heights. Equal
/// gaps do not look equal: the hole under the arm of a T adds itself to the
/// gap beside it, the bowls of a B fall away from their neighbour, and a
/// diagonal leaves a wedge of room where a stem leaves none.
fn kern(left: char, right: char) -> f32 {
    match (left, right) {
        ('T', 'T') => -0.03,
        ('U', 'T') | ('T', 'E') => -0.015,
        ('B', 'U') => -0.01,
        ('E', 'T') => -0.05,
        ('W', 'T') => -0.03,
        // In CANDY only N-D sets stem against stem. The others give up most
        // of what the C's open side and the slopes of the A and Y add.
        ('C', 'A') => -0.08,
        ('A', 'N') => -0.06,
        ('D', 'Y') => -0.09,
        // The full stop tucks in under the arm. The space after it is closed
        // up, and so is the one before the 4: a stop fills only the foot of
        // its width and the 4 has nothing at top left, so both leave more
        // room than the letters around the other word spaces.
        ('T', '.') => -0.12,
        ('.', ' ') | (' ', '4') => -0.10,
        _ => 0.0,
    }
}

impl Run {
    /// Each letter with where it starts, in cap heights from the start of
    /// the run.
    fn letters(&self) -> impl Iterator<Item = (f32, char)> + '_ {
        let mut x = 0.0;
        let mut before = None;
        self.text.chars().map(move |c| {
            if let Some(b) = before.replace(c) {
                x += self.tracking + kern(b, c);
            }
            let start = x;
            x += glyph(c).0;
            (start, c)
        })
    }

    fn width(&self) -> f32 {
        let end = self.letters().last().map_or(0.0, |(start, c)| start + glyph(c).0);
        end * self.cap * self.stretch
    }

    /// Signed distance to the ink, in logo units.
    fn sdf(&self, px: f32, py: f32) -> f32 {
        let v = (py - self.top) / self.cap;
        if !(-REACH..=1.0 + REACH).contains(&v) {
            return f32::MAX;
        }
        let u = (px - self.x) / (self.cap * self.stretch);
        let mut d = f32::MAX;
        for (start, c) in self.letters() {
            let (w, ops) = glyph(c);
            if u > start - REACH && u < start + w + REACH {
                d = d.min(glyph_sdf(ops, u - start, v));
            }
        }
        if d == f32::MAX { d } else { (d + self.thin) * self.cap }
    }
}

// ---- the sweet -------------------------------------------------------------

/// How far each twisted wrapper end sticks out past the body.
const END: f32 = 0.40;
const BODY_Y0: f32 = 0.12;
const BODY_Y1: f32 = 0.88;
const BODY_RADIUS: f32 = 0.075;

struct Layout {
    /// Total width in logo units (height is 1).
    width: f32,
    body_x0: f32,
    body_x1: f32,
    runs: Vec<Run>,
}

fn layout(logo: Logo) -> Layout {
    let x0 = END;
    // The name is set a touch wider and more open than the small print.
    let name = |text, x, top, cap| Run {
        text,
        x,
        top,
        cap,
        tracking: 0.13,
        thin: 0.008,
        stretch: 1.07,
    };
    match logo {
        Logo::Full => {
            // The label spells the name: BUTTER, with CANDY under it in widely
            // spaced small capitals. The stick is as long as the name.
            let butter = name("BUTTER", x0 + 0.20, 0.215, 0.385);
            let right = butter.x + butter.width();
            // Everything under the name stands on one baseline.
            let under = |text, cap, tracking, thin, stretch| Run {
                text,
                x: 0.0,
                top: 0.785 - cap,
                cap,
                tracking,
                thin,
                stretch,
            };
            // CANDY is the other half of the name: the same weight and width
            // as BUTTER, spaced out, and centred under it.
            let mut candy = under("CANDY", 0.13, 0.32, 0.008, 1.07);
            candy.x = (butter.x + right - candy.width()) * 0.5;
            // The net weight a stick of butter carries, as the fine print it
            // is on the real thing, tucked into the corner of the label. It
            // is a few pixels tall on screen, where the weight of the name
            // would clot, so it is cut lighter and wider and set a little
            // open.
            let mut net = under("NET WT. 4 OZ.", 0.038, 0.22, 0.012, 1.22);
            // It ends flush with the name. The closing full stop hangs past
            // the edge: the eye lines up the letters, not the dot.
            net.x = right - net.width() + 0.006;
            let x1 = right + 0.20;
            Layout {
                width: x1 + END,
                body_x0: x0,
                body_x1: x1,
                runs: vec![butter, candy, net],
            }
        }
        Logo::Compact => {
            let butter = name("BUTTER", x0 + 0.21, 0.285, 0.43);
            let x1 = butter.x + butter.width() + 0.21;
            Layout {
                width: x1 + END,
                body_x0: x0,
                body_x1: x1,
                runs: vec![butter],
            }
        }
        Logo::Mark => {
            let b = name("B", x0 + 0.27, 0.24, 0.52);
            let x1 = b.x + b.width() + 0.27;
            Layout {
                width: x1 + END,
                body_x0: x0,
                body_x1: x1,
                runs: vec![b],
            }
        }
    }
}

/// Width divided by height.
pub fn aspect(logo: Logo) -> f32 {
    layout(logo).width
}

/// How far out from the end of the block the wrapper is twisted. Everything
/// gathered runs to this point: the pleats of the fan from one side, and the
/// paper drawn off the end of the block from the other.
const TWIST: f32 = 0.07;

/// How much shorter each wrapper end is drawn in the smallest icons.
const TINY_TRIM: f32 = 0.10;

/// The gathers pulled into the paper between the twist and the block, as
/// (slope, length). They radiate from the twist and die out on the block.
const GATHERS: [(f32, f32); 5] = [(0.0, 0.17), (0.42, 0.21), (-0.42, 0.21), (0.95, 0.19), (-0.95, 0.19)];

/// One twisted wrapper end: the paper is drawn off the end of the block into
/// a twist, then opens into a pleated fan with a pinked edge.
/// `dir` is -1 for the left end, +1 for the right.
struct WrapEnd {
    k: f32,
    dir: f32,
    /// How far out from the end of the block the twist is.
    twist: f32,
    outline: [(f32, f32); 29],
    /// Slopes of the folds to one side of the middle ridge, seen from the
    /// twist: a valley, the next ridge, and the edge of the fan.
    folds: [f32; 3],
}

impl WrapEnd {
    /// `tiny` runs from 0 (as designed) to 1 (a few pixels long), where the
    /// end is a short blunt wedge: fine points and a narrow neck would only
    /// dissolve into the background. `scale` is in pixels to the unit.
    fn new(k: f32, dir: f32, tiny: f32, scale: f32) -> Self {
        // The points of the fan stop short of `END` by their rounding and a
        // pixel more, or the edge of the image would clip them.
        let reach = END - 0.018 - (1.0 / scale).min(0.03) - TINY_TRIM * tiny;
        let twist = TWIST * (1.0 - 0.6 * tiny);
        let spread = 0.39 - 0.07 * tiny;
        let notch = 0.065 * (1.0 - tiny);
        let bell = 0.2 * (1.0 - tiny);
        // The neck is never thinner than a couple of pixels, or a small fan
        // would come away from its block.
        let waist = (1.3 / scale - 0.018).max(0.04);
        // The upper half, as (distance out from the block, height above the
        // middle), from the block round to the middle of the pinked edge.
        let corner = (reach - 0.035 - 0.03 * tiny, spread);
        let ridge = (reach - 0.01, 0.66 * spread);
        let valley = (reach - notch, 0.33 * spread);
        // Through the twist the edge of the paper is a hyperbola: narrowest
        // at the twist, opening into the fan on one side and, on the other,
        // widening until it is well inside the block, so that the two
        // overlap. The fan's edges are drawn in a little, like the bell of a
        // trumpet.
        let length = corner.0 - twist;
        let flare = (spread * spread - waist * waist).sqrt() / length;
        let upper = |i: usize| match i {
            // Closest together through the twist, where the curve is tightest.
            0..=10 => {
                let t: f32 = [-0.48, -0.18, -0.09, -0.04, 0.0, 0.04, 0.08, 0.14, 0.24, 0.42, 0.7][i];
                let bow = 1.0 - bell * t.max(0.0) * (1.0 - t);
                (twist + t * length, waist.hypot(flare * t * length) * bow)
            }
            11 => corner,
            12 => ridge,
            13 => valley,
            _ => (reach, 0.0),
        };
        let outline = std::array::from_fn(|i| {
            let (dx, up) = if i < 15 { upper(i) } else { upper(28 - i) };
            (k + dir * dx, if i < 15 { 0.5 - up } else { 0.5 + up })
        });
        let slope = |(dx, up): (f32, f32)| up / (dx - twist);
        Self {
            k,
            dir,
            twist,
            outline,
            folds: [slope(valley), slope(ridge), slope(corner)],
        }
    }

    /// A point as (distance out from the end of the block, depth below the
    /// middle of the logo).
    fn local(&self, x: f32, y: f32) -> (f32, f32) {
        ((x - self.k) * self.dir, y - 0.5)
    }

    fn sdf(&self, x: f32, y: f32) -> f32 {
        polygon(x, y, &self.outline) - 0.018
    }

    /// Where a point of the fan lies across its pleats: 0 on the middle
    /// ridge, 1 in the valleys either side of it, 2 on the next ridges and 3
    /// at the edges of the fan. Negative above the middle.
    fn pleat(&self, dx: f32, dy: f32) -> f32 {
        let slope = dy / (dx - self.twist).max(1e-3);
        let [valley, ridge, edge] = self.folds;
        let s = slope.abs();
        let u = if s < valley {
            s / valley
        } else if s < ridge {
            1.0 + (s - valley) / (ridge - valley)
        } else {
            2.0 + (s - ridge) / (edge - ridge)
        };
        u.copysign(slope)
    }

    /// The colour of the paper at a point of the end, drawn at `scale`
    /// pixels to the unit.
    fn paper(&self, dx: f32, dy: f32, scale: f32) -> [f32; 3] {
        let out = dx - self.twist;
        let u = self.pleat(dx, dy);
        let v = u.abs();
        // 1 on a ridge, 0 in a valley or at the edge of the paper.
        let ridge = if v < 2.0 { (v - 1.0).abs() } else { (3.0 - v).max(0.0) };
        // The light is overhead, so the face that climbs to a ridge from the
        // valley above it is lit, and the face that falls away beneath the
        // ridge is in shade.
        let faces_up = (u > 0.0) == (1.0..2.0).contains(&v);
        // The outermost faces end at the edge of the paper, not in a fold.
        let to_fold = if v < 2.0 { ridge.min(1.0 - ridge) } else { v - 2.0 };
        // The pleats close up toward the twist, and everywhere in a small
        // icon. They fade out before they are too narrow to draw.
        let pleat_px = 0.4 * out.max(0.0) * scale;
        let pleats = smoothstep(1.2, 3.0, pleat_px);
        let contrast = (to_fold * pleat_px).min(0.5) * pleats;
        // Without its pleats the paper is drawn a shade deeper than the top
        // of the block, to stay distinct from it and from a pale background.
        let lit = 0.4 + 0.1 * pleats + if faces_up { contrast } else { -contrast };

        let mut paper = mix(WRAP_SHADE, WRAP, lit);
        paper = mix(paper, SHEEN, 0.5 * pleats * lit * ridge * ridge);
        if v < 2.0 {
            paper = mix(paper, WRAP_DEEP, 0.4 * pleats * (1.0 - ridge).powi(2));
        }
        // Gathered into the twist, the paper is all crease.
        paper = mix(paper, WRAP_DEEP, 0.8 * smoothstep(0.22, 0.0, out));
        // From the twist back to the block it is stretched like a tent, its
        // upper slope to the light.
        let tent = mix(WRAP_SHADE, WRAP_DEEP, smoothstep(-0.08, 0.08, dy));
        mix(paper, tent, smoothstep(0.0, -0.05, out))
    }

    /// The gathers between the twist and the block, each a small fold: how
    /// far a point is in the shade beneath one. They are shadow only; lit
    /// along their tops as well, they read as a sparkle.
    fn gathers(&self, dx: f32, dy: f32, scale: f32) -> f32 {
        let back = self.twist - dx;
        let mut shade = 0.0f32;
        for (slope, len) in GATHERS {
            let steep = slope.hypot(1.0);
            let below = (dy - slope * back) / steep;
            // Each tapers to nothing at its tip.
            let taper = (1.0 - back * steep / len).clamp(0.0, 1.0);
            let width = 0.03 * (0.3 + 0.7 * taper);
            let under = (0.5 + below * scale).clamp(0.0, 1.0);
            shade = shade.max(taper * under * smoothstep(width, 0.0, below));
        }
        // They rise out of the twist rather than crossing it, and are left
        // out altogether once they would be only a pixel or two wide.
        shade * smoothstep(0.0, 0.03, back) * smoothstep(1.4, 3.0, 0.03 * scale)
    }
}

/// Rasterise the logo into a `w x h` straight-alpha RGBA image, as large as
/// fits, centred, and rotated by `angle` radians.
pub fn rasterize(logo: Logo, w: u32, h: u32, angle: f32) -> Vec<u8> {
    let mut l = layout(logo);
    let (sin, cos) = angle.sin_cos();
    // Pixels per logo unit, so that the rotated bounding box fits the canvas.
    let fit = |width: f32, height: f32| {
        (w as f32 / (width * cos.abs() + height * sin.abs())).min(h as f32 / (width * sin.abs() + height * cos.abs()))
    };
    // A small icon has too few pixels for the wrapper ends as designed. They
    // are drawn shorter, and the room that frees goes to the block and its
    // letter.
    let tiny = 1.0 - smoothstep(10.0, 28.0, fit(l.width, 1.0));
    let scale = fit(l.width - 2.0 * TINY_TRIM * tiny, 1.0 - 0.1 * tiny);
    // Upright, the baseline and cap line of each line of print are moved on
    // to pixel boundaries (by half a pixel at most), so that small lettering
    // has crisp tops and feet beside the interface's hinted text. Fitting the
    // baseline first keeps lines that share one level. A tilted logo has no
    // rows to fit to.
    if angle == 0.0 {
        let mid = h as f32 * 0.5;
        for run in &mut l.runs {
            let width = run.width();
            let base = ((run.top + run.cap - 0.5) * scale + mid).round();
            let cap = (run.cap * scale).round().max(1.0);
            run.top = (base - cap - mid) / scale + 0.5;
            run.cap = cap / scale;
            // The letters scale with their height: keep the line centred.
            run.x += (width - run.width()) * 0.5;
        }
    }
    let ends = [
        WrapEnd::new(l.body_x0, -1.0, tiny, scale),
        WrapEnd::new(l.body_x1, 1.0, tiny, scale),
    ];

    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for py in 0..h {
        for px in 0..w {
            // Canvas pixel -> logo units.
            let cx = (px as f32 + 0.5 - w as f32 * 0.5) / scale;
            let cy = (py as f32 + 0.5 - h as f32 * 0.5) / scale;
            let x = cx * cos + cy * sin + l.width * 0.5;
            let y = -cx * sin + cy * cos + 0.5;

            let d_body = round_rect(
                x,
                y,
                l.body_x0,
                BODY_Y0,
                l.body_x1 - l.body_x0,
                BODY_Y1 - BODY_Y0,
                BODY_RADIUS,
            );
            let end = if x < l.width * 0.5 { &ends[0] } else { &ends[1] };
            let (dx, dy) = end.local(x, y);
            // Deep inside the block the wrapper end is too far off to matter.
            let d_end = if dx > -0.2 { end.sdf(x, y) } else { f32::MAX };
            // A smooth union: the paper is stretched taut from the end of
            // the block to the twist, with no corner where it leaves.
            let web = (1.0 - (d_body - d_end).abs() / 0.09).max(0.0);
            let alpha = (0.5 - (d_body.min(d_end) - 0.0225 * web * web) * scale).clamp(0.0, 1.0);
            if alpha <= 0.0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }

            let on_body = (0.5 - d_body * scale).clamp(0.0, 1.0);
            let mut color = if on_body < 1.0 {
                end.paper(dx, dy, scale)
            } else {
                [0.0; 3]
            };
            if on_body > 0.0 {
                // The butter: a soft block lit from overhead, with a glint
                // along its top shoulder and its edges rounded off, the
                // underside most.
                let t = (y - BODY_Y0) / (BODY_Y1 - BODY_Y0);
                let mut body = mix(BODY_TOP, BODY_BOTTOM, smoothstep(0.0, 1.0, t));
                let glint = round_rect(
                    x,
                    y,
                    l.body_x0 + 0.11,
                    BODY_Y0 + 0.04,
                    l.body_x1 - l.body_x0 - 0.22,
                    0.03,
                    0.015,
                );
                body = mix(body, SHEEN, 0.55 * smoothstep(0.04, -0.015, glint));
                body = mix(
                    body,
                    BODY_EDGE,
                    (0.25 + 0.55 * t) * smoothstep(-0.05 - 0.04 * t, 0.0, d_body),
                );
                // A shadow where the paper is pinched off the end of the block.
                body = mix(body, BODY_EDGE, 0.5 * smoothstep(0.2, 0.0, dx.hypot(dy)));
                color = mix(color, body, on_body);
            }
            // The gathers cross from the stretched paper on to the block: it
            // is one sheet.
            if dx > -0.2 && dx < end.twist {
                color = mix(color, WRAP_DEEP, 0.5 * end.gathers(dx, dy, scale));
            }

            // The print: one ink, on the body only.
            let d_ink = l.runs.iter().map(|r| r.sdf(x, y)).fold(f32::MAX, f32::min);
            let ink = (0.5 - d_ink * scale).clamp(0.0, 1.0) * on_body;
            color = mix(color, INK, ink);

            out.extend_from_slice(&[
                (color[0] * 255.0 + 0.5) as u8,
                (color[1] * 255.0 + 0.5) as u8,
                (color[2] * 255.0 + 0.5) as u8,
                (alpha * 255.0 + 0.5) as u8,
            ]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_ink(px: &&[u8; 4]) -> bool {
        px[3] > 200 && px[2] as i32 > px[0] as i32 + 40
    }

    #[test]
    fn every_variant_has_butter_ink_and_clear_corners() {
        for logo in [Logo::Full, Logo::Compact, Logo::Mark] {
            let h = 96u32;
            let w = (h as f32 * aspect(logo)).round() as u32;
            let img = rasterize(logo, w, h, 0.0);
            assert_eq!(img.len(), (w * h * 4) as usize);
            let px = |x: u32, y: u32| &img[((y * w + x) * 4) as usize..][..4];
            // The middle of the left edge of the body is butter, not ink.
            let body = px((END * h as f32) as u32 + 6, h / 2);
            assert!(
                body[3] == 255 && body[0] > 200 && body[1] > 170 && body[2] < 140,
                "{logo:?}: {body:?}"
            );
            let ink = img.as_chunks::<4>().0.iter().filter(is_ink).count();
            assert!(ink > 300, "{logo:?} has almost no lettering ({ink} px)");
            // The silhouette does not touch the corners of its box.
            for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
                assert_eq!(px(x, y)[3], 0, "{logo:?} corner {x},{y}");
            }
            // Nor do the points of the wrapper ends run off its sides.
            for y in 0..h {
                assert!(
                    px(0, y)[3] == 0 && px(w - 1, y)[3] == 0,
                    "{logo:?} is cut off at row {y}"
                );
            }
        }
    }

    /// The logo `h` pixels tall, and for each row the first and last column
    /// holding ink (rows without ink are `None`).
    fn ink_rows(logo: Logo, h: u32) -> (u32, Vec<Option<(u32, u32)>>) {
        let w = (h as f32 * aspect(logo)).round() as u32;
        let img = rasterize(logo, w, h, 0.0);
        let rows = img
            .as_chunks::<4>()
            .0
            .chunks_exact(w as usize)
            .map(|row| {
                let first = row.iter().position(|p| is_ink(&p))?;
                let last = row.iter().rposition(|p| is_ink(&p))?;
                Some((first as u32, last as u32))
            })
            .collect();
        (w, rows)
    }

    /// The lines of print, top to bottom: (first row, last row, first
    /// column, last column) of each unbroken run of inked rows.
    fn print_lines(rows: &[Option<(u32, u32)>]) -> Vec<(u32, u32, u32, u32)> {
        let mut out: Vec<(u32, u32, u32, u32)> = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            let (y, Some((x0, x1))) = (y as u32, *row) else {
                continue;
            };
            match out.last_mut() {
                Some(line) if line.1 + 1 == y => *line = (line.0, y, line.2.min(x0), line.3.max(x1)),
                _ => out.push((y, y, x0, x1)),
            }
        }
        out
    }

    #[test]
    fn the_full_label_sets_candy_under_butter() {
        let h = 240;
        let (_, rows) = ink_rows(Logo::Full, h);
        let lines = print_lines(&rows);
        // Two lines of print with clear butter between them: the name, then
        // a line a third of its height.
        let [butter, second] = lines[..] else {
            panic!("expected two lines of print, found {lines:?}")
        };
        let height = |l: (u32, u32, u32, u32)| l.1 - l.0 + 1;
        assert!(second.0 > butter.1 + 4, "{lines:?}");
        assert!(
            height(butter) > 2 * height(second) && height(butter) < 4 * height(second),
            "{lines:?}"
        );

        // The top half of the second line is CANDY alone, centred under the
        // name: the fine print beside it is not a third as tall.
        let top_half = &rows[second.0 as usize..((second.0 + second.1) / 2) as usize];
        let first = top_half.iter().flatten().map(|r| r.0).min().unwrap();
        let last = top_half.iter().flatten().map(|r| r.1).max().unwrap();
        assert!(
            (first + last).abs_diff(butter.2 + butter.3) <= 6,
            "CANDY spans {first}..{last}: {lines:?}"
        );
        // The fine print stands on CANDY's baseline, clear of it, and ends
        // flush with the name.
        let (_, end) = rows[second.1 as usize].expect("ink on the baseline row");
        assert!(
            end > last + height(second) && end.abs_diff(butter.3) <= 3,
            "fine print ends at {end}: {lines:?}"
        );
        assert!(second.2 == first, "nothing is printed to the left of CANDY: {lines:?}");

        // The other two carry a single line.
        for logo in [Logo::Compact, Logo::Mark] {
            assert_eq!(print_lines(&ink_rows(logo, h).1).len(), 1, "{logo:?}");
        }
    }

    #[test]
    fn the_stick_is_fitted_to_the_name() {
        let h = 240;
        for logo in [Logo::Full, Logo::Compact, Logo::Mark] {
            let (w, rows) = ink_rows(logo, h);
            let x0 = rows.iter().flatten().map(|r| r.0).min().unwrap();
            let x1 = rows.iter().flatten().map(|r| r.1).max().unwrap();
            // The print is centred: the same margin either side, with nothing
            // set beside the name to push it off.
            let (left, right) = (x0 as i32, (w - 1 - x1) as i32);
            assert!((left - right).abs() <= 4, "{logo:?}: margins {left} and {right}");
            // And the name runs most of the length of the stick.
            if logo != Logo::Mark {
                let body = w as f32 - 2.0 * END * h as f32;
                let name = (x1 - x0 + 1) as f32;
                assert!(name > 0.78 * body, "{logo:?}: name {name} px on a {body} px stick");
            }
        }
    }

    #[test]
    fn bowl_distance_is_signed_and_true_near_the_curve() {
        let (x1, h, rx) = (0.6_f32, 0.5_f32, 0.3_f32);
        let d = |x: f32, y: f32| bowl(x, y, 0.0, 0.0, x1, h, (rx, h * 0.5));
        assert!(d(0.2, 0.25) < -0.1 && d(0.5, 0.25) < -0.05);
        assert!(d(0.7, 0.25) > 0.05 && d(0.59, 0.01) > 0.0 && d(0.59, 0.49) > 0.0);
        // The straight sides are exact, the side of a lobe among them.
        assert!((d(0.1, -0.03) - 0.03).abs() < 1e-6 && (d(-0.03, 0.2) - 0.03).abs() < 1e-6);
        assert!((bowl(0.63, 0.25, 0.0, 0.0, x1, h, (rx, 0.2)) - 0.03).abs() < 1e-6);
        // Round the curve, a point a little way off the outline measures as
        // far from it as it is from a fine tracing of the outline.
        let outline: Vec<(f32, f32)> = (0..=4000)
            .map(|i| {
                let (sin, cos) = ((i as f32 / 4000.0 - 0.5) * std::f32::consts::PI).sin_cos();
                (
                    x1 - rx + rx * cos.powf(0.8),
                    h * 0.5 * (1.0 + sin.signum() * sin.abs().powf(0.8)),
                )
            })
            .collect();
        for &(ox, oy) in outline.iter().skip(100).step_by(200) {
            for (px, py) in [(ox + 0.01, oy), (ox - 0.01, oy), (ox, oy + 0.01), (ox, oy - 0.01)] {
                let traced = outline
                    .iter()
                    .map(|&(x, y)| (px - x).hypot(py - y))
                    .fold(f32::MAX, f32::min);
                assert!(
                    (d(px, py).abs() - traced).abs() < 0.001,
                    "at {px},{py}: {} vs {traced}",
                    d(px, py)
                );
            }
        }
    }

    #[test]
    fn rotated_marks_stay_inside_a_square_icon() {
        // Every size the window icon is shown at: the small ones are fitted
        // to the canvas differently.
        for size in [16u32, 24, 32, 48, 64] {
            let img = rasterize(Logo::Mark, size, size, -0.5);
            let alpha = |x: u32, y: u32| img[((y * size + x) * 4 + 3) as usize];
            for i in 0..size {
                assert!(
                    alpha(i, 0) == 0 && alpha(i, size - 1) == 0 && alpha(0, i) == 0 && alpha(size - 1, i) == 0,
                    "the {size} px icon is cut off"
                );
            }
            let solid = img.as_chunks::<4>().0.iter().filter(|p| p[3] > 128).count() as u32;
            assert!(
                solid * 5 > size * size,
                "the {size} px icon is nearly empty ({solid} px)"
            );
        }
    }
}
