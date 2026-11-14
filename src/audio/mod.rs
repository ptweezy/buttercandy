//! Audio I/O: file playback, the built-in demo synth, system-audio loopback
//! and microphone capture. Whatever is sounding is mirrored into a [`Tap`]
//! for the analyser.

pub mod analysis;
pub mod demo;
mod fft;
pub mod player;
mod resample;
pub mod ring;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{ErrorKind, FromSample, Sample, SampleFormat, SizedSample, StreamConfig};

use demo::DemoSynth;
use player::{Cmd, PlayerShared, TrackInfo};
use ring::Tap;

pub enum AudioEvent {
    /// A file opened and started playing.
    Loaded { id: u64, info: TrackInfo },
    /// Cover art for the playing file (`ART_SIZE` square RGBA), if it has any.
    Art { id: u64, rgba: Option<Vec<u8>> },
    /// A file could not be opened or decoded.
    Failed { id: u64, error: String },
    /// A file played through to its end.
    Ended { id: u64 },
    /// Tags for a queued file, read in the background.
    Meta { id: u64, info: TrackInfo },
    /// An audio device went away.
    DeviceLost(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    None,
    Player,
    Demo,
    System,
    Mic,
}

const MODE_SILENT: u8 = 0;
const MODE_PLAYER: u8 = 1;
const MODE_DEMO: u8 = 2;

/// State the output callback reads.
struct OutShared {
    mode: AtomicU8,
    paused: AtomicBool,
    volume: AtomicU32,
    demo_restart: AtomicBool,
    failed: AtomicBool,
    player: Arc<PlayerShared>,
    tap: Arc<Tap>,
}

/// Fill `buf` (interleaved stereo) with whatever should be playing right now.
fn render(shared: &OutShared, synth: &mut DemoSynth, buf: &mut [f32]) {
    let p = &shared.player;
    if p.flush.load(Ordering::Acquire) {
        p.ring.clear();
        p.played.store(0, Ordering::Relaxed);
        p.flush.store(false, Ordering::Release);
    }
    let paused = shared.paused.load(Ordering::Relaxed);
    match shared.mode.load(Ordering::Relaxed) {
        MODE_PLAYER if !paused => {
            let n = p.ring.pop(buf);
            buf[n..].fill(0.0);
            p.played.fetch_add((n / 2) as u64, Ordering::Relaxed);
            if n > 0 {
                shared.tap.push_interleaved(buf, 2);
            }
            if n < buf.len() {
                let eof = p.eof.load(Ordering::Acquire);
                if eof != 0 && p.ring.len() == 0 {
                    p.finished.store(eof, Ordering::Release);
                }
            }
        }
        MODE_DEMO if !paused => {
            if shared.demo_restart.swap(false, Ordering::Relaxed) {
                synth.restart();
            }
            synth.render(buf);
            shared.tap.push_interleaved(buf, 2);
        }
        _ => buf.fill(0.0),
    }
}

fn output_stream<T>(
    device: &cpal::Device,
    config: StreamConfig,
    shared: Arc<OutShared>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels.max(1) as usize;
    let mut synth = DemoSynth::new(config.sample_rate);
    let mut scratch = vec![0.0f32; 16384];
    let mut volume = 0.0f32;
    let on_error = shared.clone();
    device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            let frames = data.len() / channels;
            if scratch.len() < frames * 2 {
                scratch.resize(frames * 2, 0.0);
            }
            let buf = &mut scratch[..frames * 2];
            render(&shared, &mut synth, buf);
            let target = f32::from_bits(shared.volume.load(Ordering::Relaxed));
            for (frame, s) in data.chunks_exact_mut(channels).zip(buf.as_chunks::<2>().0) {
                // Glide the gain so volume changes never click.
                volume += (target - volume) * 0.002;
                let (l, r) = (s[0] * volume, s[1] * volume);
                if channels == 1 {
                    frame[0] = T::from_sample((l + r) * 0.5);
                } else {
                    frame[0] = T::from_sample(l);
                    frame[1] = T::from_sample(r);
                    for extra in &mut frame[2..] {
                        *extra = T::from_sample(0.0);
                    }
                }
            }
        },
        move |err: cpal::Error| match err.kind() {
            // cpal follows default-device changes by itself; these are not fatal.
            ErrorKind::DeviceChanged | ErrorKind::Xrun | ErrorKind::RealtimeDenied => {
                log::debug!("audio output: {err}")
            }
            _ => {
                log::warn!("audio output failed: {err}");
                on_error.failed.store(true, Ordering::Relaxed);
            }
        },
        None,
    )
}

