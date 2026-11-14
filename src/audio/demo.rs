//! A tiny generative synth so buttercandy has something to dance to with zero
//! setup: four-on-the-floor drums, an offbeat bass, a detuned pad and a
//! delayed arpeggio, arranged into sections that build and break down.
//!
//! Everything here is allocation-free after construction so it can run inside
//! the audio callback.

use std::f32::consts::{PI, TAU};

const BPM: f32 = 124.0;

struct Section {
    bars: u32,
    kick: bool,
    clap: bool,
    hat: f32,
    bass: bool,
    arp: f32,
    pad: f32,
    cutoff: f32,
    octave: i32,
    riser: bool,
}

const fn sec(
    bars: u32,
    kick: bool,
    clap: bool,
    hat: f32,
    bass: bool,
    arp: f32,
    pad: f32,
    cutoff: f32,
    octave: i32,
    riser: bool,
) -> Section {
    Section {
        bars,
        kick,
        clap,
        hat,
        bass,
        arp,
        pad,
        cutoff,
        octave,
        riser,
    }
}

/// The first section is an intro; the rest loop.
const SECTIONS: [Section; 7] = [
    sec(4, false, false, 0.0, false, 0.6, 1.0, 900.0, 0, false),
    sec(8, true, false, 0.5, true, 0.7, 1.0, 1400.0, 0, false),
    sec(8, true, true, 1.0, true, 1.0, 1.0, 2600.0, 0, false),
    sec(8, true, true, 1.0, true, 1.0, 0.8, 4200.0, 12, false),
    sec(8, false, false, 0.4, false, 0.8, 1.0, 1200.0, 0, true),
    sec(8, true, true, 1.0, true, 1.0, 1.0, 5000.0, 12, false),
    sec(8, true, true, 1.0, true, 0.9, 0.7, 3200.0, 0, false),
];

/// (bass root, pad voicing) in MIDI notes: Am9, Fmaj9, Cmaj7, Gadd9.
const CHORDS: [(i32, [i32; 5]); 4] = [
    (33, [57, 60, 64, 67, 71]),
    (29, [53, 57, 60, 64, 67]),
    (36, [55, 60, 64, 67, 71]),
    (31, [55, 59, 62, 69, 74]),
];

const BASS_PAT: [u8; 16] = [0, 0, 1, 1, 0, 0, 1, 2, 0, 0, 1, 1, 0, 1, 2, 1];
const ARP_PAT: [usize; 16] = [0, 2, 4, 1, 3, 5, 2, 4, 6, 4, 3, 1, 5, 3, 2, 0];
const ARP_SPARSE: [bool; 16] = [
    true, false, false, true, false, false, true, false, true, false, false, true, false, true, false, false,
];

fn mtof(midi: i32) -> f32 {
    440.0 * 2f32.powf((midi - 69) as f32 / 12.0)
}

/// Zavalishin's trapezoidal state-variable filter.
#[derive(Clone, Copy, Default)]
struct Svf {
    ic1: f32,
    ic2: f32,
}

impl Svf {
    /// Returns (lowpass, bandpass, highpass). `g` is the warped cutoff, `k = 1/Q`.
    #[inline]
    fn tick(&mut self, x: f32, g: f32, k: f32) -> (f32, f32, f32) {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, x - k * v1 - v2)
    }
}

/// Polynomial `tan(pi * fc / sr)`, accurate enough over the audio band.
#[inline]
fn warp(fc: f32, sr: f32) -> f32 {
    let x = (PI * fc / sr).min(1.2);
    let x2 = x * x;
    x * (1.0 + x2 * (1.0 / 3.0 + x2 * (2.0 / 15.0)))
}

#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        2.0 * x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + 2.0 * x + 1.0
    } else {
        0.0
    }
}

/// Band-limited sawtooth oscillator.
#[derive(Clone, Copy, Default)]
struct Saw {
    phase: f32,
    inc: f32,
}

impl Saw {
    #[inline]
    fn tick(&mut self) -> f32 {
        let t = self.phase;
        self.phase += self.inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        2.0 * t - 1.0 - poly_blep(t, self.inc)
    }
}

