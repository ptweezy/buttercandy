//! Jawbreaker's fall into the Mandelbrot set, worked out in 64-bit floats.
//!
//! The shader has 32-bit floats, which run out about five thousand times
//! deep. So it does not iterate each pixel's own point c. It iterates the
//! *difference* between the pixel's orbit and one orbit worked out here in
//! 64 bits, the reference orbit, and a difference stays accurate however
//! small it is (perturbation). This driver supplies:
//!
//! - the place to fall to: the centre (nucleus) of a miniature copy of the
//!   set, found by `find_leg`, small enough to be a hundred thousand million
//!   times deep or more;
//! - that nucleus's orbit, which is periodic, as the reference;
//! - where to start: the first few thousand iterations move every pixel of
//!   the view almost alike, so the view's orbits are a cubic polynomial in
//!   the pixel's offset there (`Series`), and the shader starts from that
//!   instead of from zero;
//! - the view itself, falling at a pace set by the music.
//!
//! A miniature copy is, to a very good approximation, the whole set scaled
//! and turned by one complex number (`size`). So when the copy is big enough
//! to see, the picture dissolves from the copy to the whole set drawn in its
//! place, and the fall goes on from the top towards the next copy: one fall
//! without end, a new path each time.
//!
//! None of these methods is new: they are how deep-zoom renderers of the set
//! work (perturbation, series approximation, starting the reference again
//! when a pixel comes nearer to 0 than to it), and how nuclei are hunted (the
//! period of a box of the plane, then Newton's method).
//!
//! The shader's loop is repeated here (`shader_escape`) to test it against
//! plain 64-bit iteration: change it and `iterate` in jawbreaker.wgsl
//! together.
//!
//! To look at a place far down without waiting to fall there, set the
//! environment variable `BUTTERCANDY_DIVE` to a number of e-folds (20 is
//! about the length of a leg) and the fall starts that deep.

use std::f64::consts::TAU;
use std::ops::{Add, Div, Mul, Sub};
use std::sync::mpsc::{Receiver, channel};

use super::driver::{DATA_LEN, Driver, PARAMS};
use crate::audio::analysis::Analysis;

/// A complex number.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Z {
    pub re: f64,
    pub im: f64,
}

impl Z {
    const ZERO: Z = Z { re: 0.0, im: 0.0 };
    const ONE: Z = Z { re: 1.0, im: 0.0 };

    const fn new(re: f64, im: f64) -> Z {
        Z { re, im }
    }

    fn polar(r: f64, th: f64) -> Z {
        Z::new(r * th.cos(), r * th.sin())
    }

    /// |z| squared.
    fn norm(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    fn abs(self) -> f64 {
        self.norm().sqrt()
    }

    fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }

    fn scale(self, k: f64) -> Z {
        Z::new(self.re * k, self.im * k)
    }

    fn finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }

    fn f32s(self) -> [f32; 2] {
        [self.re as f32, self.im as f32]
    }
}

impl Add for Z {
    type Output = Z;
    fn add(self, o: Z) -> Z {
        Z::new(self.re + o.re, self.im + o.im)
    }
}

impl Sub for Z {
    type Output = Z;
    fn sub(self, o: Z) -> Z {
        Z::new(self.re - o.re, self.im - o.im)
    }
}

impl Mul for Z {
    type Output = Z;
    fn mul(self, o: Z) -> Z {
        Z::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
}

impl Div for Z {
    type Output = Z;
    fn div(self, o: Z) -> Z {
        let n = o.norm();
        Z::new(
            (self.re * o.re + self.im * o.im) / n,
            (self.im * o.re - self.re * o.im) / n,
        )
    }
}

/// How many iterations c takes to leave, or `None` if it has not by `cap`.
fn escape_time(c: Z, cap: u32) -> Option<u32> {
    let mut z = Z::ZERO;
    for n in 0..cap {
        z = z * z + c;
        if z.norm() > 256.0 {
            return Some(n);
        }
    }
    None
}

/// The lowest period of any nucleus in the square of half-width `half` round
/// `centre`, if one shows itself within `cap` iterations. The four corners
/// are iterated together; the first time their images surround the origin,
/// some point of the square has come back to it.
fn box_period(centre: Z, half: f64, cap: u32) -> Option<u32> {
    let corners =
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(x, y)| centre + Z::new(x * half, y * half));
    let mut z = [Z::ZERO; 4];
    for period in 1..=cap {
        for i in 0..4 {
            z[i] = z[i] * z[i] + corners[i];
            if z[i].norm() > 1e20 {
                return None;
            }
        }
        // Does a ray from the origin along +x cross the outline an odd number
        // of times?
        let mut inside = false;
        for i in 0..4 {
            let (a, b) = (z[i], z[(i + 1) % 4]);
            if (a.im > 0.0) != (b.im > 0.0) && a.re + (0.0 - a.im) * (b.re - a.re) / (b.im - a.im) > 0.0 {
                inside = !inside;
            }
        }
        if inside {
            return Some(period);
        }
    }
    None
}