fn input_stream<T>(
    device: &cpal::Device,
    config: StreamConfig,
    tap: Arc<Tap>,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels.max(1) as usize;
    let mut scratch: Vec<f32> = Vec::with_capacity(16384);
    device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            scratch.clear();
            scratch.extend(data.iter().map(|&s| f32::from_sample(s)));
            tap.push_interleaved(&scratch, channels);
        },
        move |err: cpal::Error| match err.kind() {
            ErrorKind::DeviceChanged | ErrorKind::Xrun | ErrorKind::RealtimeDenied => {
                log::debug!("audio input: {err}")
            }
            _ => {
                log::warn!("audio input failed: {err}");
                failed.store(true, Ordering::Relaxed);
            }
        },
        None,
    )
}

/// Dispatch on the device's native sample format.
macro_rules! for_format {
    ($format:expr, $build:ident ( $($arg:expr),* )) => {
        match $format {
            SampleFormat::F32 => $build::<f32>($($arg),*),
            SampleFormat::F64 => $build::<f64>($($arg),*),
            SampleFormat::I8 => $build::<i8>($($arg),*),
            SampleFormat::I16 => $build::<i16>($($arg),*),
            SampleFormat::I32 => $build::<i32>($($arg),*),
            SampleFormat::U8 => $build::<u8>($($arg),*),
            SampleFormat::U16 => $build::<u16>($($arg),*),
            SampleFormat::U32 => $build::<u32>($($arg),*),
            other => return Err(format!("unsupported sample format {other}")),
        }
    };
}

/// The host sound is played through. Given the choice cpal prefers to talk
/// to the sound server directly, but ALSA's default device ends up at the same
/// server and is the path the player has been tested on.
fn output_host() -> cpal::Host {
    #[cfg(target_os = "linux")]
    if let Ok(host) = cpal::host_from_id(cpal::HostId::Alsa) {
        return host;
    }
    cpal::default_host()
}

/// The monitor source of the default output: whatever the system is playing.
/// `None` if there is no PulseAudio-compatible sound server to ask.
#[cfg(target_os = "linux")]
fn output_monitor() -> Option<cpal::Device> {
    let host = cpal::host_from_id(cpal::HostId::PulseAudio).ok()?;
    let output = host.default_output_device()?.id().ok()?;
    let monitor = format!("{}.monitor", output.id());
    host.input_devices()
        .ok()?
        .find(|d| d.id().is_ok_and(|id| id.id() == monitor))
}

#[cfg(not(target_os = "linux"))]
fn output_monitor() -> Option<cpal::Device> {
    None
}

fn device_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "audio device".into())
}

pub struct Engine {
    pub tap: Arc<Tap>,
    pub source: Source,
    shared: Arc<OutShared>,
    out_stream: Option<cpal::Stream>,
    out_rate: u32,
    in_stream: Option<cpal::Stream>,
    in_failed: Arc<AtomicBool>,
    commands: Sender<Cmd>,
    events_tx: Sender<AudioEvent>,
    events_rx: Receiver<AudioEvent>,
}

