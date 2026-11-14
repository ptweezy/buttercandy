//! Sugarplum's tree, grown again on the CPU.
//!
//! The Sugarplum shader walks its tree for every pixel, and skips any branch
//! when the pixel lies outside a sphere known to hold everything that grows
//! from it. Those spheres are constants in the shader (`CTR` and `RAD`), and
//! they are only right for the growth rule they were measured from. This
//! grows the same tree, from the constants in the shader's own source, in the
//! poses the wind and the kick can put it in, and checks that the spheres
//! hold it. If they do not, the failure prints the tables that would.
//!
//! The growth rule is repeated here (`grow`): change it and the shader's
//! `walk` together.

use super::presets::{PRESETS, index_of};

type V3 = [f64; 3];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: V3) -> V3 {
    scale(a, 1.0 / dot(a, a).sqrt())
}

/// `const NAME: type = value;` in the shader.
fn scalar(src: &str, name: &str) -> f64 {
    let key = format!("const {name}: ");
    let line = src
        .lines()
        .find(|l| l.starts_with(&key))
        .unwrap_or_else(|| panic!("the shader has no constant {name}"));
    let value = line.split('=').nth(1).unwrap().trim().trim_end_matches(';');
    value.parse().unwrap_or_else(|_| panic!("{name} = {value}?"))
}

/// `const NAME: array<f32, 10> = array<f32, 10>(...);` in the shader.
fn table(src: &str, name: &str) -> Vec<f64> {
    let key = format!("const {name}: array<f32, 10> = array<f32, 10>(");
    let line = src
        .lines()
        .find(|l| l.starts_with(&key))
        .unwrap_or_else(|| panic!("the shader has no table {name}"));
    line[key.len()..]
        .trim_end_matches(");")
        .split(',')
        .map(|v| v.trim().parse().unwrap())
        .collect()
}

/// The shader's `ih`.
fn ih(n: u32, j: u32, k: u32) -> u32 {
    let mut x = j
        .wrapping_mul(747_796_405)
        .wrapping_add(n.wrapping_mul(2_891_336_453))
        .wrapping_add(k.wrapping_mul(1_013_904_223))
        .wrapping_add(12345);
    x = ((x >> ((x >> 28) + 4)) ^ x).wrapping_mul(277_803_737);
    (x >> 22) ^ x
}

/// The shader's `bits`.
fn bits(h: u32, at: u32) -> f64 {
    ((h >> at) & 1023) as f64 / 1024.0
}

struct Rule {
    nlev: u32,
    t0: [f64; 2],
    t: [f64; 2],
    tj: f64,
    up0: f64,
    ups: f64,
    heave: f64,
    sway: f64,
    len: Vec<f64>,
}

/// One branch: its level, where it starts, the chord direction its sphere is
/// placed along, and the range of `points` that it and all above it own.
struct Branch {
    level: usize,
    start: V3,
    axis: V3,
    points: std::ops::Range<usize>,
}

struct Tree {
    points: Vec<V3>,
    branches: Vec<Branch>,
}

/// Grow branch `j` of level `n`, and everything above it, as the shader's
/// `walk` does. `heave` is how far a kick has opened the forks, 0..1, and
/// `wind` gives each fork its push, -1..1.
fn grow(
    rule: &Rule,
    tree: &mut Tree,
    (n, j): (u32, u32),
    (o, dp, d, u): (V3, V3, V3, V3),
    heave: f64,
    wind: &mut dyn FnMut() -> f64,
) {
    let hn = ih(n, j, 0);
    let len = rule.len[n as usize] * if n == 0 { 1.0 } else { 0.84 + 0.32 * bits(hn, 0) };
    let cc = add(o, scale(dp, 0.5 * len));
    let e = add(cc, scale(d, 0.5 * len));
    let first = tree.points.len();
    tree.points.extend([o, cc, e]);
    let at = tree.branches.len();
    tree.branches.push(Branch {
        level: n as usize,
        start: o,
        axis: scale(add(dp, d), 0.5),
        points: first..first,
    });
    if n < rule.nlev {
        let side = if n == 0 {
            [1.0, 0.0]
        } else {
            let hp = bits(hn, 10);
            let (x, y) = (0.267 + (-0.971 - 0.267) * hp, 0.964 + (0.239 - 0.964) * hp);
            let l = x.hypot(y);
            [x / l, y / l]
        };
        let u1 = add(scale(u, side[0]), scale(cross(d, u), side[1]));
        for k in 0..2u32 {
            let hc = ih(n + 1, j * 2 + k, 1);
            let lean = if n == 0 {
                rule.t0[k as usize] + 0.12 * bits(hc, 0)
            } else {
                rule.t[k as usize] + rule.tj * bits(hc, 0)
            };
            let amp = ((hc >> 13) & 255) as f64 / 128.0 - 1.0;
            let lean = lean * (1.0 + rule.heave * heave) + rule.sway * (0.4 + 0.12 * n as f64) * amp * wind();
            let mut dk = add(d, scale(u1, (k as f64 * 2.0 - 1.0) * lean));
            dk[1] += rule.up0 - rule.ups * n as f64;
            let dk = unit(dk);
            let uk = unit(add(u1, scale(dk, -dot(u1, dk))));
            grow(rule, tree, (n + 1, j * 2 + k), (e, d, dk, uk), heave, wind);
        }
    }
    tree.branches[at].points.end = tree.points.len();
}

