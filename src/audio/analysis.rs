//! Turns raw samples into the signals the shaders consume: a log-frequency
//! spectrum, a stabilised waveform, band energies, a beat pulse and per-band
//! onsets, brightness, the harmony (chroma) and a scrolling spectrogram.

use std::f32::consts::TAU;

use super::fft::Fft;
use super::ring::Tap;

/// Width of the audio texture: log-spaced spectrum bins / waveform samples.
pub const BINS: usize = 512;
/// Rows in the audio texture: spectrum, waveform, peak-hold, fast spectrum,
/// chroma.
pub const ROWS: usize = 5;
/// Texels each pitch class fills in the chroma row. Wide blocks, so a
/// linearly filtered lookup at a block's centre never blends its neighbours.
pub const CHROMA_TEXELS: usize = BINS / 12;
/// The age reported for an onset that has not happened (yet, or for a while).
pub const AGE_CAP: f32 = 10.0;
/// Spectrogram history texture size (frequency x time).
pub const HIST_W: usize = 128;
pub const HIST_H: usize = 256;

const HIST_RATE: f32 = 60.0;
const F_MIN: f32 = 30.0;
const F_MAX: f32 = 16_000.0;
const DB_FLOOR: f32 = -70.0;
const DB_CEIL: f32 = -12.0;
/// How spread out across a band new sound has to be to count as noise (a
/// snare, a clap, a hat) rather than notes. See [`spread`].
const NOISY: f32 = 0.36;
/// How long a hit above the kick waits before it counts, in seconds.
const CONFIRM: f32 = 0.02;
/// The voices a pending hit may turn out to belong to.
const SNARE: u8 = 0;
const HAT: u8 = 1;
const NOTE: u8 = 2;

