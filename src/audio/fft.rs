//! A small radix-2 FFT, enough for one spectrum per frame.

use std::f32::consts::PI;

pub struct Fft {
    n: usize,
    rev: Vec<u32>,
    cos: Vec<f32>,
    sin: Vec<f32>,
    window: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
}

impl Fft {
    /// `n` must be a power of two.
    pub fn new(n: usize) -> Self {
        assert!(n.is_power_of_two() && n >= 4);
        let bits = n.trailing_zeros();
        let rev = (0..n as u32).map(|i| i.reverse_bits() >> (32 - bits)).collect();
        let half = n / 2;
        let cos = (0..half).map(|k| (2.0 * PI * k as f32 / n as f32).cos()).collect();
        let sin = (0..half).map(|k| (2.0 * PI * k as f32 / n as f32).sin()).collect();
        // Hann window.
        let window = (0..n)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / n as f32).cos())
            .collect();
        Self {
            n,
            rev,
            cos,
            sin,
            window,
            re: vec![0.0; n],
            im: vec![0.0; n],
        }
    }

    pub fn len(&self) -> usize {
        self.n
    }

    /// Windowed magnitude spectrum of `samples` (length `n`) into `out`
    /// (length `n / 2`), scaled so a full-scale sine reads 1.0.
    pub fn magnitudes(&mut self, samples: &[f32], out: &mut [f32]) {
        let n = self.n;
        let (re, im) = (&mut self.re, &mut self.im);
        for i in 0..n {
            let j = self.rev[i] as usize;
            re[j] = samples[i] * self.window[i];
            im[j] = 0.0;
        }
        let mut len = 2;
        while len <= n {
            let half = len / 2;
            let step = n / len;
            for start in (0..n).step_by(len) {
                for k in 0..half {
                    let (c, s) = (self.cos[k * step], self.sin[k * step]);
                    let (a, b) = (start + k, start + k + half);
                    let tr = re[b] * c + im[b] * s;
                    let ti = im[b] * c - re[b] * s;
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            len <<= 1;
        }
        // The Hann window has a coherent gain of 0.5, so a unit sine peaks at n/4.
        let norm = 4.0 / n as f32;
        for (i, o) in out.iter_mut().take(n / 2).enumerate() {
            *o = (re[i] * re[i] + im[i] * im[i]).sqrt() * norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_lands_in_its_bin_at_unit_magnitude() {
        let n = 1024;
        let mut fft = Fft::new(n);
        let bin = 37;
        let samples: Vec<f32> = (0..n)
            .map(|i| (2.0 * PI * bin as f32 * i as f32 / n as f32).sin())
            .collect();
        let mut out = vec![0.0; n / 2];
        fft.magnitudes(&samples, &mut out);
        let peak = out.iter().cloned().fold(0.0, f32::max);
        assert!((out[bin] - 1.0).abs() < 1e-3, "bin magnitude {}", out[bin]);
        assert_eq!(peak, out[bin]);
        assert!(out[bin + 5] < 1e-3);
    }
}