fn row(values: &[f64]) -> String {
    let cells: Vec<String> = values.iter().map(|v| format!("{v:.4}")).collect();
    format!("array<f32, 10> = array<f32, 10>({});", cells.join(", "))
}

#[test]
fn sugarplum_bounds_hold() {
    let src = PRESETS[index_of("sugarplum")].source;
    // The tree grown here is the shader's only while they share a hash.
    assert!(src.contains("var x = j * 747796405u + n * 2891336453u + k * 1013904223u + 12345u;"));
    assert!(src.contains("x = ((x >> ((x >> 28u) + 4u)) ^ x) * 277803737u;"));
    assert!(src.contains("return (x >> 22u) ^ x;"));
    let rule = Rule {
        nlev: scalar(src, "NLEV") as u32,
        t0: [scalar(src, "T0A"), scalar(src, "T0B")],
        t: [scalar(src, "TA"), scalar(src, "TB")],
        tj: scalar(src, "TJ"),
        up0: scalar(src, "UP0"),
        ups: scalar(src, "UPS"),
        heave: scalar(src, "HEAVE"),
        sway: scalar(src, "SWAY"),
        len: table(src, "LEN"),
    };
    let (ctr, rad) = (table(src, "CTR"), table(src, "RAD"));
    let levels = rule.nlev as usize + 1;
    assert!(levels <= 10, "the tables hold ten levels");
    let total: f64 = rule.len[..levels].iter().sum();
    assert!((total - scalar(src, "STOT")).abs() < 1e-3, "STOT should be {total:.4}");

    // Centres to try, as multiples of a level's branch length, should the
    // tables need making again.
    let tries: Vec<f64> = (0..=80).map(|i| i as f64 * 0.05).collect();
    // The radius each level needs: with its centre where the shader has it,
    // and with it at each of the centres to try.
    let mut needed = vec![0.0f64; levels];
    let mut needed_at = vec![vec![0.0f64; tries.len()]; levels];

    let up = [0.0, 1.0, 0.0];
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    // The tree at rest, then with the wind in it, the kick's push on and off.
    for pose in 0..24 {
        let mut wind = || {
            if pose == 0 {
                return 0.0;
            }
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 52) as f64 - 1.0
        };
        let mut tree = Tree {
            points: Vec::new(),
            branches: Vec::new(),
        };
        let heave = (pose % 2) as f64;
        grow(
            &rule,
            &mut tree,
            (0, 0),
            ([0.0; 3], up, up, [1.0, 0.0, 0.0]),
            heave,
            &mut wind,
        );
        assert_eq!(tree.branches.len(), (1 << levels) - 1);
        for b in &tree.branches {
            let len = rule.len[b.level];
            for &pt in &tree.points[b.points.clone()] {
                let from = |along: f64| {
                    let c = add(b.start, scale(b.axis, along));
                    let r = add(pt, scale(c, -1.0));
                    dot(r, r).sqrt()
                };
                needed[b.level] = needed[b.level].max(from(ctr[b.level]));
                for (i, t) in tries.iter().enumerate() {
                    needed_at[b.level][i] = needed_at[b.level][i].max(from(t * len));
                }
            }
        }
    }

    // The tables as they should be: the best centre for each level, and its
    // radius with 6% to spare for poses not tried here.
    let (mut best_ctr, mut best_rad) = (vec![0.0; 10], vec![0.0; 10]);
    for n in 0..levels {
        let (i, r) = needed_at[n]
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .unwrap();
        best_ctr[n] = tries[i] * rule.len[n];
        best_rad[n] = r * 1.06;
    }
    let fix = format!(
        "in sugarplum.wgsl these should read\nconst CTR: {}\nconst RAD: {}",
        row(&best_ctr),
        row(&best_rad)
    );
    for n in 0..levels {
        assert!(
            needed[n] <= rad[n],
            "level {n} needs a sphere of radius {:.4}, not {:.4}: branches would be cut off; {fix}",
            needed[n],
            rad[n]
        );
        // Too big is no error in the picture, but every pixel pays for it.
        assert!(
            needed[n] >= 0.85 * rad[n],
            "level {n} needs a sphere of radius only {:.4}, not {:.4}; {fix}",
            needed[n],
            rad[n]
        );
    }
}
