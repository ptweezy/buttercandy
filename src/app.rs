//! The windowed application: owns the window, the audio engine and the
//! playlist, and turns input into actions.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{CursorIcon, Fullscreen, Theme, Window, WindowId};

use crate::Opts;
use crate::audio::analysis::BINS;
use crate::audio::{AudioEvent, Engine, Source};
use crate::gfx::presets::{self, GENRES, PRESETS};
use crate::gfx::{Gpu, pick_surface_format};
use crate::library;
use crate::palette::{self, PALETTES};
use crate::settings::{self, Settings};
use crate::stage::Stage;
use crate::ui::Input;
use crate::ui::logo::{self, Logo};
use crate::ui::view::{Action, Model, Popover, TrackRow};

pub enum UserEvent {
    FilesPicked(Vec<PathBuf>),
}

struct Track {
    id: u64,
    path: PathBuf,
}

struct Running {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    stage: Stage,
    engine: Engine,
    settings: Settings,
    settings_dirty: Option<Instant>,

    tracks: Vec<Track>,
    rows: Vec<TrackRow>,
    current: Option<usize>,
    next_id: u64,
    has_art: bool,
    failures: usize,
    /// Name of the device being captured, for the dock.
    device: String,

    preset: usize,
    /// The preset last shown in each genre, to come back to.
    last_in_genre: Vec<usize>,
    palette: usize,
    last_cycle: f32,

    input: Input,
    dropping: bool,
    dropped: Vec<PathBuf>,
    last_frame: Instant,
    fps: f32,
    /// When the next frame is due, while the frame rate is held to a target.
    next_frame: Instant,
    /// The refresh rate of the display the window is on, in Hz, and when it
    /// was last asked for.
    display_hz: Option<u32>,
    display_checked: Instant,
    cursor_hidden: bool,
    cursor_pointer: bool,
    exit_at: Option<Instant>,
    window_shot: Option<String>,
    /// False for smoke-test runs, which must not touch the saved settings.
    persist: bool,
}

struct App {
    opts: Opts,
    proxy: EventLoopProxy<UserEvent>,
    running: Option<Running>,
}

pub fn run(opts: Opts) -> Result<(), String> {
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        opts,
        proxy: event_loop.create_proxy(),
        running: None,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())
}

/// The logo, tilted to fill a square, as the window icon.
fn window_icon() -> Option<winit::window::Icon> {
    let size = 64;
    winit::window::Icon::from_rgba(logo::rasterize(Logo::Mark, size, size, -0.5), size, size).ok()
}

/// The refresh rate of the display `window` is on, to the nearest Hz.
fn display_hz(window: &Window) -> Option<u32> {
    let mhz = window.current_monitor()?.refresh_rate_millihertz()?;
    Some((mhz + 500) / 1000).filter(|hz| *hz > 0)
}