/// The nucleus of the given period nearest `guess`, by Newton's method on
/// "0 comes back to 0 after `period` steps".
fn nucleus(guess: Z, period: u32) -> Option<Z> {
    let mut c = guess;
    for _ in 0..64 {
        let (mut z, mut dz) = (Z::ZERO, Z::ZERO);
        for _ in 0..period {
            dz = (z * dz).scale(2.0) + Z::ONE;
            z = z * z + c;
        }
        let step = z / dz;
        if !step.finite() {
            return None;
        }
        c = c - step;
        // As near as 64 bits can put it.
        if step.norm() <= 1e-30 * c.norm().max(1e-12) {
            return Some(c);
        }
    }
    None
}

/// One period of a nucleus's orbit, from 0.
fn orbit_of(nucleus: Z, period: u32) -> Vec<Z> {
    let mut orbit = Vec::with_capacity(period as usize);
    let mut z = Z::ZERO;
    for _ in 0..period {
        orbit.push(z);
        z = z * z + nucleus;
    }
    orbit
}

/// The size and turn of the miniature copy of the set round a nucleus, as
/// one complex number: the copy is roughly `nucleus + size * (the whole
/// set)`. `orbit` is one period of the nucleus's orbit.
///
/// A copy is there because, near the nucleus, a whole period of steps acts
/// as a single step of the same kind, only smaller. How much smaller comes
/// from the orbit: with `stretch` the product of 2z over the orbit after its
/// start, and `sum` one plus the reciprocals of that product as it builds
/// up, the factor is 1 / (sum * stretch^2).
fn size(orbit: &[Z]) -> Z {
    let mut stretch = Z::ONE;
    let mut sum = Z::ONE;
    for z in &orbit[1..] {
        stretch = stretch * z.scale(2.0);
        sum = sum + Z::ONE / stretch;
    }
    Z::ONE / (sum * stretch * stretch)
}

/// The Taylor series, in a point's offset dc from the nucleus, of how far
/// its orbit is from the nucleus's after n steps:
/// a[n] dc + b[n] dc^2 + c[n] dc^3.
struct Series {
    a: Vec<Z>,
    b: Vec<Z>,
    c: Vec<Z>,
}

/// Steps the series is worked out for. Past this the shader iterates.
const SERIES: usize = 1 << 16;
/// The cubic term may be this fraction of the linear one at the edge of the
/// view before the series is no longer trusted.
const TRUST: f64 = 1e-4;

impl Series {
    fn new(orbit: &[Z]) -> Series {
        let mut s = Series {
            a: vec![Z::ZERO],
            b: vec![Z::ZERO],
            c: vec![Z::ZERO],
        };
        let (mut a, mut b, mut c) = (Z::ZERO, Z::ZERO, Z::ZERO);
        for n in 0..SERIES {
            let z2 = orbit[n % orbit.len()].scale(2.0);
            (a, b, c) = (z2 * a + Z::ONE, z2 * b + a * a, z2 * c + (a * b).scale(2.0));
            if !(a.finite() && b.finite() && c.finite()) {
                break;
            }
            s.a.push(a);
            s.b.push(b);
            s.c.push(c);
        }
        s
    }

    /// Can the first n steps be skipped, for offsets up to `reach` screen
    /// units at `scale` plane units to the screen unit?
    fn trusted(&self, n: usize, scale: f64, reach: f64) -> bool {
        let r = scale * reach;
        n < self.a.len() && self.c[n].abs() * r * r <= TRUST * self.a[n].abs()
    }

