//! File playback: a decoder thread streams resampled stereo into a lock-free
//! ring that the output callback drains.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::time::{Duration, Instant};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, MetadataRevision, StandardTagKey};
use symphonia::core::probe::Hint;
use symphonia::core::units::{Time, TimeBase};

use super::AudioEvent;
use super::resample::Resampler;
use super::ring::SpscRing;

/// Size of the decoded cover art handed to the UI.
pub const ART_SIZE: u32 = 256;

#[derive(Default, Clone)]
pub struct TrackInfo {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<f64>,
}

/// State shared between the decoder thread and the audio callback.
pub struct PlayerShared {
    pub ring: SpscRing,
    /// Decoder asks the callback to drop everything queued (load / seek).
    pub flush: AtomicBool,
    /// Id of the track whose final samples the decoder has queued (0 = none).
    pub eof: AtomicU64,
    /// Id of the track the callback has played to the end (0 = none).
    pub finished: AtomicU64,
    /// Frames played since the last flush.
    pub played: AtomicU64,
    /// Stream time (f64 bits) at the last flush.
    pub base: AtomicU64,
    /// Track length in seconds (f64 bits), 0 when unknown.
    pub duration: AtomicU64,
    /// Output device sample rate.
    pub rate: AtomicU32,
}

impl PlayerShared {
    pub fn new() -> Self {
        Self {
            // About 1.4 s of stereo at 48 kHz.
            ring: SpscRing::new(1 << 17),
            flush: AtomicBool::new(false),
            eof: AtomicU64::new(0),
            finished: AtomicU64::new(0),
            played: AtomicU64::new(0),
            base: AtomicU64::new(0),
            duration: AtomicU64::new(0),
            rate: AtomicU32::new(48_000),
        }
    }

    pub fn position(&self) -> f64 {
        let rate = self.rate.load(Ordering::Relaxed).max(1) as f64;
        f64::from_bits(self.base.load(Ordering::Relaxed)) + self.played.load(Ordering::Relaxed) as f64 / rate
    }

    pub fn duration(&self) -> f64 {
        f64::from_bits(self.duration.load(Ordering::Relaxed))
    }
}

pub enum Cmd {
    Load { id: u64, path: PathBuf },
    Seek(f64),
    Stop,
}

struct Stream {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    resampler: Resampler,
    in_rate: u32,
    out_rate: u32,
    sample_buf: Option<SampleBuffer<f32>>,
    stereo: Vec<f32>,
    /// Input frames still to discard after a seek landed early.
    skip: u64,
    done: bool,
}

fn read_tags(rev: &MetadataRevision, info: &mut TrackInfo, art: &mut Option<Vec<u8>>) {
    for tag in rev.tags() {
        let value = tag.value.to_string();
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        let slot = match tag.std_key {
            Some(StandardTagKey::TrackTitle) => &mut info.title,
            Some(StandardTagKey::Artist) => &mut info.artist,
            Some(StandardTagKey::Album) => &mut info.album,
            _ => continue,
        };
        slot.get_or_insert_with(|| value.to_string());
    }
    if art.is_none()
        && let Some(visual) = rev.visuals().first()
    {
        *art = Some(visual.data.to_vec());
    }
}

struct Opened {
    format: Box<dyn FormatReader>,
    track_id: u32,
    params: symphonia::core::codecs::CodecParameters,
    info: TrackInfo,
    art: Option<Vec<u8>>,
}

fn open(path: &Path) -> Result<Opened, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let format_opts = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let mut probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &MetadataOptions::default())
        .map_err(|e| format!("unsupported file ({e})"))?;

    let mut info = TrackInfo::default();
    let mut art = None;
    // Tags can live in the container or in a wrapper read while probing (ID3).
    if let Some(rev) = probed.format.metadata().current() {
        read_tags(rev, &mut info, &mut art);
    }
    if let Some(mut meta) = probed.metadata.get()
        && let Some(rev) = meta.skip_to_latest()
    {
        read_tags(rev, &mut info, &mut art);
    }

    let track = probed
        .format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("no audio track")?;
    let params = track.codec_params.clone();
    let track_id = track.id;
    if let (Some(frames), Some(rate)) = (params.n_frames, params.sample_rate) {
        info.duration = Some(frames as f64 / rate as f64);
    }
    Ok(Opened {
        format: probed.format,
        track_id,
        params,
        info,
        art,
    })
}