impl Running {
    fn new(event_loop: &ActiveEventLoop, opts: &Opts) -> Result<Self, String> {
        let mut settings = Settings::load();
        if let Some(p) = &opts.preset_arg {
            settings.preset = p.clone();
        }
        if let Some(p) = &opts.palette_arg {
            settings.palette = p.clone();
        }
        if let Some(v) = opts.volume {
            settings.volume = v.clamp(0.0, 1.0);
            settings.muted = false;
        }
        if let Some(v) = opts.reactivity {
            settings.reactivity = v;
        }
        if let Some(v) = opts.target_fps {
            settings.target_fps = settings::clamp_target_fps(v);
        }

        let attrs = Window::default_attributes()
            .with_title("buttercandy")
            .with_inner_size(LogicalSize::new(1280.0, 760.0))
            .with_min_inner_size(LogicalSize::new(780.0, 520.0))
            .with_theme(Some(Theme::Dark))
            .with_window_icon(window_icon());
        // A Linux desktop finds a window's name and icon through its
        // application id (see desktop.rs); the window icon above is for
        // Windows and for X11 window managers that show one.
        #[cfg(target_os = "linux")]
        let attrs = {
            use winit::platform::wayland::WindowAttributesExtWayland;
            use winit::platform::x11::WindowAttributesExtX11;
            let id = crate::desktop::APP_ID;
            let attrs = WindowAttributesExtWayland::with_name(attrs, id, id);
            WindowAttributesExtX11::with_name(attrs, id, id)
        };
        let window = Arc::new(event_loop.create_window(attrs).map_err(|e| e.to_string())?);
        let (gpu, surface, adapter) = Gpu::for_window(window.clone(), event_loop.owned_display_handle())?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("the GPU cannot present to this window")?;
        config.format = pick_surface_format(&caps);
        config.present_mode = wgpu::PresentMode::AutoVsync;
        if opts.window_shot.is_some() && caps.usages.contains(wgpu::TextureUsages::COPY_SRC) {
            config.usage |= wgpu::TextureUsages::COPY_SRC;
        }
        surface.configure(&gpu.device, &config);
        log::info!(
            "window {}x{} px at scale {:.2}, surface {:?}, {:?}",
            config.width,
            config.height,
            window.scale_factor(),
            config.format,
            config.present_mode
        );

        let preset = presets::index_of(&settings.preset);
        let pal = palette::index_of(&settings.palette);
        let mut stage = Stage::new(&gpu, config.format, (config.width, config.height), preset, pal);
        // Presets that choose their own path take a new one every run.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |t| t.as_nanos() as u64);
        stage.renderer.shuffle_drivers(seed);
        stage.renderer.glow = settings.glow;
        stage.renderer.set_render_scale(&gpu, settings.render_scale);
        stage.analysis.sensitivity = settings.sensitivity;
        stage.analysis.reactivity = settings.reactivity;

        let engine = Engine::new();
        engine.set_volume(if settings.muted { 0.0 } else { settings.volume });

        let mut r = Self {
            window,
            surface,
            config,
            gpu,
            stage,
            engine,
            settings,
            settings_dirty: None,
            tracks: Vec::new(),
            rows: Vec::new(),
            current: None,
            next_id: 1,
            has_art: false,
            failures: 0,
            device: String::new(),
            preset,
            last_in_genre: (0..GENRES.len())
                .map(|g| {
                    if PRESETS[preset].genre == g {
                        preset
                    } else {
                        presets::in_genre(g).next().unwrap_or(0)
                    }
                })
                .collect(),
            palette: pal,
            last_cycle: 0.0,
            input: Input::default(),
            dropping: false,
            dropped: Vec::new(),
            last_frame: Instant::now(),
            fps: 60.0,
            next_frame: Instant::now(),
            display_hz: None,
            display_checked: Instant::now(),
            cursor_hidden: false,
            cursor_pointer: false,
            exit_at: opts.exit_after.map(|s| Instant::now() + Duration::from_secs_f32(s)),
            window_shot: opts.window_shot.clone(),
            persist: opts.exit_after.is_none(),
        };
        if opts.fullscreen {
            r.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        if !opts.files.is_empty() {
            r.add_files(opts.files.clone());
        } else {
            match opts.source.as_deref() {
                Some("demo") => r.apply(Action::StartDemo, None),
                Some("system") => r.apply(Action::StartSystem, None),
                Some("mic") => r.apply(Action::StartMic, None),
                Some(_) => {}
                // Left to itself, buttercandy listens to whatever the
                // computer is playing; any other choice is a click away.
                None if Engine::system_audio_available() => r.apply(Action::StartSystem, None),
                None => {}
            }
        }
        Ok(r)
    }

    fn touch_settings(&mut self) {
        self.settings_dirty.get_or_insert_with(Instant::now);
    }

    fn toast(&mut self, text: impl Into<String>) {
        self.stage.view.toast(text);
    }

    // ---- playlist ---------------------------------------------------------

    fn add_files(&mut self, paths: Vec<PathBuf>) {
        let files = library::expand(paths);
        if files.is_empty() {
            self.toast("No playable audio files there");
            return;
        }
        let first_new = self.tracks.len();
        let mut scan = Vec::with_capacity(files.len());
        for path in files {
            let (title, artist) = library::display_name(&path);
            let id = self.next_id;
            self.next_id += 1;
            scan.push((id, path.clone()));
            self.tracks.push(Track { id, path });
            self.rows.push(TrackRow {
                title,
                artist,
                duration: None,
            });
        }
        let added = self.tracks.len() - first_new;
        self.engine.scan(scan);
        // Start playing unless a file is already playing.
        if self.engine.source != Source::Player {
            self.play_index(first_new);
        } else {
            self.toast(format!("Added {added} track{}", if added == 1 { "" } else { "s" }));
        }
    }

    fn play_index(&mut self, index: usize) {
        let Some(track) = self.tracks.get(index) else { return };
        self.has_art = false;
        match self.engine.play_file(track.id, track.path.clone()) {
            Ok(()) => self.current = Some(index),
            Err(e) => self.toast(e),
        }
    }

    fn step_track(&mut self, delta: i32) {
        if self.tracks.is_empty() {
            return;
        }
        let n = self.tracks.len() as i32;
        let from = self.current.map_or(if delta > 0 { -1 } else { 0 }, |c| c as i32);
        self.play_index((from + delta).rem_euclid(n) as usize);
    }

    fn open_dialog(&self, proxy: Option<&EventLoopProxy<UserEvent>>) {
        let Some(proxy) = proxy.cloned() else { return };
        // The dialog blocks, so it gets a thread of its own.
        std::thread::spawn(move || {
            let picked = pollster::block_on(
                rfd::AsyncFileDialog::new()
                    .set_title("Open music")
                    .add_filter("Audio", library::EXTENSIONS)
                    .pick_files(),
            );
            if let Some(files) = picked {
                let paths = files.iter().map(|f| f.path().to_path_buf()).collect();
                let _ = proxy.send_event(UserEvent::FilesPicked(paths));
            }
        });
    }

    // ---- presets / palettes -------------------------------------------------

    fn set_preset(&mut self, index: usize, announce: bool) {
        let index = index % PRESETS.len();
        if !self.stage.renderer.preset_available(index) {
            self.toast(format!("{} failed to compile on this GPU", PRESETS[index].name));
            return;
        }
        self.preset = index;
        self.last_in_genre[PRESETS[index].genre] = index;
        self.stage.renderer.set_preset(index, false);
        self.settings.preset = PRESETS[index].id.into();
        self.last_cycle = self.stage.time;
        self.touch_settings();
        if announce {
            self.toast(PRESETS[index].name);
        }
    }

    /// Step through the presets of the genre on screen, round and round.
    fn step_preset(&mut self, delta: i32, announce: bool) {
        let members: Vec<usize> = presets::in_genre(PRESETS[self.preset].genre).collect();
        let n = members.len() as i32;
        // Skip over any preset that failed to compile.
        let mut i = members.iter().position(|&p| p == self.preset).unwrap_or(0) as i32;
        for _ in 0..n {
            i = (i + delta).rem_euclid(n);
            if self.stage.renderer.preset_available(members[i as usize]) {
                break;
            }
        }
        self.set_preset(members[i as usize], announce);
    }

    /// Switch to another genre, at the preset last shown there.
    fn step_genre(&mut self, delta: i32) {
        let n = GENRES.len() as i32;
        let mut g = PRESETS[self.preset].genre as i32;
        for _ in 0..n {
            g = (g + delta).rem_euclid(n);
            let last = self.last_in_genre[g as usize];
            let pick = std::iter::once(last)
                .chain(presets::in_genre(g as usize))
                .find(|&p| self.stage.renderer.preset_available(p));
            if let Some(p) = pick {
                if p != self.preset {
                    self.set_preset(p, false);
                    self.toast(format!("{} · {}", GENRES[g as usize].name, PRESETS[p].name));
                }
                return;
            }
        }
    }

    fn set_palette(&mut self, index: usize, announce: bool) {
        let index = index % PALETTES.len();
        self.palette = index;
        self.stage.renderer.set_palette(&self.gpu, &PALETTES[index]);
        self.settings.palette = PALETTES[index].id.into();
        self.touch_settings();
        if announce {
            self.toast(format!("Palette · {}", PALETTES[index].name));
        }
    }

    fn set_volume(&mut self, v: f32) {
        self.settings.volume = v.clamp(0.0, 1.0);
        self.settings.muted = false;
        self.engine.set_volume(self.settings.volume);
        self.touch_settings();
    }

    fn toggle_fullscreen(&mut self) {
        let on = self.window.fullscreen().is_some();
        self.window
            .set_fullscreen(if on { None } else { Some(Fullscreen::Borderless(None)) });
    }

    fn snapshot(&mut self) {
        let texture = self.stage.renderer.frame_texture();
        let (w, h) = (texture.width(), texture.height());
        let rgba = match self.gpu.read_texture(texture) {
            Ok(rgba) => rgba,
            Err(e) => return self.toast(format!("Screenshot failed: {e}")),
        };
        let dir = dirs::picture_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_default()
            .join("buttercandy");
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let path = dir.join(format!("buttercandy-{}-{stamp}.png", PRESETS[self.preset].id));
        self.toast(format!("Saved {}", path.display()));
        // Encoding a PNG takes a moment; keep it off the render thread.
        std::thread::spawn(move || {
            let _ = std::fs::create_dir_all(&dir);
            if let Err(e) = image::save_buffer(&path, &rgba, w, h, image::ExtendedColorType::Rgba8) {
                log::warn!("could not save {}: {e}", path.display());
            }
        });
    }

    // ---- actions --------------------------------------------------------------

    fn apply(&mut self, action: Action, proxy: Option<&EventLoopProxy<UserEvent>>) {
        match action {
            Action::SetPreset(i) => self.set_preset(i, false),
            Action::StepPreset(d) => self.step_preset(d, true),
            Action::StepGenre(d) => self.step_genre(d),
            Action::SetPalette(i) => self.set_palette(i, false),
            Action::OpenFiles => self.open_dialog(proxy),
            Action::StartDemo => match self.engine.start_demo() {
                Ok(()) => {
                    self.current = None;
                    self.toast("Playing the demo track");
                }
                Err(e) => self.toast(e),
            },
            Action::StartSystem | Action::StartMic => {
                let loopback = action == Action::StartSystem;
                match self.engine.start_capture(loopback) {
                    Ok((name, as_asked)) => {
                        self.current = None;
                        if as_asked {
                            self.toast(format!("Listening to {name}"));
                        } else {
                            // No monitor source to be found: we are on the default
                            // input until the user points us at one.
                            self.toast("Route a “Monitor of …” source to buttercandy in your sound settings");
                        }
                        self.device = name;
                    }
                    Err(e) => self.toast(e),
                }
            }
            Action::TogglePlay => match self.engine.source {
                Source::None => {
                    if self.tracks.is_empty() {
                        self.open_dialog(proxy);
                    } else {
                        self.play_index(self.current.unwrap_or(0));
                    }
                }
                Source::Player | Source::Demo => {
                    let paused = !self.engine.paused();
                    self.engine.set_paused(paused);
                }
                Source::System | Source::Mic => {
                    self.engine.stop();
                    self.toast("Stopped listening");
                }
            },
            Action::Next => self.step_track(1),
            Action::Prev => {
                // Like every player: back restarts the song unless it just began.
                if self.engine.source == Source::Player && self.engine.position() > 3.0 {
                    self.engine.seek(0.0);
                } else {
                    self.step_track(-1);
                }
            }
            Action::Seek(f) => {
                let d = self.engine.duration();
                if d > 0.0 {
                    self.engine.seek(f as f64 * d);
                }
            }
            Action::SetVolume(v) => self.set_volume(v),
            Action::ToggleMute => {
                self.settings.muted = !self.settings.muted;
                self.engine
                    .set_volume(if self.settings.muted { 0.0 } else { self.settings.volume });
                self.touch_settings();
            }
            Action::ToggleFullscreen => self.toggle_fullscreen(),
            Action::ToggleAutoCycle => {
                let secs = if self.settings.auto_cycle > 0 { 0 } else { 30 };
                self.apply(Action::SetAutoCycle(secs), proxy);
                self.toast(if secs > 0 {
                    "Auto-cycle on · every 30 s"
                } else {
                    "Auto-cycle off"
                });
            }
            Action::SetAutoCycle(secs) => {
                self.settings.auto_cycle = secs;
                self.last_cycle = self.stage.time;
                self.touch_settings();
            }
            Action::SetSensitivity(v) => {
                self.settings.sensitivity = v.clamp(0.25, 3.0);
                self.stage.analysis.sensitivity = self.settings.sensitivity;
                self.touch_settings();
            }
            Action::SetReactivity(v) => {
                self.settings.reactivity = v.clamp(0.0, 1.0);
                self.stage.analysis.reactivity = self.settings.reactivity;
                self.touch_settings();
            }
            Action::SetGlassFrost(v) => {
                self.settings.glass_frost = v.clamp(0.0, 1.0);
                self.touch_settings();
            }
            Action::SetGlow(v) => {
                self.settings.glow = v.clamp(0.0, 2.0);
                self.stage.renderer.glow = self.settings.glow;
                self.touch_settings();
            }
            Action::SetRenderScale(s) => {
                // In steps of 5%, which is as fine as the slider goes.
                let s = (s.clamp(0.25, 1.0) * 20.0).round() / 20.0;
                self.settings.render_scale = s;
                self.stage.renderer.set_render_scale(&self.gpu, s);
                self.touch_settings();
            }
            Action::SetTargetFps(fps) => {
                self.settings.target_fps = settings::clamp_target_fps(fps);
                self.next_frame = Instant::now();
                self.touch_settings();
            }
            Action::ToggleFps => {
                self.settings.show_fps = !self.settings.show_fps;
                self.touch_settings();
            }
            Action::PlayIndex(i) => self.play_index(i),
            Action::RemoveIndex(i) => {
                if i >= self.tracks.len() {
                    return;
                }
                self.tracks.remove(i);
                self.rows.remove(i);
                match self.current {
                    Some(c) if c == i => {
                        // The playing track was removed: move on, or stop.
                        if self.tracks.is_empty() {
                            self.current = None;
                            self.engine.stop();
                        } else if self.engine.source == Source::Player {
                            self.play_index(i.min(self.tracks.len() - 1));
                        } else {
                            self.current = None;
                        }
                    }
                    Some(c) if c > i => self.current = Some(c - 1),
                    _ => {}
                }
            }
            Action::ClearQueue => {
                self.tracks.clear();
                self.rows.clear();
                self.current = None;
                if self.engine.source == Source::Player {
                    self.engine.stop();
                }
            }
        }
    }

    fn audio_events(&mut self) {
        for event in self.engine.poll() {
            let current_id = self.current.and_then(|c| self.tracks.get(c)).map(|t| t.id);
            match event {
                AudioEvent::Loaded { id, info } | AudioEvent::Meta { id, info } => {
                    if Some(id) == current_id {
                        self.failures = 0;
                    }
                    if let Some(i) = self.tracks.iter().position(|t| t.id == id) {
                        let row = &mut self.rows[i];
                        if let Some(title) = info.title {
                            row.title = title;
                            // A real title without an artist beats a guessed artist.
                            row.artist = info.artist.clone().unwrap_or_default();
                        } else if let Some(artist) = info.artist {
                            row.artist = artist;
                        }
                        if info.duration.is_some() {
                            row.duration = info.duration;
                        }
                    }
                }
                AudioEvent::Art { id, rgba } => {
                    if Some(id) == current_id {
                        self.has_art = rgba.is_some();
                        if let Some(rgba) = rgba {
                            self.stage.ui_renderer.set_art(&self.gpu, &rgba);
                        }
                    }
                }
                AudioEvent::Failed { id, error } => {
                    let name = self
                        .tracks
                        .iter()
                        .position(|t| t.id == id)
                        .map(|i| self.rows[i].title.clone());
                    self.toast(format!(
                        "Can't play {}: {error}",
                        name.as_deref().unwrap_or("that file")
                    ));
                    if Some(id) == current_id {
                        // Skip ahead, but give up once every track has failed.
                        self.failures += 1;
                        if self.failures < self.tracks.len() {
                            self.step_track(1);
                        } else {
                            self.failures = 0;
                            self.engine.stop();
                        }
                    }
                }
                AudioEvent::Ended { id } => {
                    if Some(id) == current_id && self.engine.source == Source::Player {
                        self.step_track(1);
                    }
                }
                AudioEvent::DeviceLost(msg) => self.toast(msg),
            }
        }
    }

    // ---- input ------------------------------------------------------------------

    fn key(&mut self, key: &Key, repeat: bool, proxy: &EventLoopProxy<UserEvent>, event_loop: &ActiveEventLoop) {
        self.stage.view.wake();
        let view = &mut self.stage.view;
        match key {
            Key::Named(NamedKey::Escape) if !repeat => {
                if view.popover != Popover::None {
                    view.popover = Popover::None;
                } else if view.queue_open {
                    view.queue_open = false;
                } else if self.window.fullscreen().is_some() {
                    self.window.set_fullscreen(None);
                }
            }
            Key::Named(NamedKey::Space) if !repeat => self.apply(Action::TogglePlay, Some(proxy)),
            Key::Named(NamedKey::ArrowRight) => self.step_preset(1, true),
            Key::Named(NamedKey::ArrowLeft) => self.step_preset(-1, true),
            Key::Named(NamedKey::ArrowUp) | Key::Named(NamedKey::ArrowDown) => {
                let delta = if matches!(key, Key::Named(NamedKey::ArrowUp)) {
                    0.05
                } else {
                    -0.05
                };
                self.set_volume(self.settings.volume + delta);
                self.toast(format!("Volume {:.0}%", self.settings.volume * 100.0));
            }
            Key::Named(NamedKey::F11) if !repeat => self.toggle_fullscreen(),
            Key::Character(c) if !repeat => match c.to_lowercase().as_str() {
                "c" => self.apply(Action::ToggleAutoCycle, Some(proxy)),
                "n" => self.apply(Action::Next, Some(proxy)),
                "b" => self.apply(Action::Prev, Some(proxy)),
                "o" => self.apply(Action::OpenFiles, Some(proxy)),
                "a" => self.apply(Action::StartSystem, Some(proxy)),
                "m" => self.apply(Action::StartMic, Some(proxy)),
                "d" => self.apply(Action::StartDemo, Some(proxy)),
                "q" => view.queue_open = !view.queue_open,
                "f" => self.toggle_fullscreen(),
                "h" => view.hidden = !view.hidden,
                "g" => self.step_genre(1),
                "k" => {
                    self.settings.show_key = !self.settings.show_key;
                    self.touch_settings();
                }
                "s" => self.snapshot(),
                "?" | "/" => view.toggle(Popover::Help),
                "-" | "_" | "=" | "+" => {
                    let step = if matches!(c.as_str(), "-" | "_") { -0.1 } else { 0.1 };
                    let r = ((self.settings.reactivity + step) * 10.0).round() / 10.0;
                    self.settings.reactivity = r.clamp(0.0, 1.0);
                    self.stage.analysis.reactivity = self.settings.reactivity;
                    self.touch_settings();
                    self.toast(format!("Reactivity {:.0}%", self.settings.reactivity * 100.0));
                }
                "[" => self.set_palette(self.palette + PALETTES.len() - 1, true),
                "]" => self.set_palette(self.palette + 1, true),
                digit if digit.len() == 1 && ("0"..="9").contains(&digit) => {
                    // As the keys lie: 1 to 9, then 0 for the tenth.
                    let i = (digit.as_bytes()[0] - b'0') as usize;
                    let i = (i + 9) % 10;
                    // Within the genre on screen.
                    if let Some(p) = presets::in_genre(PRESETS[self.preset].genre).nth(i) {
                        self.set_preset(p, true);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        let _ = event_loop;
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.gpu.device, &self.config);
        self.stage.renderer.resize(&self.gpu, width, height);
    }

    // ---- frame ------------------------------------------------------------------

    /// The status line a smoke test ends on.
    fn log_exit(&self) {
        let a = &self.stage.analysis;
        let row = self.current.and_then(|c| self.rows.get(c));
        log::info!(
            "exiting after the requested time: {:.0} fps, source {:?}, position {:.2}/{:.2} s, level {:.2}, bass {:.2}, beats {}, idle {:.2}, track {:?}, art {}",
            self.fps,
            self.engine.source,
            self.engine.position(),
            self.engine.duration(),
            a.level,
            a.bass,
            a.beat.count,
            a.idle,
            row.map(|t| (t.title.as_str(), t.artist.as_str(), t.duration)),
            self.has_art,
        );
    }

    /// How long each frame lasts when the frame rate is held to a target.
    /// A target the display is too slow for is no target: the display paces
    /// the picture at its own rate.
    fn frame_interval(&self) -> Option<Duration> {
        let fps = self.settings.target_fps;
        let held = fps > 0 && self.display_hz.is_none_or(|hz| fps <= settings::max_target_fps(hz));
        held.then(|| Duration::from_secs_f64(1.0 / fps as f64))
    }

    /// Ask which display the window is on, and say so when that changes.
    fn check_display(&mut self, now: Instant) {
        let hz = display_hz(&self.window);
        self.display_checked = now;
        if hz != self.display_hz {
            self.display_hz = hz;
            let rate = hz.map_or("an unknown rate".into(), |hz| format!("{hz} Hz"));
            match self.frame_interval() {
                Some(_) => log::info!("display at {rate}, picture held to {} fps", self.settings.target_fps),
                None => log::info!("display at {rate}, which paces the picture"),
            }
        }
    }

    fn frame(&mut self, proxy: &EventLoopProxy<UserEvent>) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().clamp(0.0005, 0.1);
        self.last_frame = now;
        self.fps += (1.0 / dt - self.fps) * 0.05;

        // A new window is not on any display yet, and it may later move to
        // one with another refresh rate.
        if self.display_hz.is_none() || now - self.display_checked > Duration::from_secs(1) {
            self.check_display(now);
        }
        // Frames are due on a steady beat, so one that starts a little late
        // does not make the next late too. A frame more than a beat late (or
        // one the system asked for early) starts the count afresh, rather
        // than leaving frames to catch up on.
        if let Some(interval) = self.frame_interval() {
            let next = self.next_frame + interval;
            self.next_frame = if next > now && next <= now + interval {
                next
            } else {
                now + interval
            };
        }

        // Files dropped during the last batch of events arrive one at a time.
        if !self.dropped.is_empty() {
            let files = std::mem::take(&mut self.dropped);
            self.add_files(files);
        }
        self.audio_events();

        self.stage.analysis.update(self.engine.active_tap(), dt);

        // Auto-cycle on a beat once the interval has passed (or shortly after,
        // if no beat comes).
        let cycle = self.settings.auto_cycle as f32;
        if cycle > 0.0 {
            let elapsed = self.stage.time - self.last_cycle;
            if elapsed > cycle && (self.stage.analysis.beat.pulse > 0.95 || elapsed > cycle + 3.0) {
                self.step_preset(1, false);
            }
        }

        let size = (self.config.width, self.config.height);
        let scale = self.window.scale_factor() as f32;
        self.stage.ui.begin(size, scale, dt, self.input);
        let source = self.engine.source;
        let row = self.current.and_then(|c| self.rows.get(c));
        let (title, subtitle): (&str, &str) = match source {
            Source::Player => row.map_or(("Loading…", ""), |r| (r.title.as_str(), r.artist.as_str())),
            Source::Demo => ("Demo track", "Generative synth · 124 BPM"),
            Source::System => ("System audio", self.device.as_str()),
            Source::Mic => ("Microphone", self.device.as_str()),
            Source::None => ("Nothing playing", "Open a file or pick a source"),
        };
        let model = Model {
            source,
            playing: self.engine.playing(),
            position: self.engine.position(),
            duration: self.engine.duration(),
            title,
            subtitle,
            has_art: self.has_art,
            tracks: &self.rows,
            current: self.current,
            preset: self.preset,
            palette: self.palette,
            settings: &self.settings,
            fullscreen: self.window.fullscreen().is_some(),
            fps: self.fps,
            display_hz: self.display_hz,
            spectrum: &self.stage.analysis.tex[..BINS],
            beat: self.stage.analysis.beat.pulse,
            voices: {
                let a = &self.stage.analysis;
                [
                    a.onsets[0].pulse,
                    a.onsets[1].pulse,
                    a.onsets[2].pulse,
                    a.bassline,
                    a.melody,
                ]
            },
            dropping: self.dropping,
        };
        let actions = self.stage.view.build(&mut self.stage.ui, &model);
        // One-shot input has been consumed.
        self.input.pressed = false;
        self.input.released = false;
        self.input.scroll = 0.0;
        for action in actions {
            self.apply(action, Some(proxy));
        }

        // Cursor: hidden with the chrome, a hand over anything clickable.
        let hide = self.stage.view.hide_cursor && self.input.inside;
        if hide != self.cursor_hidden {
            self.window.set_cursor_visible(!hide);
            self.cursor_hidden = hide;
        }
        let pointer = self.stage.ui.hovering;
        if pointer != self.cursor_pointer {
            self.window.set_cursor(if pointer {
                CursorIcon::Pointer
            } else {
                CursorIcon::Default
            });
            self.cursor_pointer = pointer;
        }

        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                let view = frame.texture.create_view(&Default::default());
                self.stage.render(&self.gpu, &view, size, dt, true);
                // Smoke tests can ask for the final frame exactly as presented.
                if self.exit_at.is_some_and(|t| Instant::now() >= t)
                    && self.config.usage.contains(wgpu::TextureUsages::COPY_SRC)
                    && let Some(path) = self.window_shot.take()
                {
                    match self.gpu.read_texture(&frame.texture) {
                        Ok(rgba) => {
                            if let Err(e) =
                                image::save_buffer(&path, &rgba, size.0, size.1, image::ExtendedColorType::Rgba8)
                            {
                                log::warn!("could not save {path}: {e}");
                            }
                        }
                        Err(e) => log::warn!("window capture failed: {e}"),
                    }
                }
                self.window.pre_present_notify();
                self.gpu.queue.present(frame);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.gpu.device, &self.config);
            }
            // Minimised or occluded: nothing to draw this time.
            _ => std::thread::sleep(Duration::from_millis(8)),
        }

        // Write settings a moment after the last change, not on every drag.
        if self
            .settings_dirty
            .is_some_and(|t| t.elapsed() > Duration::from_secs(1))
        {
            if self.persist {
                self.settings.save();
            }
            self.settings_dirty = None;
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        match Running::new(event_loop, &self.opts) {
            Ok(r) => self.running = Some(r),
            Err(e) => {
                eprintln!("buttercandy: {e}");
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, _: &ActiveEventLoop, event: UserEvent) {
        if let Some(r) = &mut self.running {
            match event {
                UserEvent::FilesPicked(paths) => r.add_files(paths),
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(r) = &mut self.running else { return };
        match event {
            WindowEvent::CloseRequested => {
                if r.persist {
                    r.settings.save();
                }
                event_loop.exit();
            }
            WindowEvent::Resized(size) => r.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                r.frame(&self.proxy);
                if r.exit_at.is_some_and(|t| Instant::now() >= t) {
                    r.log_exit();
                    event_loop.exit();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let s = r.window.scale_factor();
                r.input.mouse = ((position.x / s) as f32, (position.y / s) as f32);
                r.input.inside = true;
            }
            WindowEvent::CursorEntered { .. } => r.input.inside = true,
            WindowEvent::CursorLeft { .. } => r.input.inside = false,
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let down = state == ElementState::Pressed;
                r.input.pressed |= down && !r.input.down;
                r.input.released |= !down && r.input.down;
                r.input.down = down;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                r.input.scroll += match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                r.key(&event.logical_key, event.repeat, &self.proxy, event_loop);
            }
            WindowEvent::HoveredFile(_) => r.dropping = true,
            WindowEvent::HoveredFileCancelled => r.dropping = false,
            WindowEvent::DroppedFile(path) => {
                r.dropping = false;
                r.dropped.push(path);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Redraws arrive as events, paced by the display. On Linux nothing
        // blocks in between, so polling would spin a whole core; doze instead.
        // Events still wake the loop at once.
        let mut flow = if cfg!(target_os = "linux") {
            ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(2))
        } else {
            ControlFlow::Poll
        };
        if let Some(r) = &mut self.running {
            // A hidden window is sent no redraws (on Wayland, none at all
            // while the screen is locked), and everything normally happens
            // in the redraw. Two things must not wait for the window to come
            // back: `--exit-after` noticing that its time is up...
            let now = Instant::now();
            if r.exit_at.is_some_and(|t| now >= t + Duration::from_millis(500)) {
                r.log_exit();
                event_loop.exit();
                return;
            }
            // ...and the queue moving on to the next track.
            if now - r.last_frame > Duration::from_millis(250) {
                r.audio_events();
            }
            // Held to a frame rate, there is nothing to draw until the next
            // frame is due: sleep until then.
            if r.frame_interval().is_some() && now < r.next_frame {
                flow = ControlFlow::WaitUntil(r.next_frame);
            } else {
                r.window.request_redraw();
            }
        }
        event_loop.set_control_flow(flow);
    }
}