fn bin_of(freq: f32) -> usize {
    let x = (freq / F_MIN).ln() / (F_MAX / F_MIN).ln();
    (x.clamp(0.0, 1.0) * BINS as f32) as usize
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// One percussive voice of the music, and the detector behind it: it fires
/// when more new sound arrives in its band than has been arriving lately
/// (spectral flux).
///
/// The detector works on linear amplitude, relative to the band's own recent
/// peak. The spectrum the shaders see is compressed for display, and on that
/// scale a kick landing on top of a bass note barely registers; here it
/// towers over it, while a quiet hi-hat still counts in a band that holds
/// nothing else.
pub struct Onset {
    /// 1.0 on a hit, decaying towards 0.
    pub pulse: f32,
    /// Number of hits so far.
    pub count: f32,
    /// Seconds since each of the last four hits, newest first, capped at
    /// [`AGE_CAP`]. Four, so a ripple can outlive the hits that follow it.
    pub ages: [f32; 4],
    /// How hard each of those four hits was, 0..1, against the hardest
    /// lately: a ghost note is small, an accent is 1.
    pub hits: [f32; 4],
    hz: (f32, f32),
    /// The shortest time between two hits, in seconds.
    gap: f32,
    /// How fast the pulse decays, per second.
    decay: f32,
    /// The band's amplitude at its loudest lately.
    peak: f32,
    /// Running statistics of the flux, for the adaptive threshold.
    mean: f32,
    var: f32,
    /// Seconds since the detector last triggered, whoever took the hit.
    since: f32,
    /// The flux of the hardest hit lately, which strengths are measured by.
    hardest: f32,
    /// This frame's flux as a fraction of what it takes to trigger.
    nearness: f32,
    /// While set, the detector does not learn from what it hears: its sense
    /// of the band's level and of what is usual stays as it was. Used to keep
    /// the sizzle of a snare from becoming the hats' idea of loud.
    deaf: bool,
}

impl Onset {
    fn new(lo_hz: f32, hi_hz: f32, gap: f32, decay: f32) -> Self {
        Self {
            pulse: 0.0,
            count: 0.0,
            ages: [AGE_CAP; 4],
            hits: [0.0; 4],
            hz: (lo_hz, hi_hz),
            gap,
            decay,
            peak: 0.0,
            mean: 0.0,
            var: 0.0,
            since: AGE_CAP,
            hardest: 0.0,
            nearness: 0.0,
            deaf: false,
        }
    }

    /// Let `dt` pass: the pulse decays and the hits grow older.
    fn tick(&mut self, dt: f32) {
        self.pulse *= (-dt * self.decay).exp();
        for age in &mut self.ages {
            *age = (*age + dt).min(AGE_CAP);
        }
        self.since = (self.since + dt).min(AGE_CAP);
    }

    /// Run the detector for this frame. Returns how hard the band was hit
    /// (0..1), if it was; whether that counts as this voice's hit is for the
    /// caller to decide and to record with [`Onset::strike`].
    ///
    /// `mags` is the FFT magnitude spectrum, `bin_hz` wide per bin, and `rise`
    /// how far each bin stands above where it was a moment ago. `floor` is
    /// the quietest peak worth measuring against, so a band with next to
    /// nothing in it does not fire on its own noise. `keen` (0..1) is how
    /// readily to fire: at 1 every onset counts; lower, only the more
    /// prominent ones do, and no faster than a longer gap apart, so a busy
    /// hi-hat line thins out to its accents.
    fn probe(&mut self, mags: &[f32], rise: &[f32], bin_hz: f32, floor: f32, keen: f32, dt: f32) -> Option<f32> {
        self.tick(dt);
        let k0 = ((self.hz.0 / bin_hz).round() as usize).clamp(1, mags.len() - 1);
        let k1 = ((self.hz.1 / bin_hz).round() as usize).clamp(k0, mags.len() - 1);
        let norm = |v: &[f32]| v[k0..=k1].iter().map(|x| x * x).sum::<f32>().sqrt();
        if !self.deaf {
            self.peak = norm(mags).max(self.peak * (-dt * 0.5).exp());
        }
        // Bin by bin, so a clap still counts while the pad under it ducks.
        let flux = norm(rise) / self.peak.max(floor);
        if !self.deaf {
            let a = 1.0 - (-dt / 0.9).exp();
            self.mean += (flux - self.mean) * a;
            self.var += ((flux - self.mean).powi(2) - self.var) * a;
        }

        let reluctance = 1.0 - keen.clamp(0.0, 1.0);
        let threshold = self.mean + self.var.sqrt() + 0.15 + 0.08 * reluctance;
        self.nearness = flux / threshold;
        self.hardest *= (-dt * 0.15).exp();
        if flux > threshold && self.since > self.gap * (1.0 + 1.5 * reluctance) {
            self.since = 0.0;
            self.hardest = self.hardest.max(flux);
            Some((flux / self.hardest).clamp(0.2, 1.0))
        } else {
            None
        }
    }

    /// Record a hit of the given strength.
    fn strike(&mut self, strength: f32) {
        self.pulse = 1.0;
        self.count += 1.0;
        self.ages.rotate_right(1);
        self.ages[0] = 0.0;
        self.hits.rotate_right(1);
        self.hits[0] = strength;
    }
}

/// How many of the bins between `lo` and `hi` Hz share in the new sound, as
/// a fraction: near 0 when it is a note or two (a few bins hold it all), a
/// third or more when it is a burst of noise (a snare, a clap, a hat).
fn spread(rise: &[f32], bin_hz: f32, lo: f32, hi: f32) -> f32 {
    let k0 = ((lo / bin_hz).round() as usize).clamp(1, rise.len() - 1);
    let k1 = ((hi / bin_hz).round() as usize).clamp(k0, rise.len() - 1);
    let band = &rise[k0..=k1];
    let (sum, squares) = band.iter().fold((0.0f32, 0.0f32), |(s, q), r| (s + r, q + r * r));
    if squares <= 1e-12 {
        return 0.0;
    }
    sum * sum / (band.len() as f32 * squares)
}

pub struct Analysis {
    /// `BINS x ROWS` single-channel texels, uploaded every frame.
    pub tex: Vec<u8>,
    /// `HIST_W x HIST_H` scrolling spectrogram (a ring of rows).
    pub hist: Vec<u8>,
    pub hist_dirty: bool,
    /// Fractional row index of "now" in the history ring.
    pub hist_head: f32,

    pub bass: f32,
    pub mid: f32,
    pub treb: f32,
    pub level: f32,
    /// The beat: onsets of the low end (below 200 Hz).
    pub beat: Onset,
    /// The drums, told apart by their sound: the kick (a hit in the low
    /// end), the snare or clap (a burst of noise through the mids) and the
    /// hats (a burst of noise up high with nothing under it).
    pub onsets: [Onset; 3],
    /// Notes starting: something tonal arriving in the mids, which is a
    /// melody or a chord rather than a drum.
    pub note: Onset,
    /// The bass line, 0..1: the low end that stays when the kicks are taken
    /// out of it.
    pub bassline: f32,
    /// How much melody and harmony there is, 0..1: the pitched part of the
    /// mids.
    pub melody: f32,
    /// How high the tune sits, 0..1 (a log scale from 250 Hz to 2 kHz):
    /// rises as a melody rises. Holds its place when there is no tune.
    pub pitch: f32,
    /// Spectral centroid on the log-frequency axis, 0..1: how bright the
    /// sound is right now.
    pub centroid: f32,
    /// How full the spectrum has been over the last couple of seconds, 0..1:
    /// low in a breakdown, high in a drop.
    pub energy: f32,
    /// Energy per pitch class (C, C#, ... B), 0..1, with the level that all
    /// twelve share (noise, drums) taken out.
    pub chroma: [f32; 12],
    /// Where the harmony sits on the circle of fifths, 0..1 and cyclic.
    /// Related chords are neighbours, so it glides rather than jumps.
    pub harmony: f32,
    /// How clearly the sound has a tonal centre, 0 (noise, silence) to 1.
    pub tonal: f32,
    /// Time that runs faster when the music is louder.
    pub phase: f32,
    /// How much of the output is the ambient idle animation (0..1).
    pub idle: f32,
    /// User gain applied on top of automatic gain control.
    pub sensitivity: f32,
    /// How hard the picture is asked to react, 0..1. At 1 every signal is
    /// passed on as measured. Lower, the band energies swing less around
    /// their recent average, the spectrum moves more smoothly, and fewer
    /// claps and hats count as events. Presets also see it as `uReact` and
    /// scale their own motion by it.
    pub reactivity: f32,

    fft: Fft,
    rate: u32,
    samples: Vec<f32>,
    mags: Vec<f32>,
    /// Each FFT bin's magnitude about 30 ms ago, and how far it has risen
    /// above that. Measured against a trailing level rather than the last
    /// frame, the rise does not depend on the frame rate.
    trail: Vec<f32>,
    rise: Vec<f32>,
    edges: Vec<f32>,
    shaped: Vec<f32>,
    smooth: Vec<f32>,
    fast: Vec<f32>,
    peaks: Vec<f32>,
    peak_vel: Vec<f32>,
    peak_hold: Vec<f32>,

    /// The band energies as measured, and their averages over the last
    /// second or two, between which `reactivity` picks.
    bands: [f32; 4],
    bands_calm: [f32; 4],
    agc: f32,
    wave_agc: f32,
    /// The amplitude of the whole spectrum at its loudest lately, which the
    /// onset detectors treat as the level of the mix.
    loud: f32,
    /// The smoothed harmony vector whose angle is `harmony`.
    key: (f32, f32),
    /// The low end's level when it last held steady, and its level about
    /// 40 ms ago, to tell whether it is holding.
    low_held: f32,
    low_trail: f32,
    /// The low end's level over the last fifth of a second, with the time of
    /// each reading.
    lows: std::collections::VecDeque<(f32, f32)>,
    /// The bass line and the tune at their loudest lately.
    bass_peak: f32,
    tune_peak: f32,
    /// Hits waiting to be confirmed: which voice, seconds left to wait, how
    /// hard, and how much new sound came with it.
    pending: Vec<(u8, f32, f32, f32)>,
    /// How much new sound the low end has taken lately: the size of a kick,
    /// to measure its click against.
    kick_weight: f32,
    /// How long there has been sound to listen to, in seconds.
    heard_for: f32,
    clock: f32,
    silent_for: f32,
    stale_for: f32,
    last_written: u64,
    hist_acc: f32,
    hist_row: usize,
}

impl Analysis {
    pub fn new() -> Self {
        let mut a = Self {
            tex: vec![0; BINS * ROWS],
            hist: vec![0; HIST_W * HIST_H],
            hist_dirty: true,
            hist_head: 0.0,
            bass: 0.0,
            mid: 0.0,
            treb: 0.0,
            level: 0.0,
            beat: Onset::new(30.0, 200.0, 0.23, 4.5),
            onsets: [
                Onset::new(30.0, 150.0, 0.16, 5.0),
                Onset::new(300.0, 2500.0, 0.12, 7.0),
                Onset::new(4000.0, 14_000.0, 0.07, 11.0),
            ],
            note: Onset::new(0.0, 0.0, 0.0, 6.0),
            bassline: 0.0,
            melody: 0.0,
            pitch: 0.5,
            centroid: 0.4,
            energy: 0.0,
            chroma: [0.0; 12],
            harmony: 0.0,
            tonal: 0.0,
            phase: 0.0,
            idle: 1.0,
            sensitivity: 1.0,
            reactivity: 1.0,
            fft: Fft::new(4096),
            rate: 0,
            samples: Vec::new(),
            mags: Vec::new(),
            trail: Vec::new(),
            rise: Vec::new(),
            edges: vec![0.0; BINS + 1],
            shaped: vec![0.0; BINS],
            smooth: vec![0.0; BINS],
            fast: vec![0.0; BINS],
            peaks: vec![0.0; BINS],
            peak_vel: vec![0.0; BINS],
            peak_hold: vec![0.0; BINS],
            bands: [0.0; 4],
            bands_calm: [0.0; 4],
            agc: 0.6,
            wave_agc: 0.3,
            loud: 0.0,
            key: (0.0, 0.0),
            low_held: 0.0,
            low_trail: 0.0,
            lows: std::collections::VecDeque::with_capacity(256),
            bass_peak: 0.0,
            tune_peak: 0.0,
            pending: Vec::with_capacity(16),
            kick_weight: 0.0,
            heard_for: 0.0,
            clock: 0.0,
            silent_for: 10.0,
            stale_for: 10.0,
            last_written: 0,
            hist_acc: 0.0,
            hist_row: 0,
        };
        a.configure(48_000);
        a
    }

    fn configure(&mut self, rate: u32) {
        self.rate = rate;
        // Keep the analysis window near 85 ms whatever the device rate is.
        let n = match rate {
            0..=50_000 => 4096,
            50_001..=100_000 => 8192,
            _ => 16384,
        };
        if self.fft.len() != n {
            self.fft = Fft::new(n);
        }
        self.samples = vec![0.0; n];
        self.mags = vec![0.0; n / 2];
        self.trail = vec![0.0; n / 2];
        self.rise = vec![0.0; n / 2];
        let bin_hz = rate as f32 / n as f32;
        for (i, e) in self.edges.iter_mut().enumerate() {
            let f = F_MIN * (F_MAX / F_MIN).powf(i as f32 / BINS as f32);
            *e = (f / bin_hz).min((n / 2 - 1) as f32);
        }
    }

    /// Advance by `dt` seconds. Pass `None` when nothing is playing.
    pub fn update(&mut self, tap: Option<&Tap>, dt: f32) {
        let dt = dt.clamp(0.0005, 0.1);
        self.clock += dt;

        // ---- acquire --------------------------------------------------------
        let mut live = false;
        if let Some(tap) = tap {
            if tap.rate() != self.rate {
                self.configure(tap.rate());
            }
            let written = tap.latest(&mut self.samples);
            // Loopback capture delivers nothing at all while the system is
            // silent, so a stalled counter means silence, not a frozen frame.
            if written == self.last_written {
                self.stale_for += dt;
            } else {
                self.stale_for = 0.0;
                self.last_written = written;
            }
            live = self.stale_for < 0.12;
        }
        let n = self.fft.len();
        if !live {
            self.samples.fill(0.0);
        }

        // ---- spectrum -------------------------------------------------------
        self.fft.magnitudes(&self.samples, &mut self.mags);
        let last = n / 2 - 1;
        let mut frame_max = 0.0f32;
        for i in 0..BINS {
            let (b0, b1) = (self.edges[i], self.edges[i + 1]);
            let mag = if b1 - b0 < 1.0 {
                // Narrower than one FFT bin: interpolate.
                let c = (b0 + b1) * 0.5;
                let k = (c as usize).min(last);
                let f = c - k as f32;
                self.mags[k] * (1.0 - f) + self.mags[(k + 1).min(last)] * f
            } else {
                let (k0, k1) = (b0.ceil() as usize, (b1.floor() as usize).min(last));
                self.mags[k0..=k1.max(k0)].iter().cloned().fold(0.0, f32::max)
            };
            let db = 20.0 * (mag + 1e-9).log10();
            let mut v = ((db - DB_FLOOR) / (DB_CEIL - DB_FLOOR)).clamp(0.0, 1.0);
            // Music rolls off towards the top end; tilt it back so highs read.
            let x = i as f32 / BINS as f32;
            v += (x - 0.3) * 0.45 * smoothstep(0.02, 0.2, v);
            self.shaped[i] = v.max(0.0);
            frame_max = frame_max.max(v);
        }

        // Automatic gain: follow the loudest bin, releasing over a few seconds,
        // so a quiet source (low system volume, a distant microphone) still
        // fills the screen. The floor stops us amplifying pure noise.
        self.agc = frame_max.max(self.agc * (-dt * 0.35).exp());
        let gain = self.sensitivity / self.agc.max(0.25);

        if frame_max < 0.03 {
            self.silent_for += dt;
        } else {
            self.silent_for = 0.0;
        }
        let idle_target = if self.silent_for > 1.0 { 1.0 } else { 0.0 };
        // Drift into the ambient animation slowly, but snap out of it for music.
        let idle_rate = if idle_target < self.idle { 6.0 } else { 1.6 };
        self.idle += (idle_target - self.idle) * (1.0 - (-dt * idle_rate).exp());
        let idle = self.idle;
        let t = self.clock;

        // ---- beat and onset detection ----------------------------------------
        let bin_hz = self.rate as f32 / n as f32;
        let whole = self.mags.iter().map(|m| m * m).sum::<f32>().sqrt();
        self.loud = whole.max(self.loud * (-dt * 0.3).exp());
        // A band more than 26 dB under the whole mix, or 70 dB under full
        // scale, is measured against that floor instead of its own peak.
        let floor = (self.loud * 0.05).max(3e-4);
        // In the first instant after silence nothing is known about the mix
        // yet, and a band holding only the faint edge of what is arriving
        // elsewhere would count it as a hit. Until there is something to go
        // by, a band has to hold a real share of the sound to fire.
        if whole > 1e-3 {
            self.heard_for += dt;
        } else if self.silent_for > 1.0 {
            self.heard_for = 0.0;
        }
        let floor = if self.heard_for < 0.15 {
            floor.max(whole * 0.3)
        } else {
            floor
        };
        // The trail lags the spectrum by 30 ms whatever the frame rate: a
        // level that climbs steadily stands the same height above it at 30
        // frames a second as at 240. (An exponential follower would lag by
        // more the slower the frames come, and find more onsets.)
        let follow = (dt / 0.03).min(1.0);
        for ((rise, trail), &m) in self.rise.iter_mut().zip(&mut self.trail).zip(&self.mags) {
            *rise = (m - *trail).max(0.0);
            *trail += (m - *trail) * follow;
        }
        // The beat and the kick are the pulse of the music and always
        // count; reactivity only decides how many claps and hats do.
        let keen = self.reactivity;
        let band = |lo: f32, hi: f32| {
            let k0 = ((lo / bin_hz).round() as usize).clamp(1, last);
            let k1 = ((hi / bin_hz).round() as usize).clamp(k0, last);
            self.rise[k0..=k1].iter().map(|r| r * r).sum::<f32>().sqrt()
        };
        // How much new sound there is in the low end, the mids and the top.
        let (lows, mids, top) = (band(30.0, 150.0), band(300.0, 3500.0), band(4000.0, 14_000.0));

        // Something arrived in the low end. It is a kick if a good share of
        // the new sound is down there; the bottom of a snare, or of any
        // burst of noise, comes with far more above it.
        if let Some(strength) = self.beat.probe(&self.mags, &self.rise, bin_hz, floor, 1.0, dt)
            && band(30.0, 200.0) >= 0.5 * band(200.0, 14_000.0)
        {
            self.beat.strike(strength);
        }
        let [kick, snare, hat] = &mut self.onsets;
        self.kick_weight = lows.max(self.kick_weight * (-dt * 12.0).exp());
        if let Some(strength) = kick.probe(&self.mags, &self.rise, bin_hz, floor, 1.0, dt)
            && lows >= 0.5 * band(150.0, 14_000.0)
        {
            kick.strike(strength);
        }

        // Something arrived in the mids. With real weight there, it is a
        // snare or a clap if it is a burst of noise that reaches up into the
        // top, and a note if it is a few clean partials. (With its weight
        // elsewhere it is the edge of a hat or of a kick, and nothing.)
        let in_mids = mids >= 0.4 * top;
        let noisy = spread(&self.rise, bin_hz, 300.0, 3500.0) >= NOISY;
        self.note.tick(dt);
        if let Some(strength) = snare.probe(&self.mags, &self.rise, bin_hz, floor, keen, dt)
            && in_mids
        {
            let voice = if noisy && top >= 0.15 * mids { SNARE } else { NOTE };
            self.pending.push((voice, CONFIRM, strength, mids));
        }
        // Something arrived up high. It is a hat if it is noise, and not the
        // top of a snare (one just hit, or the mids are filling with noise
        // this very moment).
        let of_snare = snare.ages[0] < 0.09 || (noisy && in_mids && snare.nearness > 0.6);
        hat.deaf = of_snare;
        if let Some(strength) = hat.probe(&self.mags, &self.rise, bin_hz, floor, keen, dt)
            && !of_snare
            && spread(&self.rise, bin_hz, 4000.0, 14_000.0) >= NOISY
        {
            self.pending.push((HAT, CONFIRM, strength, top));
        }

        // Nothing above the kick counts until it has waited a moment,
        // because the parts of one sound do not arrive together: the click
        // of a kick comes before its thump, the top of a snare before its
        // body. By then it is clear what belonged to what.
        let kick_just_hit = kick.ages[0] < 0.08;
        // (A snare still waiting its own turn counts as one that has hit.)
        let mut snare_hit = snare.ages[0] < 0.09 || self.pending.iter().any(|(voice, ..)| *voice == SNARE);
        self.pending.retain_mut(|(voice, wait, strength, weight)| {
            *wait -= dt;
            if *wait > 0.0 {
                return true;
            }
            // The click of a kick is a small thing beside the kick itself.
            let click = kick_just_hit && (*voice == HAT || *weight < 0.1 * self.kick_weight);
            match *voice {
                _ if click => {}
                SNARE => {
                    snare.strike(*strength);
                    snare_hit = true;
                }
                NOTE => self.note.strike(*strength),
                // The top of a snare is not a hat.
                _ if !snare_hit => hat.strike(*strength),
                _ => {}
            }
            false
        });
        if snare_hit {
            self.pending.retain(|(voice, ..)| *voice != HAT);
        }

        // ---- contrast -------------------------------------------------------
        let (mut sum, mut moment) = (0.0, 0.0);
        for (i, v) in self.shaped.iter_mut().enumerate() {
            let c = ((*v * gain).clamp(0.0, 1.0) - 0.2).max(0.0) / 0.8;
            *v = c * c.sqrt(); // ^1.5
            sum += *v;
            moment += *v * (i as f32 + 0.5);
        }

        // ---- brightness and fullness ----------------------------------------
        // With nothing sounding the centroid has no meaning: hold it, and let
        // the ambient animation sway it instead.
        let heard = if sum > 0.5 {
            moment / (sum * BINS as f32)
        } else {
            self.centroid
        };
        let centroid = heard * (1.0 - idle) + (0.42 + 0.14 * (t * 0.21).sin()) * idle;
        self.centroid += (centroid - self.centroid) * (1.0 - (-dt * 7.0).exp());
        let energy = (sum / BINS as f32 * 3.2).min(1.0) * (1.0 - idle) + 0.25 * idle;
        self.energy += (energy - self.energy) * (1.0 - (-dt / 1.6).exp());

        // ---- chroma: energy per pitch class ----------------------------------
        // Spectral peaks between 120 Hz and 4 kHz, each refined to a fraction
        // of an FFT bin and added to the pitch class it is nearest to.
        let k0 = ((120.0 / bin_hz).ceil() as usize).max(1);
        let k1 = ((4000.0 / bin_hz) as usize).min(last - 1);
        let mut classes = [0.0f32; 12];
        // The tune: how much pitched sound there is above the bass, and
        // where on a log-frequency scale its weight lies.
        let (mut tune, mut tune_height) = (0.0f32, 0.0f32);
        for k in k0..=k1 {
            let m = self.mags[k];
            if m < 1e-4 || m <= self.mags[k - 1] || m < self.mags[k + 1] {
                continue;
            }
            let (a, b, c) = (self.mags[k - 1].max(1e-9).ln(), m.ln(), self.mags[k + 1].max(1e-9).ln());
            let curve = a - 2.0 * b + c;
            let off = if curve < -1e-6 {
                (0.5 * (a - c) / curve).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            let hz = (k as f32 + off) * bin_hz;
            let note = 69.0 + 12.0 * (hz / 440.0).log2();
            // Peaks that fall between two notes are more likely noise, and
            // ones that have only just appeared may be a drum passing through
            // (a kick sweeps down across an octave or two on its way).
            let in_tune = 1.0 - (note - note.round()).abs();
            let settled = 1.0 - (self.rise[k] / m).min(1.0);
            let weight = m * in_tune * settled;
            classes[(note.round() as i32).rem_euclid(12) as usize] += weight;
            if hz >= 250.0 {
                tune += weight;
                tune_height += weight * (hz / 250.0).ln();
            }
        }

        // ---- the continuous voices: bass line and melody ---------------------
        // The bass line is the low end while it holds steady. A kick does
        // not: it swells for an instant and is dying away from then on. So
        // a reading is only taken when the level is where it was a moment
        // ago, and not in the fifth of a second after a kick, when its swell
        // would pass for steady. Between readings the last one stands.
        let k0 = ((40.0 / bin_hz).round() as usize).max(1);
        let k1 = ((140.0 / bin_hz).round() as usize).clamp(k0, last);
        let low = self.mags[k0..=k1].iter().map(|m| m * m).sum::<f32>().sqrt();
        let was = self.low_trail;
        self.low_trail += (low - was) * (dt / 0.04).min(1.0);
        let steady = (low - was).abs() <= 0.15 * low.max(was) + floor * 0.1;
        // Nor does a dying kick hold a candle to what it was a fifth of a
        // second ago; a bass note, or a silence, does.
        while self.lows.front().is_some_and(|&(at, _)| self.clock - at > 0.2) {
            self.lows.pop_front();
        }
        self.lows.push_back((self.clock, low));
        let lately = self.lows.iter().map(|&(_, v)| v).fold(0.0, f32::max);
        if steady && self.onsets[0].ages[0] > 0.2 && low >= 0.6 * lately {
            self.low_held = low;
        }
        let held = self.low_held;
        // Each is measured against its own loudest lately, so that a tune
        // does not seem to fade when the drums come in over it; but against
        // a fair share of the whole mix at least, so that the tail of a
        // kick is not taken for a bass line.
        let follow = |level: f32, least: f32, peak: &mut f32, out: &mut f32, ambient: f32| {
            *peak = level.max(*peak * (-dt * 0.12).exp());
            let heard = (level / peak.max(least)).min(1.0);
            let v = heard * (1.0 - idle) + ambient * idle;
            *out += (v - *out) * (1.0 - (-dt * if v > *out { 22.0 } else { 4.0 }).exp());
        };
        follow(
            held,
            self.loud * 0.5,
            &mut self.bass_peak,
            &mut self.bassline,
            0.3 + 0.15 * (t * 0.43).sin(),
        );
        // Only what stands above the level that all twelve pitch classes
        // share is tune; the rest is noise that happens to have peaks.
        let shared = classes.iter().sum::<f32>() / 12.0;
        // (Noise can pile up in one class by chance; how clearly there is
        // a key at all, as of the last frame, settles it.)
        let pitched = classes.iter().map(|c| (c - shared).max(0.0)).sum::<f32>() * self.tonal * self.tonal;
        follow(
            pitched,
            self.loud * 0.1,
            &mut self.tune_peak,
            &mut self.melody,
            0.35 + 0.15 * (t * 0.31).cos(),
        );
        // How high the tune sits: the centre of its weight, on a log scale
        // from 250 Hz to 2 kHz. With no tune to speak of, it stays put.
        if pitched > 0.2 * self.tune_peak.max(floor) && tune > 0.0 {
            let height = (tune_height / tune / 8.0f32.ln()).clamp(0.0, 1.0);
            self.pitch += (height - self.pitch) * (1.0 - (-dt * 6.0).exp());
        }
        let wander = 0.5 + 0.25 * (t * 0.17).sin();
        self.pitch += (wander - self.pitch) * idle * (1.0 - (-dt * 2.0).exp());
        // What all twelve have in common is noise and drums, not harmony.
        let mean = classes.iter().sum::<f32>() / 12.0;
        let max = classes.iter().cloned().fold(1e-4, f32::max);
        let up = 1.0 - (-dt * 14.0).exp();
        let down = 1.0 - (-dt * 3.5).exp();
        let (mut kx, mut ky, mut total) = (0.0, 0.0, 0.0);
        for (i, cur) in self.chroma.iter_mut().enumerate() {
            // Seven semitones per step walks the circle of fifths.
            let angle = (i * 7 % 12) as f32 / 12.0 * TAU;
            let heard = ((classes[i] - mean).max(0.0) / max * (12.0 / 11.0)).min(1.0);
            // While idle, a cluster of related notes drifts around the circle.
            let ambient = 0.75 * (0.5 + 0.5 * (angle - t * 0.25).cos()).powi(2);
            let v = heard * (1.0 - idle) + ambient * idle;
            *cur += (v - *cur) * if v > *cur { up } else { down };
            kx += *cur * angle.cos();
            ky += *cur * angle.sin();
            total += *cur;
        }
        self.tonal = (kx.hypot(ky) / total.max(1.0)).min(1.0);
        // A weak tonal centre pulls weakly, so noise leaves the key where the
        // last chord put it rather than spinning it.
        // Asked to react less, the key also takes longer to move: within a
        // chord it shifts a little with every note, and a picture steered
        // by it would never rest.
        let react = self.reactivity.clamp(0.0, 1.0);
        // And with no tune to speak of (drums alone) it does not move at
        // all: there is no key to find, only noise to mistake for one.
        let tuned = smoothstep(0.05, 0.3, self.melody);
        let pull = 1.0 - (-dt * (0.35 + 1.65 * react * react * react) * tuned).exp();
        self.key.0 += (kx / 12.0 - self.key.0) * pull;
        self.key.1 += (ky / 12.0 - self.key.1) * pull;
        if self.key.0.hypot(self.key.1) > 1e-4 {
            self.harmony = (self.key.1.atan2(self.key.0) / TAU).rem_euclid(1.0);
        }
        for (i, texel) in self.tex[BINS * 4..].iter_mut().enumerate() {
            *texel = byte(self.chroma[(i / CHROMA_TEXELS).min(11)]);
        }

        // ---- smoothing, peaks, texture rows ---------------------------------
        // A calmer picture gets a spectrum that takes longer to move.
        let ease = 0.4 + 0.6 * self.reactivity.clamp(0.0, 1.0);
        let up = 1.0 - (-dt * 45.0 * ease).exp();
        let down = 1.0 - (-dt * 8.0 * ease).exp();
        let up_fast = 1.0 - (-dt * 90.0 * ease).exp();
        let down_fast = 1.0 - (-dt * 22.0 * ease).exp();
        for i in 0..BINS {
            let x = i as f32 / BINS as f32;
            // Ambient motion shown while there is nothing to listen to.
            let ambient =
                (0.2 + 0.17 * (x * 9.0 + t * 0.9).sin() * (x * 23.0 - t * 0.6).sin() + 0.1 * (x * 3.0 - t * 0.4).sin())
                    * (1.0 - 0.45 * x);
            let v = self.shaped[i] * (1.0 - idle) + ambient.max(0.0) * idle;

            let s = &mut self.smooth[i];
            *s += (v - *s) * if v > *s { up } else { down };
            let f = &mut self.fast[i];
            *f += (v - *f) * if v > *f { up_fast } else { down_fast };

            let s = self.smooth[i];
            if s >= self.peaks[i] {
                self.peaks[i] = s;
                self.peak_hold[i] = 0.3;
                self.peak_vel[i] = 0.0;
            } else if self.peak_hold[i] > 0.0 {
                self.peak_hold[i] -= dt;
            } else {
                self.peak_vel[i] += dt * 1.8;
                self.peaks[i] = (self.peaks[i] - self.peak_vel[i] * dt).max(s);
            }

            self.tex[i] = byte(s);
            self.tex[BINS * 2 + i] = byte(self.peaks[i]);
            self.tex[BINS * 3 + i] = byte(self.fast[i]);
        }

        // ---- waveform -------------------------------------------------------
        // Start on a rising zero crossing so the scope holds still.
        let span = BINS * 2;
        let mut start = 0;
        for i in 1..n.saturating_sub(span + 1) {
            if self.samples[i - 1] < 0.0 && self.samples[i] >= 0.0 {
                start = i;
                break;
            }
        }
        let mut peak = 0.0f32;
        let mut sum_sq = 0.0f32;
        for s in &self.samples[start..start + span] {
            peak = peak.max(s.abs());
            sum_sq += s * s;
        }
        let rms = (sum_sq / span as f32).sqrt();
        self.wave_agc = peak.max(self.wave_agc * (-dt * 0.3).exp());
        let wave_gain = 0.85 / self.wave_agc.max(0.03);
        for i in 0..BINS {
            let s = (self.samples[start + i * 2] + self.samples[start + i * 2 + 1]) * 0.5;
            let x = i as f32 / BINS as f32 * TAU;
            let ambient = 0.22 * (x * 2.0 + t * 1.2).sin() + 0.1 * (x * 5.0 - t * 1.9).sin();
            let v = s * wave_gain * (1.0 - idle) + ambient * idle;
            self.tex[BINS + i] = byte(0.5 + 0.5 * v);
        }

        // ---- band energies --------------------------------------------------
        let band = |lo: f32, hi: f32, src: &[f32]| {
            let (a, b) = (bin_of(lo), bin_of(hi).max(bin_of(lo) + 1));
            let slice = &src[a..b.min(BINS)];
            let avg = slice.iter().sum::<f32>() / slice.len() as f32;
            let max = slice.iter().cloned().fold(0.0, f32::max);
            avg * 0.65 + max * 0.35
        };
        let targets = [
            band(30.0, 150.0, &self.fast),
            band(300.0, 2500.0, &self.fast),
            band(3500.0, 14000.0, &self.fast),
            (rms * wave_gain * 2.2).min(1.0) * (1.0 - idle) + 0.2 * idle,
        ];
        let up = 1.0 - (-dt * 35.0).exp();
        let down = 1.0 - (-dt * 7.0).exp();
        let settle = 1.0 - (-dt / 1.5).exp();
        let react = self.reactivity.clamp(0.0, 1.0);
        let outs = [&mut self.bass, &mut self.mid, &mut self.treb, &mut self.level];
        for (i, (out, target)) in outs.into_iter().zip(targets).enumerate() {
            let cur = &mut self.bands[i];
            *cur += (target - *cur) * if target > *cur { up } else { down };
            let calm = &mut self.bands_calm[i];
            *calm += (*cur - *calm) * settle;
            // What a preset sees swings around the recent average, by as
            // much as it is asked to react.
            *out = *calm + (*cur - *calm) * react;
        }
        self.phase += dt * (0.3 + 1.1 * self.level + 1.4 * self.bass * self.bass);

        // ---- spectrogram history --------------------------------------------
        self.hist_acc += dt * HIST_RATE;
        let mut pushes = 0;
        while self.hist_acc >= 1.0 {
            self.hist_acc -= 1.0;
            if pushes < 4 {
                self.hist_row = (self.hist_row + 1) % HIST_H;
                let row = &mut self.hist[self.hist_row * HIST_W..(self.hist_row + 1) * HIST_W];
                let per = BINS / HIST_W;
                for (j, out) in row.iter_mut().enumerate() {
                    let sum: f32 = self.smooth[j * per..(j + 1) * per].iter().sum();
                    *out = byte(sum / per as f32);
                }
                pushes += 1;
            }
        }
        if pushes > 0 {
            self.hist_dirty = true;
        }
        // "Now" sits between the two newest rows so the scroll is continuous.
        self.hist_head = self.hist_row as f32 - 1.0 + self.hist_acc.min(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(tap: &Tap, analysis: &mut Analysis, seconds: f32, f: impl Fn(f32) -> f32) {
        let rate = 48_000.0;
        let frames = (seconds * 60.0) as usize;
        let mut t = 0usize;
        for _ in 0..frames {
            let chunk: Vec<f32> = (0..800).map(|i| f((t + i) as f32 / rate)).collect();
            t += 800;
            tap.push_interleaved(&chunk, 1);
            analysis.update(Some(tap), 1.0 / 60.0);
        }
    }

    #[test]
    fn a_tone_lights_up_its_band() {
        let tap = Tap::new();
        let mut a = Analysis::new();
        feed(&tap, &mut a, 1.0, |t| 0.5 * (TAU * 80.0 * t).sin());
        assert!(a.bass > 0.5, "bass {}", a.bass);
        assert!(a.treb < 0.1, "treb {}", a.treb);
        assert!(a.idle < 0.05);
        let peak_bin = (0..BINS).max_by_key(|&i| a.tex[i]).unwrap();
        assert!((peak_bin as i32 - bin_of(80.0) as i32).abs() < 12, "peak at {peak_bin}");
    }

    #[test]
    fn kicks_register_as_beats() {
        let tap = Tap::new();
        let mut a = Analysis::new();
        // 120 BPM kick: a decaying 60 Hz thump every half second, with a
        // few milliseconds of attack so that it has no click.
        feed(&tap, &mut a, 4.0, |t| {
            let p = t % 0.5;
            0.8 * (TAU * 60.0 * p).sin() * (-p * 14.0).exp() * smoothstep(0.0, 0.008, p)
        });
        assert!((6.0..=9.0).contains(&a.beat.count), "beats {}", a.beat.count);
        // The kick is a low onset too, and nothing else is.
        let [low, mid, high] = &a.onsets;
        assert!((6.0..=9.0).contains(&low.count), "low onsets {}", low.count);
        assert_eq!((mid.count, high.count), (0.0, 0.0));
        // Ages run newest first, half a second apart.
        let ages = a.beat.ages;
        assert!(ages[0] < 0.5 && ages[0] < ages[1] && ages[1] < ages[2], "ages {ages:?}");
        assert!((ages[1] - ages[0] - 0.5).abs() < 0.05, "ages {ages:?}");
    }

    /// White noise from a small linear congruential generator, -1..1.
    fn noise(i: u32) -> f32 {
        let x = i.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
        let x = ((x >> ((x >> 28) + 4)) ^ x).wrapping_mul(277_803_737);
        ((x >> 22) ^ x) as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    #[test]
    fn hats_are_high_onsets_only() {
        let tap = Tap::new();
        let mut a = Analysis::new();
        // A closed hi-hat every quarter second: a short burst of noise with
        // everything but the top differenced away (24 dB per octave).
        feed(&tap, &mut a, 4.0, |t| {
            let i = (t * 48_000.0).round() as u32;
            let n = |back: u32| noise(i.wrapping_sub(back));
            let p = t % 0.25;
            0.06 * (n(0) - 4.0 * n(1) + 6.0 * n(2) - 4.0 * n(3) + n(4)) * (-p * 60.0).exp()
        });
        let [low, mid, high] = &a.onsets;
        assert!((12.0..=17.0).contains(&high.count), "high onsets {}", high.count);
        assert_eq!(low.count, 0.0);
        assert_eq!(a.beat.count, 0.0);
        // The very first burst arrives before the level of the mix is known.
        assert!(mid.count <= 1.0, "mid onsets {}", mid.count);
        assert!(a.centroid > 0.7, "centroid {}", a.centroid);
    }

    /// A snare: a burst of noise through the mids and the top, over a short
    /// thump of body.
    fn snare_drum(t: f32, every: f32) -> f32 {
        let i = (t * 48_000.0).round() as u32;
        let p = t % every;
        0.35 * noise(i) * (-p * 28.0).exp() + 0.25 * (TAU * 190.0 * p).sin() * (-p * 40.0).exp()
    }

    #[test]
    fn a_snare_is_a_snare_and_nothing_else() {
        let tap = Tap::new();
        let mut a = Analysis::new();
        feed(&tap, &mut a, 4.0, |t| snare_drum(t, 0.5));
        let [kick, snare, hat] = &a.onsets;
        assert!((7.0..=8.0).contains(&snare.count), "snares {}", snare.count);
        // Its sizzle is not a hat, its body is not a kick, and it is no note.
        assert!(hat.count <= 1.0, "hats {}", hat.count);
        // (The very first one arrives before the level of the mix is known.)
        assert!(kick.count <= 1.0, "kicks {}", kick.count);
        assert!(a.note.count <= 1.0, "notes {}", a.note.count);
        // Hits of equal weight are reported at equal strength.
        assert!(snare.hits.iter().all(|&h| h > 0.8), "hits {:?}", snare.hits);
    }

    #[test]
    fn a_plucked_note_is_a_note_not_a_drum() {
        let tap = Tap::new();
        let mut a = Analysis::new();
        // A plucked string twice a second, climbing an octave over the run.
        let pluck = |t: f32| {
            let p = t % 0.5;
            let hz = 330.0 * 2f32.powf((t / 0.5).floor() / 8.0);
            (1..=4)
                .map(|n| 0.3 / n as f32 * (TAU * hz * n as f32 * p).sin())
                .sum::<f32>()
                * (-p * 7.0).exp()
                * smoothstep(0.0, 0.004, p)
        };
        feed(&tap, &mut a, 4.0, pluck);
        assert!((6.0..=8.0).contains(&a.note.count), "notes {}", a.note.count);
        let [kick, snare, hat] = &a.onsets;
        assert_eq!((kick.count, snare.count, hat.count), (0.0, 0.0, 0.0));
        assert!(a.bassline < 0.2, "bass line {}", a.bassline);
        // The melody swells with each note and falls away as it dies.
        let mut loudest = 0.0f32;
        for i in 0..30 {
            feed(&tap, &mut a, 1.0 / 60.0, |t| pluck(t + i as f32 / 60.0));
            loudest = loudest.max(a.melody);
        }
        assert!(loudest > 0.3, "melody at most {loudest}");
        assert!(a.melody < loudest * 0.8, "melody {} after {loudest}", a.melody);
        // It ended an octave above where it began.
        assert!(a.pitch > 0.4, "pitch {}", a.pitch);
    }

    #[test]
    fn the_bass_line_is_what_the_kicks_leave() {
        let kick = |t: f32| {
            let p = t % 0.5;
            0.8 * (TAU * 60.0 * p).sin() * (-p * 14.0).exp() * smoothstep(0.0, 0.008, p)
        };
        let tap = Tap::new();
        let mut a = Analysis::new();
        feed(&tap, &mut a, 3.0, kick);
        let alone = a.bassline;
        // The same kicks over a held bass note.
        feed(&tap, &mut a, 3.0, |t| kick(t) + 0.3 * (TAU * 82.4 * t).sin());
        assert!(alone < 0.25, "bass line under kicks alone {alone}");
        assert!(a.bassline > 0.6, "bass line with a bass note {}", a.bassline);
        assert!(
            (5.0..=7.0).contains(&(a.onsets[0].count - 6.0)),
            "kicks {}",
            a.onsets[0].count
        );
    }

    #[test]
    fn low_reactivity_calms_the_signals() {
        // A kick on every beat with a hat on every sixteenth.
        let groove = |t: f32| {
            let i = (t * 48_000.0).round() as u32;
            let n = |back: u32| noise(i.wrapping_sub(back));
            let (k, h) = (t % 0.5, t % 0.125);
            0.7 * (TAU * 60.0 * k).sin() * (-k * 14.0).exp() * smoothstep(0.0, 0.008, k)
                + 0.03 * (n(0) - 4.0 * n(1) + 6.0 * n(2) - 4.0 * n(3) + n(4)) * (-h * 70.0).exp()
        };
        let run = |reactivity: f32| {
            let tap = Tap::new();
            let mut a = Analysis::new();
            a.reactivity = reactivity;
            feed(&tap, &mut a, 4.0, groove);
            // The swing of the bass over two more beats.
            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
            for _ in 0..60 {
                feed(&tap, &mut a, 1.0 / 60.0, |t| groove(t + 4.0));
                lo = lo.min(a.bass);
                hi = hi.max(a.bass);
            }
            (hi - lo, a.beat.count, a.onsets[2].count)
        };
        let (swing, beats, hats) = run(1.0);
        let (calm_swing, calm_beats, calm_hats) = run(0.3);
        assert!(calm_swing < swing * 0.5, "bass swing {calm_swing} vs {swing}");
        // Every kick still counts, but only every other hat.
        assert_eq!(beats, calm_beats);
        assert!(hats >= 18.0, "hats {hats}");
        assert!(calm_hats < hats * 0.7, "hats {calm_hats} vs {hats}");
        assert!(calm_hats >= beats, "hats {calm_hats}, beats {beats}");
    }

    #[test]
    fn noise_has_no_pitch() {
        let tap = Tap::new();
        let mut a = Analysis::new();
        feed(&tap, &mut a, 3.0, |t| 0.3 * noise((t * 48_000.0).round() as u32));
        assert!(a.chroma.iter().all(|&c| c < 0.3), "chroma {:?}", a.chroma);
        assert!(a.tonal < 0.25, "tonal {}", a.tonal);
        // Nor does steady noise have onsets, once it has started.
        let before: Vec<f32> = a.onsets.iter().map(|o| o.count).collect();
        feed(&tap, &mut a, 3.0, |t| {
            0.3 * noise((t * 48_000.0).round() as u32 + 144_000)
        });
        let after: Vec<f32> = a.onsets.iter().map(|o| o.count).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn a_note_lights_its_pitch_class() {
        let tone = |f: f32| move |t: f32| 0.4 * (TAU * f * t).sin();
        let tap = Tap::new();
        let mut a = Analysis::new();
        // A4, then the A two octaves above it: the same class.
        for freq in [440.0, 1760.0] {
            feed(&tap, &mut a, 2.0, tone(freq));
            assert!(a.chroma[9] > 0.9, "A at {freq} Hz: {:?}", a.chroma);
            for (i, &c) in a.chroma.iter().enumerate() {
                assert!(i == 9 || c < 0.05, "class {i} at {freq} Hz: {:?}", a.chroma);
            }
            assert!(a.tonal > 0.9, "tonal {}", a.tonal);
        }
        // The chroma row of the texture carries it, in blocks.
        let row = &a.tex[BINS * 4..];
        assert!(row[9 * CHROMA_TEXELS] > 230 && row[10 * CHROMA_TEXELS - 1] > 230);
        assert!(row[9 * CHROMA_TEXELS - 1] < 13 && row[10 * CHROMA_TEXELS] < 13);

        // The centroid follows the pitch.
        let high = a.centroid;
        feed(&tap, &mut a, 1.0, tone(110.0));
        assert!(a.centroid < high - 0.2, "centroid {} after {high}", a.centroid);
    }

    #[test]
    fn harmony_moves_by_fifths() {
        let chord = |notes: [f32; 3]| move |t: f32| notes.iter().map(|f| 0.25 * (TAU * f * t).sin()).sum::<f32>();
        let tap = Tap::new();
        let mut a = Analysis::new();
        // C major, then G major (a fifth up), then F# major (as far as it gets).
        feed(&tap, &mut a, 2.0, chord([261.63, 329.63, 392.0]));
        let c = a.harmony;
        assert!(
            a.chroma[0] > 0.6 && a.chroma[4] > 0.6 && a.chroma[7] > 0.6,
            "{:?}",
            a.chroma
        );
        assert!((0.4..0.8).contains(&a.tonal), "tonal {}", a.tonal);
        feed(&tap, &mut a, 2.0, chord([392.0, 493.88, 587.33]));
        let g = a.harmony;
        feed(&tap, &mut a, 2.0, chord([369.99, 466.16, 554.37]));
        let f_sharp = a.harmony;
        let around = |x: f32, y: f32| ((x - y).rem_euclid(1.0)).min((y - x).rem_euclid(1.0));
        assert!((around(c, g) - 1.0 / 12.0).abs() < 0.02, "C {c} G {g}");
        assert!((around(c, f_sharp) - 0.5).abs() < 0.02, "C {c} F# {f_sharp}");
    }

    #[test]
    fn silence_falls_back_to_the_ambient_animation() {
        let mut a = Analysis::new();
        for _ in 0..240 {
            a.update(None, 1.0 / 60.0);
        }
        assert!(a.idle > 0.9);
        assert!(a.tex[..BINS].iter().any(|&v| v > 20));
        assert_eq!(a.beat.count, 0.0);
        assert_eq!(a.beat.ages, [AGE_CAP; 4]);
        // The idle animation gives the new signals something to do as well.
        assert!(a.chroma.iter().any(|&c| c > 0.3), "chroma {:?}", a.chroma);
        assert!(a.tonal > 0.3, "tonal {}", a.tonal);
        assert!((0.2..0.6).contains(&a.centroid), "centroid {}", a.centroid);
    }
}
