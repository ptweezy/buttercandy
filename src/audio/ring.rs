//! Lock-free buffers shared between the audio threads and everything else.
//!
//! Samples are stored as `AtomicU32` bit patterns, which keeps both buffers
//! entirely in safe Rust: the audio callback never blocks, never allocates and
//! never takes a lock.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

fn atomic_buf(len: usize) -> Box<[AtomicU32]> {
    (0..len).map(|_| AtomicU32::new(0)).collect()
}

/// A wait-free "most recent N samples" window of mono audio.
///
/// Whichever thread is producing sound (playback, loopback capture, microphone)
/// writes here; the render thread reads the newest window once per frame. A
/// reader can observe a partially updated window, which is harmless for a
/// visualizer and is the price of never stalling the audio thread.
pub struct Tap {
    buf: Box<[AtomicU32]>,
    mask: usize,
    written: AtomicU64,
    rate: AtomicU32,
}

impl Tap {
    const LEN: usize = 1 << 16;

    pub fn new() -> Self {
        Self {
            buf: atomic_buf(Self::LEN),
            mask: Self::LEN - 1,
            written: AtomicU64::new(0),
            rate: AtomicU32::new(48_000),
        }
    }

    pub fn set_rate(&self, hz: u32) {
        self.rate.store(hz.max(1), Ordering::Relaxed);
    }

    pub fn rate(&self) -> u32 {
        self.rate.load(Ordering::Relaxed)
    }

    /// Push interleaved frames, downmixed to mono. Single writer at a time.
    pub fn push_interleaved(&self, data: &[f32], channels: usize) {
        let channels = channels.max(1);
        let mut w = self.written.load(Ordering::Relaxed);
        for frame in data.chunks_exact(channels) {
            let s = if channels == 1 {
                frame[0]
            } else {
                (frame[0] + frame[1]) * 0.5
            };
            self.buf[w as usize & self.mask].store(s.to_bits(), Ordering::Relaxed);
            w = w.wrapping_add(1);
        }
        self.written.store(w, Ordering::Release);
    }

    /// Copy the newest `out.len()` samples, oldest first. Returns the running
    /// sample count so callers can tell whether anything new has arrived.
    pub fn latest(&self, out: &mut [f32]) -> u64 {
        let w = self.written.load(Ordering::Acquire);
        let n = out.len() as u64;
        for (i, o) in out.iter_mut().enumerate() {
            let idx = w.wrapping_sub(n - i as u64);
            *o = f32::from_bits(self.buf[idx as usize & self.mask].load(Ordering::Relaxed));
        }
        w
    }
}

/// Single-producer / single-consumer ring of `f32` samples.
pub struct SpscRing {
    buf: Box<[AtomicU32]>,
    mask: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl SpscRing {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.next_power_of_two();
        Self {
            buf: atomic_buf(cap),
            mask: cap - 1,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    pub fn len(&self) -> usize {
        self.head
            .load(Ordering::Acquire)
            .wrapping_sub(self.tail.load(Ordering::Acquire))
    }

    /// Producer side. Returns how many samples were accepted.
    pub fn push(&self, data: &[f32]) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        let n = data.len().min(self.buf.len() - head.wrapping_sub(tail));
        for (i, s) in data[..n].iter().enumerate() {
            self.buf[head.wrapping_add(i) & self.mask].store(s.to_bits(), Ordering::Relaxed);
        }
        self.head.store(head.wrapping_add(n), Ordering::Release);
        n
    }

    /// Consumer side. Returns how many samples were written to `out`.
    pub fn pop(&self, out: &mut [f32]) -> usize {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        let n = out.len().min(head.wrapping_sub(tail));
        for (i, o) in out[..n].iter_mut().enumerate() {
            *o = f32::from_bits(self.buf[tail.wrapping_add(i) & self.mask].load(Ordering::Relaxed));
        }
        self.tail.store(tail.wrapping_add(n), Ordering::Release);
        n
    }

    /// Consumer side: drop everything currently queued.
    pub fn clear(&self) {
        self.tail.store(self.head.load(Ordering::Acquire), Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_round_trips_across_the_wrap() {
        let ring = SpscRing::new(8);
        let mut out = [0.0; 8];
        for round in 0..10 {
            let data: Vec<f32> = (0..5).map(|i| (round * 5 + i) as f32).collect();
            assert_eq!(ring.push(&data), 5);
            assert_eq!(ring.pop(&mut out), 5);
            assert_eq!(&out[..5], &data[..]);
        }
        assert_eq!(ring.push(&[1.0; 12]), 8);
        assert_eq!(ring.len(), 8);
        ring.clear();
        assert_eq!(ring.len(), 0);
    }

    #[test]
    fn tap_returns_newest_window() {
        let tap = Tap::new();
        let data: Vec<f32> = (0..100).map(|i| i as f32).collect();
        tap.push_interleaved(&data, 1);
        let mut out = [0.0; 4];
        assert_eq!(tap.latest(&mut out), 100);
        assert_eq!(out, [96.0, 97.0, 98.0, 99.0]);
        tap.push_interleaved(&[1.0, 3.0, 5.0, 7.0], 2);
        tap.latest(&mut out);
        assert_eq!(out, [98.0, 99.0, 2.0, 6.0]);
    }
}