#[derive(Clone, Copy, Default)]
struct Pluck {
    osc: Saw,
    filter: Svf,
    age: f32,
    vel: f32,
    pan: f32,
    cutoff: f32,
    on: bool,
}

struct Comb {
    buf: Vec<f32>,
    idx: usize,
    store: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            idx: 0,
            store: 0.0,
        }
    }

    #[inline]
    fn tick(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let y = self.buf[self.idx];
        self.store = y * (1.0 - damp) + self.store * damp;
        self.buf[self.idx] = x + self.store * feedback;
        self.idx = (self.idx + 1) % self.buf.len();
        y
    }
}

struct Allpass {
    buf: Vec<f32>,
    idx: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            idx: 0,
        }
    }

    #[inline]
    fn tick(&mut self, x: f32) -> f32 {
        let b = self.buf[self.idx];
        self.buf[self.idx] = x + b * 0.5;
        self.idx = (self.idx + 1) % self.buf.len();
        b - x
    }
}

struct Reverb {
    combs: [Comb; 4],
    allpasses: [Allpass; 2],
}

impl Reverb {
    fn new(sr: f32, spread: usize) -> Self {
        let s = |n: usize| ((n + spread) as f32 * sr / 44_100.0) as usize;
        Self {
            combs: [
                Comb::new(s(1116)),
                Comb::new(s(1277)),
                Comb::new(s(1422)),
                Comb::new(s(1557)),
            ],
            allpasses: [Allpass::new(s(556)), Allpass::new(s(341))],
        }
    }

    #[inline]
    fn tick(&mut self, x: f32) -> f32 {
        let mut y = 0.0;
        for c in &mut self.combs {
            y += c.tick(x, 0.86, 0.28);
        }
        for a in &mut self.allpasses {
            y = a.tick(y);
        }
        y * 0.25
    }
}

pub struct DemoSynth {
    /// Which voices play: kick, snare (the clap), hats (and crash), bass,
    /// tune (pad and arpeggio). All of them, unless one is being soloed to
    /// see what the visuals make of it alone; the riser plays only in the
    /// full mix.
    pub voices: [bool; 5],
    sr: f32,
    dt: f32,
    step_len: f32,
    until_step: f32,
    step: u32,
    rng: u32,

    kick_age: f32,
    kick_phase: f32,
    kick_vel: f32,

    hat_age: f32,
    hat_vel: f32,
    hat_decay: f32,
    hat_filter: Svf,

    clap_age: f32,
    clap_filter: Svf,

    crash_age: f32,
    crash_filter: Svf,
    riser_age: f32,
    riser_len: f32,
    riser_filter: Svf,

    bass_osc: Saw,
    bass_sub: f32,
    bass_age: f32,
    bass_len: f32,
    bass_vel: f32,
    bass_filter: Svf,

    pad_oscs: [Saw; 10],
    pad_level: f32,
    pad_target: f32,
    pad_filters: [Svf; 2],
    pad_cutoff: f32,
    pad_cutoff_target: f32,
    lfo: f32,

    plucks: [Pluck; 8],
    next_pluck: usize,

    delay: [Vec<f32>; 2],
    delay_idx: usize,
    delay_lp: [f32; 2],
    reverb: [Reverb; 2],
}