impl Engine {
    pub fn new() -> Self {
        let tap = Arc::new(Tap::new());
        let player = Arc::new(PlayerShared::new());
        let shared = Arc::new(OutShared {
            mode: AtomicU8::new(MODE_SILENT),
            paused: AtomicBool::new(false),
            volume: AtomicU32::new(0.6f32.to_bits()),
            demo_restart: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            player: player.clone(),
            tap: tap.clone(),
        });
        let (commands, command_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        let thread_events = events_tx.clone();
        std::thread::Builder::new()
            .name("decoder".into())
            .spawn(move || player::run(player, command_rx, thread_events))
            .expect("spawn decoder thread");
        Self {
            tap,
            source: Source::None,
            shared,
            out_stream: None,
            out_rate: 48_000,
            in_stream: None,
            in_failed: Arc::new(AtomicBool::new(false)),
            commands,
            events_tx,
            events_rx,
        }
    }

    /// Open the default output device if it is not open already.
    fn ensure_output(&mut self) -> Result<(), String> {
        if self.out_stream.is_some() {
            return Ok(());
        }
        let host = output_host();
        let device = host.default_output_device().ok_or("No audio output device found")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let rate = config.sample_rate;
        let stream = for_format!(format, output_stream(&device, config, self.shared.clone()))
            .map_err(|e| format!("Could not open audio output: {e}"))?;
        stream.play().map_err(|e| e.to_string())?;
        log::info!("audio output: {} @ {rate} Hz", device_name(&device));
        self.out_rate = rate;
        self.shared.player.rate.store(rate, Ordering::Relaxed);
        self.shared.failed.store(false, Ordering::Relaxed);
        self.out_stream = Some(stream);
        Ok(())
    }

    fn stop_capture(&mut self) {
        self.in_stream = None;
        self.in_failed.store(false, Ordering::Relaxed);
    }

    /// Start decoding and playing a file.
    pub fn play_file(&mut self, id: u64, path: PathBuf) -> Result<(), String> {
        self.ensure_output()?;
        self.stop_capture();
        self.tap.set_rate(self.out_rate);
        self.shared.paused.store(false, Ordering::Relaxed);
        self.shared.mode.store(MODE_PLAYER, Ordering::Relaxed);
        self.source = Source::Player;
        let _ = self.commands.send(Cmd::Load { id, path });
        Ok(())
    }

    pub fn start_demo(&mut self) -> Result<(), String> {
        self.ensure_output()?;
        self.stop_capture();
        let _ = self.commands.send(Cmd::Stop);
        self.tap.set_rate(self.out_rate);
        self.shared.demo_restart.store(true, Ordering::Relaxed);
        self.shared.paused.store(false, Ordering::Relaxed);
        self.shared.mode.store(MODE_DEMO, Ordering::Relaxed);
        self.source = Source::Demo;
        Ok(())
    }

    /// Whether what the system is playing can be captured as such. Where it
    /// cannot (Linux with no sound server offering a monitor source),
    /// `start_capture(true)` would fall back to the microphone, which is not
    /// something to open without being asked.
    pub fn system_audio_available() -> bool {
        !cfg!(target_os = "linux") || output_monitor().is_some()
    }

    /// Capture what the system is playing (`loopback`) or the default input
    /// device. Returns the name of the device being captured, and whether it
    /// is the one that was asked for: on Linux without a sound server to
    /// provide a monitor source, `loopback` falls back to the default input,
    /// which the user then has to route by hand.
    pub fn start_capture(&mut self, loopback: bool) -> Result<(String, bool), String> {
        let host = output_host();
        // Windows (WASAPI) and macOS 14.6+ can record an output device
        // directly. On Linux the sound server exposes a "monitor" source
        // for each output instead.
        let monitor = if loopback { output_monitor() } else { None };
        let direct_loopback = loopback && !cfg!(target_os = "linux");
        let monitor_used = monitor.is_some();
        let as_asked = !loopback || direct_loopback || monitor_used;
        let (device, supported) = if direct_loopback {
            let device = host
                .default_output_device()
                .ok_or("No audio output device to capture")?;
            let config = device.default_output_config().map_err(|e| e.to_string())?;
            (device, config)
        } else {
            let device = match monitor {
                Some(device) => device,
                None => host.default_input_device().ok_or("No audio input device found")?,
            };
            let config = device.default_input_config().map_err(|e| e.to_string())?;
            (device, config)
        };
        let format = supported.sample_format();
        let mut config: StreamConfig = supported.into();
        let rate = config.sample_rate;
        // Ask for the sound in pieces of about 10 ms. Left to choose, a
        // sound server hands a recording over in pieces of up to a second
        // or two, and the picture trails the music by as much.
        if cfg!(target_os = "linux") && monitor_used {
            config.buffer_size = cpal::BufferSize::Fixed((rate / 100).max(64));
        }
        self.in_failed.store(false, Ordering::Relaxed);
        let stream = for_format!(
            format,
            input_stream(&device, config, self.tap.clone(), self.in_failed.clone())
        )
        .map_err(|e| format!("Could not start capture: {e}"))?;
        stream.play().map_err(|e| e.to_string())?;

        self.shared.mode.store(MODE_SILENT, Ordering::Relaxed);
        let _ = self.commands.send(Cmd::Stop);
        self.tap.set_rate(rate);
        self.in_stream = Some(stream);
        self.source = if loopback { Source::System } else { Source::Mic };
        let name = device_name(&device);
        log::info!("capturing {name} @ {rate} Hz");
        Ok((name, as_asked))
    }

    /// Stop everything and return to the idle state.
    pub fn stop(&mut self) {
        self.shared.mode.store(MODE_SILENT, Ordering::Relaxed);
        self.stop_capture();
        let _ = self.commands.send(Cmd::Stop);
        self.source = Source::None;
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Relaxed);
    }