    /// The most steps that can be skipped, given that `from` could be a
    /// moment ago.
    fn skip(&self, from: usize, scale: f64, reach: f64) -> usize {
        let mut n = from.min(self.a.len() - 1);
        while n > 0 && !self.trusted(n, scale, reach) {
            n -= 1;
        }
        while self.trusted(n + 1, scale, reach) {
            n += 1;
        }
        n
    }
}

/// One fall: from wherever the view is to a miniature copy of the set.
struct Leg {
    nucleus: Z,
    size: Z,
    /// The nucleus's orbit, one period of it, from 0.
    orbit: Vec<Z>,
    series: Series,
}

/// Entries of the table a leg's orbit may take: half of it, so that two legs
/// fit while one dissolves into the next.
const ORBIT: usize = DATA_LEN / 8;

impl Leg {
    fn new(nucleus: Z, period: u32) -> Leg {
        let orbit = orbit_of(nucleus, period);
        Leg {
            nucleus,
            size: size(&orbit),
            series: Series::new(&orbit),
            orbit,
        }
    }
}

/// A small generator of random numbers: the path has to repeat exactly in a
/// headless run.
struct Rng(u64);

impl Rng {
    /// In 0..1.
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A copy has to be at least this small to be worth the fall, and no smaller
/// than this for 64 bits to place it.
const BIGGEST: f64 = 1e-9;
const SMALLEST: f64 = 1e-13;

/// The first leg of every run: a copy found by `find_leg`, kept here so that
/// the preset opens at once and opens well.
const FIRST: (Z, u32) = (Z::new(-0.744_535_257_930_44, -0.133_230_561_064_629_9), 114);

/// Of `tries` points drawn by `draw`, one of those that take longest to
/// leave, and so lie nearest the boundary of the set: the very slowest for a
/// `rank` of 0, the median for 0.5.
fn slow(tries: usize, cap: u32, rank: f64, mut draw: impl FnMut() -> Z) -> Option<Z> {
    let mut left: Vec<(u32, Z)> = (0..tries)
        .filter_map(|_| {
            let p = draw();
            escape_time(p, cap).map(|n| (n, p))
        })
        .collect();
    left.sort_by_key(|a| std::cmp::Reverse(a.0));
    left.get((rank * left.len() as f64) as usize).map(|&(_, p)| p)
}

/// How far c is from the set, roughly, or `None` if it has not left by `cap`.
fn distance(c: Z, cap: usize) -> Option<f64> {
    let (mut z, mut dz) = (Z::ZERO, Z::ZERO);
    for _ in 0..cap {
        if z.norm() > 1e6 {
            return Some(z.abs() * z.abs().ln() / dz.abs());
        }
        dz = (z * dz).scale(2.0) + Z::ONE;
        z = z * z + c;
    }
    None
}

/// Look for a miniature copy to fall to: first somewhere on the boundary of
/// the set, then the copy nearest it, then a copy in the filaments round
/// that copy, and so on down, each much smaller than the last, until one is
/// small enough.
fn try_leg(rng: &mut Rng) -> Option<Leg> {
    let mut centre = Z::new(-0.75, 0.0);
    let mut half = 1.3;
    // How near the thick of things to go: the very slowest points are in the
    // densest tangles, the next few in the open ground beside them.
    let rank = 0.2 * rng.next() * rng.next();
    for step in 0..5 {
        centre = slow(40, 400 * (step + 2), rank, || {
            centre + Z::new((rng.next() * 2.0 - 1.0) * half, (rng.next() * 2.0 - 1.0) * half)
        })?;
        half *= 0.2;
    }
    let mut period = box_period(centre, half, ORBIT as u32)?;
    let mut at = nucleus(centre, period)?;
    for _ in 0..8 {
        let r = size(&orbit_of(at, period)).abs();
        if !r.is_finite() || r < SMALLEST {
            return None;
        }
        if r <= BIGGEST {
            return Some(Leg::new(at, period));
        }
        // A point in this copy's filaments, some sizes out from it.
        let near = slow(24, 60 * period + 4000, rank, || {
            at + Z::polar(r * (4.0 + 10.0 * rng.next()), TAU * rng.next())
        })?;
        let reach = (near - at).abs() * 0.25;
        let p = box_period(near, reach, ORBIT as u32)?;
        let n = nucleus(near, p)?;
        if (n - near).abs() > 4.0 * reach || p <= period {
            return None;
        }
        (period, at) = (p, n);
    }
    None
}

/// How much of the fall to a copy is over open ground, 0..1: the share of the
/// picture, at four depths on the way down, that the shader can resolve and
/// that is more than a few pixels from the set. The rest is tangle.
fn openness(leg: &Leg) -> f64 {
    let (top, bottom) = (1.5f64.ln(), (DISSOLVE_FROM * leg.size.abs()).ln());
    let mut open = 0;
    let mut all = 0;
    let mut skip = 0;
    for depth in [0.25, 0.45, 0.65, 0.85] {
        let s = (top + depth * (bottom - top)).exp();
        skip = leg.series.skip(skip, s, REACH);
        for k in 0..48 {
            let u = Z::new(((k % 8) as f64 + 0.5) / 4.0 - 1.0, ((k / 8) as f64 + 0.5) / 3.0 - 1.0);
            let d = distance(leg.nucleus + Z::new(u.re * 1.6, u.im * 0.9).scale(s), skip + STEPS);
            all += 1;
            open += d.is_some_and(|d| d > 0.004 * s) as usize;
        }
    }
    open as f64 / all as f64
}

/// Steps the shader takes after the skipped ones: `MAXIT` in jawbreaker.wgsl.
const STEPS: usize = 640;

/// The share of open ground that looks best: tangle and terraces in about
/// equal parts.
const OPEN: f64 = 0.55;

/// The next copy to fall to: of the first few the search turns up, the one
/// whose fall is nearest to half open ground, half tangle.
fn find_leg(seed: u64) -> Leg {
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let mut best: Option<(f64, Leg)> = None;
    let mut found = 0;
    for _ in 0..800 {
        if let Some(leg) = try_leg(&mut rng) {
            let off = (openness(&leg) - OPEN).abs();
            if best.as_ref().is_none_or(|(least, _)| off < *least) {
                best = Some((off, leg));
            }
            found += 1;
            if found == 10 {
                break;
            }
        }
    }
    best.map_or_else(first_leg, |(_, leg)| leg)
}

fn first_leg() -> Leg {
    Leg::new(FIRST.0, FIRST.1)
}

/// A screen unit (half the picture's height) is this many times the copy's
/// size when the dissolve to the whole set begins, and this many when it is
/// over: the copy is a small jewel at the centre throughout. Nearer than
/// that its pixels need thousands of iterations each.
const DISSOLVE_FROM: f64 = 60.0;
const DISSOLVE_TO: f64 = 16.0;
/// The edge of the picture, in screen units from its centre, with room for a
/// very wide window.
const REACH: f64 = 2.8;

/// A view of the plane: the point at the centre of the picture, the log of
/// how many plane units a screen unit is, and the turn.
#[derive(Clone, Copy)]
struct View {
    centre: Z,
    ln_s: f64,
    theta: f64,
}

/// What the shader does to a screen direction to turn it by `a`:
/// `rot(a) * v` in preset_header.wgsl.
fn turn(a: f64, v: Z) -> Z {
    let (s, c) = a.sin_cos();
    Z::new(c * v.re + s * v.im, c * v.im - s * v.re)
}

pub struct Dive {
    leg: Leg,
    /// Which half of the table the leg's orbit is in.
    half: usize,
    /// The leg after this one, once it has been found.
    next: Option<Leg>,
    search: Option<Receiver<Leg>>,
    seed: u64,
    /// Where this leg's fall began, and where the view is now.
    start: View,
    view: View,
    /// The view of the frame before, in this leg's terms.
    before: View,
    skip: usize,
    /// The analyser's clock when last seen.
    phase: f32,
    table: Vec<f32>,
    dirty: bool,
    /// What the shader has kept is no use for the next frame: it is of
    /// another leg, or there is none yet.
    renew: bool,
    params: [[f32; 4]; PARAMS],
}

impl Dive {
    pub fn new() -> Dive {
        let view = View {
            centre: Z::new(-0.6, 0.0),
            ln_s: 1.5f64.ln(),
            theta: 0.0,
        };
        let mut dive = Dive {
            leg: first_leg(),
            half: 0,
            next: None,
            search: None,
            seed: 0x5eed_cafe_f00d_d00d,
            start: view,
            view,
            before: view,
            skip: 0,
            phase: 0.0,
            table: vec![0.0; DATA_LEN],
            dirty: true,
            renew: true,
            params: [[0.0; 4]; PARAMS],
        };
        dive.write_orbit(0);
        dive.pack(0.0);
        // A testing aid: start this many e-folds into the fall.
        if let Some(deep) = std::env::var("BUTTERCANDY_DIVE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
        {
            dive.fast_forward(deep);
        }
        dive
    }

    /// The seed for the next search.
    fn next_seed(&mut self) -> u64 {
        self.seed = self.seed.wrapping_mul(0x9e37_79b9_7f4a_7c15).wrapping_add(1);
        self.seed
    }

    /// Fall `fall` e-folds further, going on to the next leg when this one
    /// is over.
    fn advance(&mut self, fall: f64) {
        let size = self.leg.size.abs().ln();
        let mut ln_s = self.view.ln_s - fall;
        if self.next.is_none() {
            // Wait at the brink until there is somewhere to go on to.
            ln_s = ln_s.max(DISSOLVE_FROM.ln() + size).min(self.view.ln_s);
        }
        self.view.ln_s = ln_s;
        self.view.centre = self.centre_at(ln_s);

        if ln_s <= DISSOLVE_TO.ln() + size
            && let Some(next) = self.next.take()
        {
            // The copy is the whole set now: go on from the top.
            let (view, before) = (self.handed_on(self.view), self.handed_on(self.before));
            self.leg = next;
            self.half = 1 - self.half;
            (self.start, self.view, self.before) = (view, view, before);
            self.skip = 0;
            self.renew = true;
        }
    }

    /// Fall `e_folds` at once, finding each next leg on the spot.
    fn fast_forward(&mut self, e_folds: f64) {
        let mut left = e_folds;
        while left > 0.0 {
            if self.next.is_none() {
                let seed = self.next_seed();
                self.next = Some(find_leg(seed));
                self.write_orbit(1);
            }
            self.advance(left.min(0.05));
            left -= 0.05;
        }
        self.pack(0.0);
        self.before = self.view;
        self.pack(0.0);
        self.renew = true;
    }

    /// Put the orbit of the leg (0) or of the one after it (1) in its half
    /// of the table.
    fn write_orbit(&mut self, which: usize) {
        let (leg, half) = match which {
            0 => (&self.leg, self.half),
            _ => match &self.next {
                Some(leg) => (leg, 1 - self.half),
                None => return,
            },
        };
        for (i, z) in leg.orbit.iter().enumerate() {
            let at = (half * ORBIT + i) * 4;
            self.table[at..at + 2].copy_from_slice(&z.f32s());
        }
        self.dirty = true;
    }

    /// The centre of the picture when a screen unit is `ln_s`: it slides
    /// from where the leg began to the nucleus, twice as fast as the view
    /// shrinks, so the copy is centred long before it can be seen.
    fn centre_at(&self, ln_s: f64) -> Z {
        let k = (2.0 * (ln_s - self.start.ln_s)).exp().min(1.0);
        self.leg.nucleus + (self.start.centre - self.leg.nucleus).scale(k)
    }

    /// How far the dissolve to the next leg has come, 0..1.
    fn dissolve(&self) -> f64 {
        let size = self.leg.size.abs().ln();
        let t = ((DISSOLVE_FROM.ln() + size - self.view.ln_s) / (DISSOLVE_FROM / DISSOLVE_TO).ln()).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// The same view, seen from the leg after this one: there the copy this
    /// leg falls to is the whole set.
    fn handed_on(&self, v: View) -> View {
        View {
            centre: (v.centre - self.leg.nucleus) / self.leg.size,
            ln_s: v.ln_s - self.leg.size.abs().ln(),
            theta: v.theta + self.leg.size.arg(),
        }
    }

    /// One view's sixteen numbers: where each pixel starts, and from which
    /// step.
    fn pack_view(leg: &Leg, half: usize, v: View, skip: usize) -> [[f32; 4]; 4] {
        let s = v.ln_s.exp();
        let off = (v.centre - leg.nucleus).scale(1.0 / s);
        let (a, b, c) = (
            leg.series.a[skip].scale(s),
            leg.series.b[skip].scale(s * s),
            leg.series.c[skip].scale(s * s * s),
        );
        let period = leg.orbit.len();
        [
            [off.re as f32, off.im as f32, s as f32, v.theta.rem_euclid(TAU) as f32],
            [
                v.ln_s as f32,
                period as f32,
                (skip % period) as f32,
                (half * ORBIT) as f32,
            ],
            [a.re as f32, a.im as f32, b.re as f32, b.im as f32],
            // The steps skipped, for the shader to count on from.
            [c.re as f32, c.im as f32, skip as f32, 0.0],
        ]
    }

    /// Work out `params` for the view as it stands; `push` is the kick's
    /// push into the fall, in logs of scale.
    fn pack(&mut self, push: f64) {
        let mut view = self.view;
        view.ln_s -= push;
        view.centre = self.centre_at(view.ln_s);
        let s = view.ln_s.exp();
        let reach = (view.centre - self.leg.nucleus).abs() / s + REACH;
        self.skip = self.leg.series.skip(self.skip, s, reach);
        let a = Self::pack_view(&self.leg, self.half, view, self.skip);

        // Where this frame's pixels were in the frame before:
        // q * turn(dth) * ps + t.
        let before = self.before;
        let q = (view.ln_s - before.ln_s).exp();
        let t = turn(-before.theta, (view.centre - before.centre).scale((-before.ln_s).exp()));
        let dth = view.theta - before.theta;
        self.before = view;

        let dissolve = if self.next.is_some() { self.dissolve() } else { 0.0 };
        let mut b = [[0.0; 4]; 4];
        if let (Some(next), true) = (&self.next, dissolve > 0.0) {
            let v = self.handed_on(view);
            let s = v.ln_s.exp();
            let reach = (v.centre - next.nucleus).abs() / s + REACH;
            b = Self::pack_view(next, 1 - self.half, v, next.series.skip(0, s, reach));
        }
        self.params = [
            a[0],
            a[1],
            a[2],
            a[3],
            b[0],
            b[1],
            b[2],
            b[3],
            [(q * dth.cos()) as f32, (q * dth.sin()) as f32, t.re as f32, t.im as f32],
            [dissolve as f32, 0.0, 0.0, 0.0],
        ];
    }
}

/// The kick's push, as the shaders' `kickSum` has it.
fn kick_sum(analysis: &Analysis) -> f32 {
    let react = analysis.reactivity.clamp(0.0, 1.0);
    let rise = 0.06 * (1.0 + 2.5 * (1.0 - react));
    let kick = &analysis.onsets[0];
    let sum: f32 = (0..4)
        .map(|k| {
            let age = kick.ages[k];
            let t = (age / rise).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t) * (-(age - rise).max(0.0) / 0.20).exp() * kick.hits[k]
        })
        .sum();
    (sum * (0.35 + 0.65 * react)).min(1.0)
}

impl Driver for Dive {
    fn update(&mut self, analysis: &Analysis, dt: f32) {
        // The next leg is looked for while this one falls.
        if self.next.is_none() {
            match &self.search {
                None => {
                    let (tx, rx) = channel();
                    let seed = self.next_seed();
                    std::thread::spawn(move || {
                        let _ = tx.send(find_leg(seed));
                    });
                    self.search = Some(rx);
                }
                Some(rx) => {
                    if let Ok(leg) = rx.try_recv() {
                        self.next = Some(leg);
                        self.search = None;
                        self.write_orbit(1);
                    }
                }
            }
        }

        // The fall: steady, and faster when the music is louder.
        let react = analysis.reactivity.clamp(0.0, 1.0) as f64;
        let pace = 0.5 + 0.5 * react;
        let dphase = (analysis.phase - self.phase).clamp(0.0, 0.2) as f64;
        self.phase = analysis.phase;
        let fall = (0.09 * dt as f64 + 0.03 * dphase) * pace;
        self.view.theta += 0.012 * dt as f64 * pace;

        self.advance(fall);

        self.pack(0.012 * (0.35 + 0.65 * react) * kick_sum(analysis) as f64);
        self.params[9][1] = std::mem::take(&mut self.renew) as u8 as f32;
    }