impl DemoSynth {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(8000) as f32;
        let step_len = sr * 60.0 / BPM / 4.0;
        let delay_len = (step_len * 3.0) as usize;
        Self {
            voices: [true; 5],
            sr,
            dt: 1.0 / sr,
            step_len,
            until_step: 0.0,
            step: 0,
            rng: 0x2545_F491,
            kick_age: 10.0,
            kick_phase: 0.0,
            kick_vel: 0.0,
            hat_age: 10.0,
            hat_vel: 0.0,
            hat_decay: 60.0,
            hat_filter: Svf::default(),
            clap_age: 10.0,
            clap_filter: Svf::default(),
            crash_age: 100.0,
            crash_filter: Svf::default(),
            riser_age: 100.0,
            riser_len: 1.0,
            riser_filter: Svf::default(),
            bass_osc: Saw::default(),
            bass_sub: 0.0,
            bass_age: 10.0,
            bass_len: 0.1,
            bass_vel: 0.0,
            bass_filter: Svf::default(),
            pad_oscs: [Saw::default(); 10],
            pad_level: 0.0,
            pad_target: 0.0,
            pad_filters: [Svf::default(); 2],
            pad_cutoff: 900.0,
            pad_cutoff_target: 900.0,
            lfo: 0.0,
            plucks: [Pluck::default(); 8],
            next_pluck: 0,
            delay: [vec![0.0; delay_len], vec![0.0; delay_len]],
            delay_idx: 0,
            delay_lp: [0.0; 2],
            reverb: [Reverb::new(sr, 0), Reverb::new(sr, 23)],
        }
    }

    /// Start the arrangement over without reallocating (safe on the audio thread).
    pub fn restart(&mut self) {
        self.step = 0;
        self.until_step = 0.0;
        self.pad_target = 0.0;
        self.kick_age = 10.0;
        self.bass_age = 10.0;
        self.riser_age = 100.0;
        self.crash_age = 100.0;
        for p in &mut self.plucks {
            p.on = false;
        }
    }

    #[inline]
    fn noise(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 * (2.0 / 16_777_216.0) - 1.0
    }

    fn section(&self, bar: u32) -> (&'static Section, u32) {
        let intro = SECTIONS[0].bars;
        if bar < intro {
            return (&SECTIONS[0], bar);
        }
        let loop_bars: u32 = SECTIONS[1..].iter().map(|s| s.bars).sum();
        let mut b = (bar - intro) % loop_bars;
        for s in &SECTIONS[1..] {
            if b < s.bars {
                return (s, b);
            }
            b -= s.bars;
        }
        (&SECTIONS[1], 0)
    }

    fn trigger_step(&mut self) {
        let step = self.step;
        self.step = self.step.wrapping_add(1);
        let bar = step / 16;
        let s16 = (step % 16) as usize;
        let (sec, bar_in_sec) = self.section(bar);
        let (root, voicing) = CHORDS[(bar / 2 % 4) as usize];

        if sec.kick && (s16.is_multiple_of(4) || (bar_in_sec == sec.bars - 1 && s16 == 14)) {
            self.kick_age = 0.0;
            self.kick_phase = 0.0;
            self.kick_vel = if s16.is_multiple_of(4) { 1.0 } else { 0.7 };
        }
        if sec.clap && (s16 == 4 || s16 == 12) {
            self.clap_age = 0.0;
        }
        if sec.hat > 0.0 {
            let open = s16 == 14 && bar_in_sec % 2 == 1;
            let vel = if s16 % 4 == 2 {
                0.9 * sec.hat
            } else if sec.hat >= 1.0 && s16 % 2 == 1 {
                0.35
            } else if sec.hat >= 1.0 {
                0.2
            } else {
                0.0
            };
            if vel > 0.0 {
                self.hat_age = 0.0;
                self.hat_vel = vel;
                self.hat_decay = if open { 14.0 } else { 70.0 };
            }
        }
        if sec.bass && BASS_PAT[s16] != 0 {
            let up = BASS_PAT[s16] == 2;
            self.bass_osc.inc = mtof(root + if up { 12 } else { 0 }) / self.sr;
            self.bass_age = 0.0;
            self.bass_len = self.step_len / self.sr * 0.9;
            self.bass_vel = if up { 0.7 } else { 1.0 };
        }
        if s16 == 0 {
            // Retune the pad on every bar; the level glides so it never clicks.
            for (i, osc) in self.pad_oscs.iter_mut().enumerate() {
                let detune = if i % 2 == 0 { -0.07 } else { 0.07 };
                osc.inc = mtof(voicing[i / 2]) * 2f32.powf(detune / 12.0) / self.sr;
            }
            self.pad_target = sec.pad;
            self.pad_cutoff_target = sec.cutoff;
            if bar_in_sec == 0 && sec.kick {
                self.crash_age = 0.0;
            }
            if sec.riser && bar_in_sec == sec.bars - 2 {
                self.riser_age = 0.0;
                self.riser_len = self.step_len * 32.0 / self.sr;
            }
        }
        if sec.arp >= 1.0 || ARP_SPARSE[s16] {
            let idx = ARP_PAT[(step % 16) as usize] + (bar % 2) as usize;
            let note = voicing[idx % 5] + 12 * (idx / 5) as i32 + sec.octave;
            let accent = if s16.is_multiple_of(4) {
                1.0
            } else if s16.is_multiple_of(2) {
                0.75
            } else {
                0.55
            };
            let slot = self.next_pluck;
            self.next_pluck = (slot + 1) % self.plucks.len();
            let p = &mut self.plucks[slot];
            p.osc.inc = mtof(note) / self.sr;
            p.age = 0.0;
            p.vel = accent * sec.arp.min(1.0);
            p.pan = if step.is_multiple_of(2) { 0.3 } else { 0.7 };
            p.cutoff = sec.cutoff;
            p.on = true;
        }
    }

    #[inline]
    fn tick(&mut self) -> (f32, f32) {
        self.until_step -= 1.0;
        if self.until_step <= 0.0 {
            self.until_step += self.step_len;
            self.trigger_step();
        }
        let (sr, dt) = (self.sr, self.dt);
        let white = self.noise();

        // --- drums ---------------------------------------------------------
        let mut drums = 0.0;
        if self.kick_age < 0.5 {
            let t = self.kick_age;
            self.kick_phase += (46.0 + 125.0 * (-t * 34.0).exp()) * dt;
            let env = (t * 500.0).min(1.0) * (-t * 8.5).exp();
            drums += (TAU * self.kick_phase).sin() * env * 0.95 * self.kick_vel;
            drums += white * (-t * 450.0).exp() * 0.18;
            self.kick_age += dt;
        }
        let duck = 1.0 - 0.72 * (-self.kick_age * 9.0).exp();

        let mut tops = 0.0;
        if self.hat_age < 0.6 {
            let (_, _, hp) = self.hat_filter.tick(white, warp(8200.0, sr), 0.9);
            tops += hp * (-self.hat_age * self.hat_decay).exp() * 0.24 * self.hat_vel;
            self.hat_age += dt;
        }
        let mut clap = 0.0;
        if self.clap_age < 0.4 {
            let t = self.clap_age;
            // Three quick bursts, then a tail.
            let burst = if t < 0.03 {
                ((t * 90.0).fract() < 0.6) as u32 as f32
            } else {
                1.0
            };
            let env = if t < 0.03 { 0.7 } else { (-(t - 0.03) * 22.0).exp() };
            let (_, bp, _) = self.clap_filter.tick(white, warp(1650.0, sr), 0.85);
            clap = bp * burst * env * 1.2;
            self.clap_age += dt;
        }
        if self.crash_age < 3.0 {
            let (_, _, hp) = self.crash_filter.tick(white, warp(5200.0, sr), 1.2);
            tops += hp * (-self.crash_age * 2.2).exp() * 0.2;
            self.crash_age += dt;
        }
        let mut riser = 0.0;
        if self.riser_age < self.riser_len {
            let x = self.riser_age / self.riser_len;
            let (_, bp, _) = self.riser_filter.tick(white, warp(300.0 * 30f32.powf(x), sr), 0.5);
            riser = bp * x * x * 0.45;
            self.riser_age += dt;
        }

        // --- bass ----------------------------------------------------------
        let mut bass = 0.0;
        if self.bass_age < self.bass_len + 0.08 {
            let t = self.bass_age;
            let gate = if t < self.bass_len {
                1.0
            } else {
                (-(t - self.bass_len) * 60.0).exp()
            };
            let env = (t * 250.0).min(1.0) * gate;
            self.bass_sub += self.bass_osc.inc * 0.5;
            if self.bass_sub >= 1.0 {
                self.bass_sub -= 1.0;
            }
            let raw = self.bass_osc.tick() * 0.6 + (TAU * self.bass_sub).sin() * 0.55;
            let cutoff = 130.0 + 1000.0 * self.bass_vel * (-t * 13.0).exp();
            let (lp, _, _) = self.bass_filter.tick(raw, warp(cutoff, sr), 0.55);
            bass = lp * env * 0.5;
            self.bass_age += dt;
        }

        // --- pad -----------------------------------------------------------
        self.pad_level += (self.pad_target - self.pad_level) * 2.5 * dt;
        self.pad_cutoff += (self.pad_cutoff_target - self.pad_cutoff) * 1.5 * dt;
        self.lfo += 0.11 * dt;
        if self.lfo >= 1.0 {
            self.lfo -= 1.0;
        }
        let mut pad = [0.0f32; 2];
        for (i, osc) in self.pad_oscs.iter_mut().enumerate() {
            pad[i % 2] += osc.tick();
        }
        let pad_g = warp(self.pad_cutoff * (0.75 + 0.35 * (TAU * self.lfo).sin()), sr);
        for (ch, f) in pad.iter_mut().zip(self.pad_filters.iter_mut()) {
            *ch = f.tick(*ch, pad_g, 0.9).0 * 0.05 * self.pad_level * duck;
        }

        // --- arpeggio ------------------------------------------------------
        let mut arp = [0.0f32; 2];
        for p in &mut self.plucks {
            if !p.on {
                continue;
            }
            let t = p.age;
            let env = (t * 600.0).min(1.0) * (-t * 9.0).exp();
            let cutoff = p.cutoff * (0.35 + 1.5 * (-t * 16.0).exp());
            let (lp, _, _) = p.filter.tick(p.osc.tick(), warp(cutoff, sr), 0.45);
            let v = lp * env * p.vel * 0.16;
            arp[0] += v * (1.0 - p.pan);
            arp[1] += v * p.pan;
            p.age += dt;
            p.on = t < 0.7;
        }

        // --- ping-pong delay on the arpeggio --------------------------------
        let idx = self.delay_idx;
        let tap = [self.delay[0][idx], self.delay[1][idx]];
        for ch in 0..2 {
            self.delay_lp[ch] += (tap[ch] - self.delay_lp[ch]) * 0.35;
        }
        self.delay[0][idx] = arp[0] + self.delay_lp[1] * 0.42;
        self.delay[1][idx] = arp[1] + self.delay_lp[0] * 0.42;
        self.delay_idx = (idx + 1) % self.delay[0].len();

        // --- reverb + mix ---------------------------------------------------
        let mut out = [0.0f32; 2];
        // Soloing: what is not wanted is dropped here, after every voice has
        // run, so that the arrangement stays in step whatever is muted.
        let on = |voice: usize| if self.voices[voice] { 1.0 } else { 0.0 };
        let (drums, clap, tops, bass) = (drums * on(0), clap * on(1), tops * on(2), bass * on(3));
        let riser = if self.voices == [true; 5] { riser } else { 0.0 };
        for ch in 0..2 {
            pad[ch] *= on(4);
            arp[ch] *= on(4);
        }
        let tap = [tap[0] * on(4), tap[1] * on(4)];
        for ch in 0..2 {
            let wet_in = pad[ch] * 0.6 + (arp[ch] + tap[ch] * 0.5) * 0.5 + (clap + riser) * 0.35;
            let wet = self.reverb[ch].tick(wet_in);
            let dry = drums
                + bass * (0.5 + 0.5 * duck)
                + tops
                + clap
                + riser
                + pad[ch]
                + (arp[ch] + tap[ch] * 0.55) * (0.6 + 0.4 * duck);
            out[ch] = ((dry + wet * 0.75) * 0.85).tanh();
        }
        (out[0], out[1])
    }

    /// Fill `out` with interleaved stereo frames.
    pub fn render(&mut self, out: &mut [f32]) {
        for frame in out.as_chunks_mut::<2>().0 {
            let (l, r) = self.tick();
            frame[0] = l;
            frame[1] = r;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_bounded_audio_with_content() {
        let mut synth = DemoSynth::new(48_000);
        let mut buf = vec![0.0f32; 48_000 * 2 * 12];
        synth.render(&mut buf);
        let peak = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let rms = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
        assert!(buf.iter().all(|s| s.is_finite()));
        assert!(peak <= 1.0, "peak {peak}");
        assert!(rms > 0.03, "rms {rms}");
    }
}