    pub fn paused(&self) -> bool {
        self.shared.paused.load(Ordering::Relaxed)
    }

    /// True when sound is flowing (or would be, for live capture).
    pub fn playing(&self) -> bool {
        match self.source {
            Source::None => false,
            Source::Player | Source::Demo => !self.paused(),
            Source::System | Source::Mic => true,
        }
    }

    pub fn seek(&self, seconds: f64) {
        let _ = self.commands.send(Cmd::Seek(seconds));
    }

    pub fn position(&self) -> f64 {
        self.shared.player.position()
    }

    pub fn duration(&self) -> f64 {
        self.shared.player.duration()
    }

    /// Linear slider position in 0..1, mapped to a perceptual gain curve.
    pub fn set_volume(&self, v: f32) {
        let v = v.clamp(0.0, 1.0);
        self.shared.volume.store((v * v).to_bits(), Ordering::Relaxed);
    }

    /// The tap to analyse, or `None` while nothing is sounding.
    pub fn active_tap(&self) -> Option<&Tap> {
        self.playing().then_some(&*self.tap)
    }

    /// Read tags for queued files off the main thread.
    pub fn scan(&self, items: Vec<(u64, PathBuf)>) {
        let events = self.events_tx.clone();
        let _ = std::thread::Builder::new().name("tag-scan".into()).spawn(move || {
            for (id, path) in items {
                if let Some(info) = player::probe(&path)
                    && events.send(AudioEvent::Meta { id, info }).is_err()
                {
                    return;
                }
            }
        });
    }

    /// Drain pending events. Call once per frame.
    pub fn poll(&mut self) -> Vec<AudioEvent> {
        let mut events: Vec<AudioEvent> = self.events_rx.try_iter().collect();
        let ended = self.shared.player.finished.swap(0, Ordering::AcqRel);
        if ended != 0 {
            events.push(AudioEvent::Ended { id: ended });
        }
        if self.shared.failed.swap(false, Ordering::Relaxed) {
            // Reopen on whatever the default device is now.
            self.out_stream = None;
            if matches!(self.source, Source::Player | Source::Demo)
                && let Err(e) = self.ensure_output()
            {
                self.stop();
                events.push(AudioEvent::DeviceLost(e));
            }
        }
        if self.in_failed.swap(false, Ordering::Relaxed) && self.in_stream.is_some() {
            self.stop();
            events.push(AudioEvent::DeviceLost("The capture device went away".into()));
        }
        events
    }
}