    fn params(&self) -> [[f32; 4]; PARAMS] {
        self.params
    }

    fn take_data(&mut self) -> Option<&[f32]> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        Some(&self.table)
    }

    fn resend(&mut self) {
        self.dirty = true;
        self.renew = true;
    }

    fn shuffle(&mut self, seed: u64) {
        self.seed ^= seed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_period_three_copy() {
        let centre = Z::new(-1.76, 0.0);
        assert_eq!(box_period(centre, 0.02, 100), Some(3));
        let n = nucleus(centre, 3).unwrap();
        assert!(
            (n.re + 1.754_877_666_246_693).abs() < 1e-13 && n.im.abs() < 1e-13,
            "{n:?}"
        );
        // It runs from its cusp at -1.75 to its tip at about -1.7903: the
        // whole set runs from 0.25 to -2.
        let s = size(&orbit_of(n, 3));
        assert!((s.re - 0.0179).abs() < 0.002 && s.im.abs() < 1e-9, "{s:?}");
    }

    #[test]
    fn a_copy_is_the_whole_set_scaled_by_its_size() {
        // The whole set has a period-2 disc centred on -1, so a copy of
        // period p has one of period 2p at nucleus - size, near enough.
        for (guess, half) in [(Z::new(-1.76, 0.0), 0.02), (Z::new(-0.1565, 1.0323), 0.01)] {
            let p = box_period(guess, half, 1000).unwrap();
            let n = nucleus(guess, p).unwrap();
            let s = size(&orbit_of(n, p));
            let disc = nucleus(n - s, 2 * p).unwrap();
            assert!(
                (disc - (n - s)).abs() < 0.1 * s.abs(),
                "period {p}: {disc:?} vs {:?}",
                n - s
            );
        }
    }

    /// Plain iteration in 64 bits: the step at which c leaves a circle of
    /// radius 1000, as the shader counts it.
    fn plain_escape(c: Z, cap: usize) -> Option<usize> {
        let mut z = Z::ZERO;
        for n in 0..cap {
            if z.norm() > 1e6 {
                return Some(n);
            }
            z = z * z + c;
        }
        None
    }

    /// The shader's loop, in the shader's 32-bit floats: the step at which
    /// the pixel `u` screen units from the nucleus leaves.
    fn shader_escape(leg: &Leg, v: View, skip: usize, u: [f32; 2], cap: usize) -> Option<usize> {
        fn mul(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
            [a[0] * b[0] - a[1] * b[1], a[0] * b[1] + a[1] * b[0]]
        }
        fn add(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
            [a[0] + b[0], a[1] + b[1]]
        }
        fn dot(a: [f32; 2]) -> f32 {
            a[0] * a[0] + a[1] * a[1]
        }
        let p = Dive::pack_view(leg, 0, v, skip);
        let s = p[0][2];
        let (a, b, c) = ([p[2][0], p[2][1]], [p[2][2], p[2][3]], [p[3][0], p[3][1]]);
        let period = leg.orbit.len();
        let orbit = |m: usize| leg.orbit[m].f32s();
        let mut dz = mul(u, add(a, mul(u, add(b, mul(u, c)))));
        let dc = [s * u[0], s * u[1]];
        let mut m = skip % period;
        let mut zm = orbit(m);
        let mut full = add(zm, dz);
        for i in 0..cap {
            if dot(full) > 1e6 {
                return Some(skip + i);
            }
            dz = add(mul(dz, add([2.0 * zm[0], 2.0 * zm[1]], dz)), dc);
            m += 1;
            if m >= period {
                m = 0;
            }
            zm = orbit(m);
            full = add(zm, dz);
            if dot(full) < dot(dz) {
                dz = full;
                m = 0;
                zm = [0.0, 0.0];
            }
        }
        None
    }

    #[test]
    fn the_shaders_loop_agrees_with_plain_iteration() {
        let leg = find_leg(7);
        let size = leg.size.abs();
        assert!((SMALLEST..=BIGGEST).contains(&size), "size {size:e}");
        // From the whole set down to where the dissolve is over.
        let mut ln_s = 0.0;
        let mut skip = 0;
        let (mut same, mut all) = (0, 0);
        while ln_s > (DISSOLVE_TO * size).ln() {
            let v = View {
                centre: leg.nucleus,
                ln_s,
                theta: 0.0,
            };
            let s = ln_s.exp();
            skip = leg.series.skip(skip, s, REACH);
            let grid = 24;
            // The soonest any point of the view leaves.
            let mut soonest = usize::MAX;
            let mut found = Vec::new();
            for k in 0..grid * grid {
                let (i, j) = (k % grid, k / grid);
                let u = Z::new(
                    ((i as f64 + 0.5) / grid as f64 * 2.0 - 1.0) * 1.78,
                    (j as f64 + 0.5) / grid as f64 * 2.0 - 1.0,
                );
                if let Some(n) = plain_escape(leg.nucleus + u.scale(s), skip + 4000) {
                    // A point that leaves before the skipped steps are over
                    // would mean the series was trusted too far.
                    assert!(
                        n > skip,
                        "at scale {s:e} a point leaves at {n}, before the {skip} skipped"
                    );
                    soonest = soonest.min(n);
                    found.push((u, n));
                }
            }
            // Past a few dozen steps of the boundary's chaos, 32 bits and 64
            // part company point by point (the picture as a whole does not:
            // the 32-bit one is the 64-bit one moved by far less than a
            // pixel). So compare the points that leave soonest.
            for (u, n) in found {
                if n <= soonest + 25 {
                    all += 1;
                    same += (shader_escape(&leg, v, skip, u.f32s(), 4000) == Some(n)) as usize;
                }
            }
            ln_s -= 0.7;
        }
        assert!(all > 300, "only {all} points to compare");
        assert!(same * 100 >= all * 97, "{same} of {all} agree");
    }

    #[test]
    fn the_search_repeats_and_varies() {
        let (a, b, c) = (find_leg(11), find_leg(11), find_leg(12));
        assert_eq!(a.nucleus, b.nucleus);
        assert_ne!(a.nucleus, c.nucleus);
        for leg in [&a, &c] {
            assert!(leg.orbit.len() <= ORBIT);
            assert!((SMALLEST..=BIGGEST).contains(&leg.size.abs()));
        }
    }

    #[test]
    fn the_first_leg_is_a_copy() {
        let leg = first_leg();
        assert!(
            (SMALLEST..=BIGGEST).contains(&leg.size.abs()),
            "size {:e}",
            leg.size.abs()
        );
        // Its orbit closes.
        let last = *leg.orbit.last().unwrap();
        assert!((last * last + leg.nucleus).abs() < 1e-6);
    }

    #[test]
    fn the_shader_takes_the_steps_the_search_counts_on() {
        let source = crate::gfx::presets::PRESETS[crate::gfx::presets::index_of("jawbreaker")].source;
        assert!(source.contains(&format!("const MAXIT: i32 = {STEPS};")));
    }

    #[test]
    fn the_search_finds_half_open_ground() {
        for seed in [3, 4, 5] {
            let open = openness(&find_leg(seed));
            assert!((0.3..=0.8).contains(&open), "seed {seed}: {open}");
        }
    }

    #[test]
    fn falls_through_leg_after_leg() {
        let mut dive = Dive::new();
        let first = dive.leg.nucleus;
        let mut legs = 0;
        let mut at = first;
        for _ in 0..200 {
            dive.fast_forward(0.5);
            if dive.leg.nucleus != at {
                legs += 1;
                at = dive.leg.nucleus;
                // A new leg starts at the top, looking at the whole set.
                assert!(
                    (dive.view.ln_s - DISSOLVE_TO.ln()).abs() < 0.6,
                    "starts at {}",
                    dive.view.ln_s
                );
                assert!(dive.view.centre.abs() < 2.0);
            }
            // Never nearer the copy than where the dissolve ends.
            assert!(dive.view.ln_s > (DISSOLVE_TO * dive.leg.size.abs()).ln() - 0.06);
            assert!(dive.params.iter().flatten().all(|x| x.is_finite()));
        }
        assert!(legs >= 3, "{legs} legs in 100 e-folds");
    }

    #[test]
    fn the_handover_keeps_the_picture() {
        // A point of the plane is at the same place on screen in a view and
        // in that view handed on to the next leg.
        let dive = Dive::new();
        let v = View {
            centre: dive.leg.nucleus + Z::new(3.0, -2.0).scale(dive.leg.size.abs()),
            ln_s: (5.0 * dive.leg.size.abs()).ln(),
            theta: 0.7,
        };
        let w = dive.handed_on(v);
        let ps = Z::new(0.4, -0.3);
        let here = v.centre + turn(v.theta, ps).scale(v.ln_s.exp());
        let there = w.centre + turn(w.theta, ps).scale(w.ln_s.exp());
        let back = dive.leg.nucleus + there * dive.leg.size;
        assert!((here - back).abs() < 1e-6 * v.ln_s.exp(), "{here:?} vs {back:?}");
    }
}
