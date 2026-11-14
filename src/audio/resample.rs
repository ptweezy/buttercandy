//! Polyphase windowed-sinc resampler for interleaved stereo.
//!
//! Decoded audio arrives at the file's sample rate and has to leave at the
//! output device's. A 32-tap Kaiser-windowed sinc with 256 interpolated phases
//! is transparent for playback and costs a few million multiply-adds a second.

const TAPS: usize = 32;
const PHASES: usize = 256;
const HALF: usize = TAPS / 2;

/// Zeroth-order modified Bessel function, for the Kaiser window.
fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
    while term > 1e-12 * sum {
        term *= (x / (2.0 * k)).powi(2);
        sum += term;
        k += 1.0;
    }
    sum
}

pub struct Resampler {
    /// Input frames consumed per output frame.
    step: f64,
    /// Read position, in frames, into `hist`.
    pos: f64,
    /// Pending interleaved stereo input, primed with `HALF` frames of silence.
    hist: Vec<f32>,
    /// `(PHASES + 1) x TAPS` filter bank.
    table: Vec<f32>,
    passthrough: bool,
}

impl Resampler {
    pub fn new(in_rate: u32, out_rate: u32) -> Self {
        let passthrough = in_rate == out_rate;
        let step = in_rate as f64 / out_rate as f64;
        // When downsampling, lower the cutoff to the new Nyquist.
        let cutoff = (1.0 / step).min(1.0) * 0.96;
        let beta = 8.6;
        let i0_beta = bessel_i0(beta);
        let mut table = vec![0.0f32; (PHASES + 1) * TAPS];
        for p in 0..=PHASES {
            let frac = p as f64 / PHASES as f64;
            let row = &mut table[p * TAPS..(p + 1) * TAPS];
            let mut sum = 0.0;
            for (t, coef) in row.iter_mut().enumerate() {
                // Distance from this tap to the fractional read position.
                let x = t as f64 - (HALF as f64 - 1.0) - frac;
                let r = x / HALF as f64;
                let window = if r.abs() < 1.0 {
                    bessel_i0(beta * (1.0 - r * r).sqrt()) / i0_beta
                } else {
                    0.0
                };
                let sinc = if x.abs() < 1e-9 {
                    1.0
                } else {
                    let a = std::f64::consts::PI * x * cutoff;
                    a.sin() / a
                };
                let v = cutoff * sinc * window;
                sum += v;
                *coef = v as f32;
            }
            // Unity gain at DC for every phase.
            for coef in row.iter_mut() {
                *coef = (*coef as f64 / sum) as f32;
            }
        }
        let mut r = Self {
            step,
            pos: 0.0,
            hist: Vec::new(),
            table,
            passthrough,
        };
        r.reset();
        r
    }

    /// Forget all history, e.g. after a seek.
    pub fn reset(&mut self) {
        self.hist.clear();
        self.hist.resize(HALF * 2, 0.0);
        // The first output frame lines up exactly with the first input frame.
        self.pos = HALF as f64;
    }

    /// Consume interleaved stereo `input`, appending resampled frames to `out`.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.passthrough {
            out.extend_from_slice(input);
            return;
        }
        self.hist.extend_from_slice(input);
        let frames = self.hist.len() / 2;
        // An output frame at `pos` reads input frames [base - HALF + 1, base + HALF].
        while (self.pos as usize) + HALF < frames {
            let base = self.pos as usize;
            let frac = self.pos - base as f64;
            let ph = frac * PHASES as f64;
            let p = ph as usize;
            let w = (ph - p as f64) as f32;
            let row0 = &self.table[p * TAPS..(p + 1) * TAPS];
            let row1 = &self.table[(p + 1) * TAPS..(p + 2) * TAPS];
            let first = base + 1 - HALF;
            let window = &self.hist[first * 2..(first + TAPS) * 2];
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for t in 0..TAPS {
                let c = row0[t] + (row1[t] - row0[t]) * w;
                l += window[t * 2] * c;
                r += window[t * 2 + 1] * c;
            }
            out.push(l);
            out.push(r);
            self.pos += self.step;
        }
        // Drop input that no future output frame can reach.
        let keep_from = (self.pos as usize + 1).saturating_sub(HALF).min(frames);
        self.hist.drain(..keep_from * 2);
        self.pos -= keep_from as f64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn run(in_rate: u32, out_rate: u32, freq: f32) -> Vec<f32> {
        let mut rs = Resampler::new(in_rate, out_rate);
        let input: Vec<f32> = (0..in_rate as usize)
            .flat_map(|i| {
                let s = (TAU * freq * i as f32 / in_rate as f32).sin();
                [s, -s]
            })
            .collect();
        let mut out = Vec::new();
        // Feed in uneven chunks to exercise the history bookkeeping.
        for chunk in input.chunks(2 * 777) {
            rs.process(chunk, &mut out);
        }
        out
    }

    #[test]
    fn preserves_a_tone_when_changing_rate() {
        for (a, b) in [(44_100, 48_000), (48_000, 44_100), (96_000, 48_000), (22_050, 48_000)] {
            let out = run(a, b, 1000.0);
            let frames = out.len() / 2;
            assert!((frames as i64 - b as i64).abs() < 64, "{a}->{b}: {frames} frames");
            // Output frame i should be the input tone sampled at i / out_rate.
            let mut err = 0.0f32;
            for i in 2000..frames - 2000 {
                let ideal = (TAU * 1000.0 * (i as f64 / b as f64) as f32).sin();
                err = err.max((out[i * 2] - ideal).abs()).max((out[i * 2 + 1] + ideal).abs());
            }
            assert!(err < 0.01, "{a}->{b}: max error {err}");
        }
    }

    #[test]
    fn passthrough_is_exact() {
        let out = run(48_000, 48_000, 440.0);
        assert_eq!(out.len(), 96_000);
    }
}