/// Read tags and duration without decoding any audio.
pub fn probe(path: &Path) -> Option<TrackInfo> {
    open(path).ok().map(|o| o.info)
}

fn decode_art(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    let img = img.resize_to_fill(ART_SIZE, ART_SIZE, image::imageops::FilterType::Triangle);
    Some(img.to_rgba8().into_raw())
}

impl Stream {
    fn new(opened: Opened, out_rate: u32) -> Result<Self, String> {
        let decoder = symphonia::default::get_codecs()
            .make(&opened.params, &DecoderOptions::default())
            .map_err(|e| format!("unsupported codec ({e})"))?;
        let in_rate = opened.params.sample_rate.unwrap_or(44_100);
        Ok(Self {
            format: opened.format,
            decoder,
            track_id: opened.track_id,
            time_base: opened.params.time_base,
            resampler: Resampler::new(in_rate, out_rate),
            in_rate,
            out_rate,
            sample_buf: None,
            stereo: Vec::new(),
            skip: 0,
            done: false,
        })
    }

    /// Decode one packet, appending output-rate stereo to `out`.
    /// Returns `Ok(false)` at end of stream.
    fn pump(&mut self, out_rate: u32, out: &mut Vec<f32>) -> Result<bool, String> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(false),
                Err(SymError::ResetRequired) => return Ok(false),
                Err(e) => return Err(e.to_string()),
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                // A corrupt packet is skipped, not fatal.
                Err(SymError::DecodeError(_)) | Err(SymError::IoError(_)) => continue,
                Err(e) => return Err(e.to_string()),
            };
            let spec = *decoded.spec();
            if spec.rate != self.in_rate || out_rate != self.out_rate {
                self.in_rate = spec.rate;
                self.out_rate = out_rate;
                self.resampler = Resampler::new(spec.rate, out_rate);
            }
            let channels = spec.channels.count().max(1);
            let needed = decoded.capacity() * channels;
            if self.sample_buf.as_ref().is_none_or(|b| b.capacity() < needed) {
                self.sample_buf = Some(SampleBuffer::new(decoded.capacity() as u64, spec));
            }
            let buf = self.sample_buf.as_mut().unwrap();
            buf.copy_interleaved_ref(decoded);
            let samples = buf.samples();

            self.stereo.clear();
            match channels {
                1 => self.stereo.extend(samples.iter().flat_map(|&s| [s, s])),
                2 => self.stereo.extend_from_slice(samples),
                _ => self
                    .stereo
                    .extend(samples.chunks_exact(channels).flat_map(|f| [f[0], f[1]])),
            }
            if self.skip > 0 {
                let drop = (self.skip as usize).min(self.stereo.len() / 2);
                self.stereo.drain(..drop * 2);
                self.skip -= drop as u64;
                if self.stereo.is_empty() {
                    continue;
                }
            }
            self.resampler.process(&self.stereo, out);
            return Ok(true);
        }
    }

    fn seek(&mut self, seconds: f64) -> Option<f64> {
        let to = SeekTo::Time {
            time: Time::from(seconds.max(0.0)),
            track_id: Some(self.track_id),
        };
        let seeked = self.format.seek(SeekMode::Accurate, to).ok()?;
        self.decoder.reset();
        self.resampler.reset();
        self.done = false;
        self.skip = 0;
        let Some(tb) = self.time_base else { return Some(seconds) };
        let secs = |ts: u64| {
            let t = tb.calc_time(ts);
            t.seconds as f64 + t.frac
        };
        let (actual, required) = (secs(seeked.actual_ts), secs(seeked.required_ts));
        // Readers land on the packet boundary at or before the target; drop
        // the difference so playback resumes exactly where it was asked to.
        self.skip = ((required - actual).max(0.0) * self.in_rate as f64).round() as u64;
        Some(required.max(actual))
    }
}

/// Ask the audio callback to drop queued audio and restart its clock at `base`.
fn flush(shared: &PlayerShared, base: f64) {
    shared.eof.store(0, Ordering::Relaxed);
    shared.finished.store(0, Ordering::Relaxed);
    shared.base.store(base.to_bits(), Ordering::Relaxed);
    shared.flush.store(true, Ordering::Release);
    // The callback runs continuously; give it a moment to acknowledge so the
    // new audio we are about to queue is not the audio it throws away.
    let start = Instant::now();
    while shared.flush.load(Ordering::Acquire) && start.elapsed() < Duration::from_millis(250) {
        std::thread::sleep(Duration::from_millis(1));
    }
}

pub fn run(shared: Arc<PlayerShared>, commands: Receiver<Cmd>, events: Sender<AudioEvent>) {
    let mut current: Option<(u64, Stream)> = None;
    let mut chunk: Vec<f32> = Vec::with_capacity(1 << 15);
    let mut offset = 0;

    loop {
        let idle = current.as_ref().is_none_or(|(_, s)| s.done) && offset >= chunk.len();
        let cmd = if idle {
            match commands.recv_timeout(Duration::from_millis(200)) {
                Ok(cmd) => Some(cmd),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        } else {
            match commands.try_recv() {
                Ok(cmd) => Some(cmd),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => return,
            }
        };

        match cmd {
            Some(Cmd::Load { id, path }) => {
                chunk.clear();
                offset = 0;
                current = None;
                flush(&shared, 0.0);
                let out_rate = shared.rate.load(Ordering::Relaxed);
                let result = open(&path).and_then(|mut opened| {
                    let info = opened.info.clone();
                    let art = opened.art.take();
                    Stream::new(opened, out_rate).map(|s| (s, info, art))
                });
                match result {
                    Ok((stream, info, art)) => {
                        shared
                            .duration
                            .store(info.duration.unwrap_or(0.0).to_bits(), Ordering::Relaxed);
                        current = Some((id, stream));
                        let _ = events.send(AudioEvent::Loaded { id, info });
                        let art = art.as_deref().and_then(decode_art);
                        let _ = events.send(AudioEvent::Art { id, rgba: art });
                    }
                    Err(error) => {
                        let _ = events.send(AudioEvent::Failed { id, error });
                    }
                }
                continue;
            }
            Some(Cmd::Seek(seconds)) => {
                if let Some((_, stream)) = current.as_mut()
                    && let Some(actual) = stream.seek(seconds)
                {
                    chunk.clear();
                    offset = 0;
                    flush(&shared, actual);
                }
                continue;
            }
            Some(Cmd::Stop) => {
                chunk.clear();
                offset = 0;
                current = None;
                flush(&shared, 0.0);
                continue;
            }
            None => {}
        }

        // Push whatever is left of the last decoded chunk.
        if offset < chunk.len() {
            let n = shared.ring.push(&chunk[offset..]);
            offset += n;
            if offset < chunk.len() {
                std::thread::sleep(Duration::from_millis(4));
            }
            continue;
        }

        let Some((id, stream)) = current.as_mut() else { continue };
        if stream.done {
            continue;
        }
        chunk.clear();
        offset = 0;
        let out_rate = shared.rate.load(Ordering::Relaxed);
        match stream.pump(out_rate, &mut chunk) {
            Ok(true) => {}
            Ok(false) => {
                stream.done = true;
                shared.eof.store(*id, Ordering::Release);
            }
            Err(error) => {
                stream.done = true;
                shared.eof.store(*id, Ordering::Release);
                let _ = events.send(AudioEvent::Failed { id: *id, error });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::mpsc;

    const RATE_IN: u32 = 44_100;
    const RATE_OUT: u32 = 48_000;
    const PEAK: f32 = 30_000.0 / 32_768.0;

    /// A mono 16-bit WAV whose samples ramp from zero to `PEAK`, so the value
    /// of a sample says where in the file it came from.
    fn ramp_wav(path: &Path, seconds: f32) {
        let n = (RATE_IN as f32 * seconds) as u32;
        let mut bytes = Vec::with_capacity(44 + n as usize * 2);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + n * 2).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
        bytes.extend_from_slice(&RATE_IN.to_le_bytes());
        bytes.extend_from_slice(&(RATE_IN * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(n * 2).to_le_bytes());
        for i in 0..n {
            bytes.extend_from_slice(&((i as f32 / n as f32 * 30_000.0) as i16).to_le_bytes());
        }
        File::create(path).unwrap().write_all(&bytes).unwrap();
    }

    /// Stand in for the audio callback: honour flush requests, pull samples.
    fn pull(shared: &PlayerShared, out: &mut [f32]) -> usize {
        if shared.flush.load(Ordering::Acquire) {
            shared.ring.clear();
            shared.played.store(0, Ordering::Relaxed);
            shared.flush.store(false, Ordering::Release);
        }
        let n = shared.ring.pop(out);
        shared.played.fetch_add((n / 2) as u64, Ordering::Relaxed);
        n
    }

    /// Keep pulling until `done` says so (given the latest chunk); panics on timeout.
    fn pull_until(shared: &PlayerShared, mut done: impl FnMut(&[f32]) -> bool) {
        let mut buf = [0.0f32; 960];
        let start = Instant::now();
        loop {
            let n = pull(shared, &mut buf);
            if done(&buf[..n]) {
                return;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "timed out waiting for the decoder"
            );
            if n == 0 {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    #[test]
    fn loads_seeks_and_reaches_the_end() {
        let path = std::env::temp_dir().join(format!("buttercandy-test-{}.wav", std::process::id()));
        ramp_wav(&path, 2.0);

        let shared = Arc::new(PlayerShared::new());
        shared.rate.store(RATE_OUT, Ordering::Relaxed);
        let (commands, command_rx) = mpsc::channel();
        let (events, event_rx) = mpsc::channel();
        let thread_shared = shared.clone();
        let decoder = std::thread::spawn(move || run(thread_shared, command_rx, events));

        // Load: playback starts at the beginning of the ramp.
        commands
            .send(Cmd::Load {
                id: 7,
                path: path.clone(),
            })
            .unwrap();
        let mut first = None;
        let mut frames = 0;
        pull_until(&shared, |chunk| {
            if first.is_none() && !chunk.is_empty() {
                first = Some((chunk[0], chunk[1]));
            }
            frames += chunk.len() / 2;
            frames >= RATE_OUT as usize / 10
        });
        let (l, r) = first.unwrap();
        assert!(
            l.abs() < 0.01 && l == r,
            "starts at the start, mono on both channels: {l} {r}"
        );
        match event_rx.recv_timeout(Duration::from_secs(5)).unwrap() {
            AudioEvent::Loaded { id, info } => {
                assert_eq!(id, 7);
                assert!((info.duration.unwrap() - 2.0).abs() < 0.01);
            }
            _ => panic!("expected Loaded first"),
        }
        assert!((shared.position() - frames as f64 / RATE_OUT as f64).abs() < 1e-6);

        // Seek to the middle: queued audio is dropped and the clock restarts there.
        commands.send(Cmd::Seek(1.0)).unwrap();
        let mut after = 0;
        let mut landed = None;
        pull_until(&shared, |chunk| {
            let seeked =
                f64::from_bits(shared.base.load(Ordering::Relaxed)) > 0.5 && !shared.flush.load(Ordering::Acquire);
            if seeked && !chunk.is_empty() {
                landed.get_or_insert(chunk[0]);
                after += chunk.len() / 2;
            }
            landed.is_some()
        });
        let landed = landed.unwrap();
        assert!(
            (landed - 0.5 * PEAK).abs() < 0.01,
            "first sample after seeking to 1.0 s of 2.0 s: {landed}"
        );

        // Play out the rest: about one more second, then end of stream.
        pull_until(&shared, |chunk| {
            after += chunk.len() / 2;
            shared.eof.load(Ordering::Acquire) == 7 && shared.ring.len() == 0
        });
        let expected = RATE_OUT as i64;
        assert!((after as i64 - expected).abs() < 400, "{after} frames after the seek");
        assert!(
            (shared.position() - 2.0).abs() < 0.02,
            "position at the end: {}",
            shared.position()
        );

        drop(commands);
        decoder.join().unwrap();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unreadable_files_fail_cleanly() {
        let path = std::env::temp_dir().join(format!("buttercandy-test-{}.mp3", std::process::id()));
        std::fs::write(&path, b"this is not audio").unwrap();
        let shared = Arc::new(PlayerShared::new());
        let (commands, command_rx) = mpsc::channel();
        let (events, event_rx) = mpsc::channel();
        let thread_shared = shared.clone();
        let decoder = std::thread::spawn(move || run(thread_shared, command_rx, events));
        // Nothing is acknowledging flushes here; the decoder must not hang on that.
        commands
            .send(Cmd::Load {
                id: 3,
                path: path.clone(),
            })
            .unwrap();
        match event_rx.recv_timeout(Duration::from_secs(5)).unwrap() {
            AudioEvent::Failed { id, .. } => assert_eq!(id, 3),
            _ => panic!("expected Failed"),
        }
        assert!(probe(&path).is_none());
        drop(commands);
        decoder.join().unwrap();
        let _ = std::fs::remove_file(&path);
    }
}
