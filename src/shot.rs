//! Headless rendering: drive the whole pipeline offscreen with the demo synth
//! as the audio source and save a PNG. No window, no audio device.
//!
//! This is how the visuals and interface are checked without a human in the
//! loop, and it doubles as a way to render wallpapers.

use crate::Opts;
use crate::audio::Source;
use crate::audio::analysis::BINS;
use crate::audio::demo::DemoSynth;
use crate::audio::ring::Tap;
use crate::gfx::Gpu;
use crate::gfx::presets::{self, PRESETS};
use crate::gfx::renderer::FRAME_FORMAT;
use crate::palette;
use crate::settings::Settings;
use crate::stage::Stage;
use crate::ui::Input;
use crate::ui::view::{Model, Popover, TrackRow};

const RATE: u32 = 48_000;

fn mock_tracks() -> Vec<TrackRow> {
    [
        ("Butterscotch Skyline", "The Confectioners", 214.0),
        ("Taffy Pull", "Sugar Rush", 187.0),
        ("Midnight Gumdrops", "Neon Pantry", 251.0),
        ("Sprinkle Theory", "Sugar Rush", 199.0),
        ("真夜中のキャンディ", "Tokyo Sweets Club", 233.0),
        (
            "Caramel Sun (Extended Sunrise Mix)",
            "The Confectioners feat. Honeycomb",
            412.0,
        ),
        ("Wormhole Lollipop", "Neon Pantry", 176.0),
    ]
    .into_iter()
    .map(|(title, artist, d)| TrackRow {
        title: title.into(),
        artist: artist.into(),
        duration: Some(d),
    })
    .collect()
}

/// What `--sheet` asks for: several frames tiled into one image.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sheet {
    /// Successive moments, `--every` seconds apart, filling a grid.
    Times { cols: u32, rows: u32 },
    /// The same moment in each of the palettes.
    Palettes,
    /// The same moment in each of the presets.
    Presets,
    /// One column per voice of the music (kick, snare, hats, bass, tune),
    /// each playing alone, with successive moments down the column: what the
    /// preset makes of each voice by itself.
    Voices,
}

/// The voices `--solo` knows, in the order the shaders number them.
pub const VOICES: [&str; 5] = ["kick", "snare", "hats", "bass", "tune"];

/// Space between and around the frames of a sheet, in pixels.
const GUTTER: u32 = 6;

/// The result of playing the demo track through one preset and palette.
struct Take {
    /// The captured frames, as tightly packed RGBA.
    frames: Vec<Vec<u8>>,
    /// Milliseconds of GPU time per frame, and how many frames that is the
    /// average of. `None` when too few were rendered to say.
    timing: Option<(f32, usize)>,
    /// With `--motion`: how much the picture changed from each frame to the
    /// next, as mean absolute difference on a 0-255 scale.
    motion: Vec<f32>,
    summary: String,
}

/// Mean absolute difference between two frames of tightly packed RGBA.
fn difference(a: &[u8], b: &[u8]) -> f32 {
    let sum: u64 = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y) as u64).sum();
    sum as f32 / a.len().max(1) as f32
}

/// Render the demo track and capture `shots` frames, `every` frames apart.
/// `body`, if given, replaces the preset's built-in shader.
fn take(
    gpu: &Gpu,
    o: &Opts,
    preset: usize,
    pal: usize,
    body: Option<&str>,
    solo: Option<usize>,
    shots: usize,
    every: usize,
) -> Result<Take, String> {
    let (w, h) = o.size;
    let mut stage = Stage::new(gpu, FRAME_FORMAT, (w, h), preset, pal);
    stage.analysis.reactivity = o.reactivity.unwrap_or(Settings::default().reactivity);
    if let Some(body) = body {
        stage.renderer.load_preset(gpu, preset, body)?;
    }
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shot"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FRAME_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&Default::default());

    // Which interface state to pose.
    let state = o.ui.as_str();
    let with_ui = state != "none";
    let silent = o.silent || state == "hero";
    let source = match state {
        "hero" => Source::None,
        "player" | "queue" | "palette" | "settings" | "help" | "drop" => Source::Player,
        "system" => Source::System,
        _ => Source::Demo,
    };
    let tracks = if source == Source::Player {
        mock_tracks()
    } else {
        Vec::new()
    };
    let settings = Settings {
        show_fps: state == "settings",
        show_key: state == "key",
        ..Default::default()
    };
    stage.view.pin = true;
    stage.view.queue_open = state == "queue";
    stage.view.popover = match state {
        "palette" => Popover::Palette,
        "settings" => Popover::Settings,
        "help" => Popover::Help,
        _ => Popover::None,
    };
    if !o.toast.is_empty() {
        stage.view.toast(o.toast.clone());
    }

    let tap = Tap::new();
    tap.set_rate(RATE);
    let mut synth = DemoSynth::new(RATE);
    if let Some(voice) = solo {
        synth.voices = std::array::from_fn(|i| i == voice);
    }
    // The frame rate being simulated: how much music and time each frame gets.
    let fps = o.fps.clamp(1.0, 1000.0);
    let mut audio = vec![0.0f32; (RATE as f32 / fps).round() as usize * 2];
    let dt = 1.0 / fps;
    let skip = (o.skip * fps) as usize;
    // The first frame captured, and the last.
    let first = skip + (o.seconds * fps).max(1.0) as usize - 1;
    let last = first + (shots - 1) * every;
    // Time the frames leading up to the first capture, less a few to let
    // the driver finish compiling the pipelines.
    let warm = skip + ((first - skip) / 8).min(10);
    let wait = || gpu.device.poll(wgpu::PollType::wait_indefinitely()).map(|_| ());
    let mut started = None;
    let mut timing = None;
    let input = Input {
        mouse: o.mouse.unwrap_or((-1.0, -1.0)),
        inside: o.mouse.is_some(),
        ..Default::default()
    };

    let mut frames = Vec::with_capacity(shots);
    let mut motion = Vec::new();
    let mut before: Option<Vec<u8>> = None;
    for i in 0..=last {
        if !silent {
            synth.render(&mut audio);
            tap.push_interleaved(&audio, 2);
        }
        stage.analysis.update((!silent).then_some(&tap), dt);
        if i < skip {
            continue;
        }
        if i == warm && first - warm >= 8 && !o.motion {
            wait().map_err(|e| e.to_string())?;
            started = Some(std::time::Instant::now());
        }
        if with_ui {
            // Popovers set from the command line should not be dismissed.
            stage.ui.begin((w, h), o.scale, dt, input);
            let (title, subtitle) = match source {
                Source::Player => ("Butterscotch Skyline", "The Confectioners"),
                Source::Demo => ("Demo track", "Generative synth · 124 BPM"),
                Source::System => ("System audio", "Speakers (Realtek Audio)"),
                _ => ("Nothing playing", "Open a file or pick a source"),
            };
            let model = Model {
                source,
                playing: source != Source::None,
                position: 83.0,
                duration: if source == Source::Player { 214.0 } else { 0.0 },
                title,
                subtitle,
                has_art: false,
                tracks: &tracks,
                current: (source == Source::Player).then_some(0),
                preset,
                palette: pal,
                settings: &settings,
                fullscreen: false,
                fps: 60.0,
                display_hz: Some(60),
                spectrum: &stage.analysis.tex[..BINS],
                beat: stage.analysis.beat.pulse,
                voices: {
                    let a = &stage.analysis;
                    [
                        a.onsets[0].pulse,
                        a.onsets[1].pulse,
                        a.onsets[2].pulse,
                        a.bassline,
                        a.melody,
                    ]
                },
                dropping: state == "drop",
            };
            stage.view.build(&mut stage.ui, &model);
        }
        stage.render(gpu, &target_view, (w, h), dt, with_ui);
        if i == first
            && let Some(started) = started
        {
            wait().map_err(|e| e.to_string())?;
            let frames = first - warm;
            timing = Some((started.elapsed().as_secs_f32() * 1000.0 / frames as f32, frames));
        }
        if o.motion && i >= warm && i <= first {
            // Reading every frame back is slow, which is why this is opt-in.
            let now = gpu.read_texture(&target)?;
            if let Some(before) = &before {
                motion.push(difference(before, &now));
            }
            before = Some(now);
        }
        if i >= first && (i - first).is_multiple_of(every) {
            frames.push(gpu.read_texture(&target)?);
        } else {
            let _ = gpu.device.poll(wgpu::PollType::Poll);
        }
    }

    let a = &stage.analysis;
    let [kick, snare, hat] = &a.onsets;
    let summary = format!(
        "bass {:.2} mid {:.2} treb {:.2} level {:.2} beats {} quads {}\n  \
         voices: kick {} snare {} hats {} notes {}, bass line {:.2}, tune {:.2} at pitch {:.2}\n  \
         centroid {:.2}, energy {:.2}, harmony {:.2}, tonal {:.2}",
        a.bass,
        a.mid,
        a.treb,
        a.level,
        a.beat.count,
        stage.ui.quads.len(),
        kick.count,
        snare.count,
        hat.count,
        a.note.count,
        a.bassline,
        a.melody,
        a.pitch,
        a.centroid,
        a.energy,
        a.harmony,
        a.tonal,
    );
    Ok(Take {
        frames,
        timing,
        motion,
        summary,
    })
}

/// Save tightly packed RGBA as a PNG, or as a JPEG if the name says so.
fn save(path: &str, rgba: &[u8], w: u32, h: u32) -> Result<(), String> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        let rgb: Vec<u8> = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|px| [px[0], px[1], px[2]])
            .collect();
        let file = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
        let mut out = std::io::BufWriter::new(file);
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
            .encode(&rgb, w, h, image::ExtendedColorType::Rgb8)
            .map_err(|e| format!("{path}: {e}"))
    } else {
        image::save_buffer(path, rgba, w, h, image::ExtendedColorType::Rgba8).map_err(|e| format!("{path}: {e}"))
    }
}

pub fn run(o: &Opts, path: &str) -> Result<(), String> {
    let gpu = Gpu::headless()?;
    let (w, h) = o.size;
    let mut preset = presets::index_of(&o.preset);
    let pal = palette::index_of(&o.palette);

    // A preset file stands in for the built-in preset it is named after, or
    // failing that for the one chosen with --preset.
    let body = match &o.preset_file {
        Some(file) => {
            let named = std::path::Path::new(file).file_stem().and_then(|s| s.to_str());
            if o.preset_arg.is_none()
                && let Some(i) = PRESETS.iter().position(|p| Some(p.id) == named)
            {
                preset = i;
            }
            Some(std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?)
        }
        None => None,
    };
    let body = body.as_deref();

    // Each cell of the sheet: its frame, from one take or from several.
    let every = ((o.every * o.fps).round() as usize).max(1);
    let solo = o.solo;
    // Down each column of a voices sheet: four moments across a beat.
    const VOICE_ROWS: u32 = 4;
    let (cols, takes) = match o.sheet {
        None => (1, vec![take(&gpu, o, preset, pal, body, solo, 1, 1)?]),
        Some(Sheet::Times { cols, rows }) => (
            cols,
            vec![take(
                &gpu,
                o,
                preset,
                pal,
                body,
                solo,
                (cols * rows).max(1) as usize,
                every,
            )?],
        ),
        Some(Sheet::Palettes) => (
            4,
            (0..palette::PALETTES.len())
                .map(|pal| take(&gpu, o, preset, pal, body, solo, 1, 1))
                .collect::<Result<_, _>>()?,
        ),
        Some(Sheet::Voices) => (
            VOICES.len() as u32,
            (0..VOICES.len())
                .map(|voice| take(&gpu, o, preset, pal, body, Some(voice), VOICE_ROWS as usize, every))
                .collect::<Result<_, _>>()?,
        ),
        Some(Sheet::Presets) => (
            // The chosen preset's genre, up to five to a row.
            presets::in_genre(PRESETS[preset].genre).count().min(5) as u32,
            presets::in_genre(PRESETS[preset].genre)
                .map(|i| {
                    let pal = match o.palette_arg {
                        Some(_) => pal,
                        None => palette::index_of(PRESETS[i].showcase),
                    };
                    take(&gpu, o, i, pal, body.filter(|_| i == preset), solo, 1, 1)
                })
                .collect::<Result<_, _>>()?,
        ),
    };

    let cells: Vec<&Vec<u8>> = takes.iter().flat_map(|t| &t.frames).collect();
    if o.sheet.is_none() {
        save(path, cells[0], w, h)?;
    } else {
        let cols = cols.max(1);
        let rows = (cells.len() as u32).div_ceil(cols);
        let (sw, sh) = (cols * (w + GUTTER) + GUTTER, rows * (h + GUTTER) + GUTTER);
        let mut sheet: Vec<u8> = [16, 12, 22, 255].repeat((sw * sh) as usize);
        // A voices sheet runs down its columns; the others along their rows.
        let down = o.sheet == Some(Sheet::Voices);
        for (i, cell) in cells.iter().enumerate() {
            let i = i as u32;
            let (col, row) = if down {
                (i / rows, i % rows)
            } else {
                (i % cols, i / cols)
            };
            let x0 = GUTTER + col * (w + GUTTER);
            let y0 = GUTTER + row * (h + GUTTER);
            for (y, row) in cell.chunks_exact((w * 4) as usize).enumerate() {
                let at = (((y0 + y as u32) * sw + x0) * 4) as usize;
                sheet[at..at + row.len()].copy_from_slice(row);
            }
        }
        save(path, &sheet, sw, sh)?;
    }

    println!("{path}: {}", takes.last().map_or("", |t| t.summary.as_str()));
    // With several takes, the slowest is the one worth knowing about.
    let slowest = takes.iter().filter_map(|t| t.timing).max_by(|a, b| a.0.total_cmp(&b.0));
    match slowest {
        Some((ms, frames)) => println!("  {ms:.2} ms/frame at {w}x{h} (GPU time, mean of {frames} frames)"),
        None if o.motion => {}
        None => println!("  too few frames to time: raise --seconds"),
    }
    // How restless the picture is: its typical frame-to-frame change, and
    // how far the biggest jumps (the hits) stand above that.
    let mut motion: Vec<f32> = takes.iter().flat_map(|t| t.motion.iter().copied()).collect();
    if !motion.is_empty() {
        motion.sort_by(f32::total_cmp);
        let at = |q: f32| motion[((motion.len() - 1) as f32 * q) as usize];
        println!(
            "  motion per frame (0-255): median {:.2}, 95th percentile {:.2}, peak {:.2}",
            at(0.5),
            at(0.95),
            at(1.0)
        );
    }
    Ok(())
}

/// Save every logo variant, at the sizes it is used, on dark and light
/// backgrounds. CPU only: handy for reviewing the logo or exporting it.
pub fn logo_sheet(path: &str) -> Result<(), String> {
    use crate::ui::logo::{self, Logo};
    let (w, h) = (1500u32, 860u32);
    let mut img = vec![0u8; (w * h * 4) as usize];
    for (i, px) in img.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        // Dark like the app on the left, paper on the right of the bottom rows.
        let (x, y) = (i as u32 % w, i as u32 / w);
        let light = y > 400 && x > 900;
        px.copy_from_slice(if light {
            &[244, 241, 234, 255]
        } else {
            &[16, 12, 22, 255]
        });
    }
    let mut place = |logo: Logo, x: u32, y: u32, lw: u32, lh: u32, angle: f32| {
        let src = logo::rasterize(logo, lw, lh, angle);
        for sy in 0..lh.min(h.saturating_sub(y)) {
            for sx in 0..lw.min(w.saturating_sub(x)) {
                let s = &src[((sy * lw + sx) * 4) as usize..][..4];
                let d = &mut img[(((y + sy) * w + x + sx) * 4) as usize..][..4];
                let a = s[3] as f32 / 255.0;
                for c in 0..3 {
                    d[c] = (s[c] as f32 * a + d[c] as f32 * (1.0 - a) + 0.5) as u8;
                }
            }
        }
    };
    let wide = |logo: Logo, lh: u32| ((lh as f32 * logo::aspect(logo)).round() as u32, lh);

    let (fw, fh) = wide(Logo::Full, 300);
    place(Logo::Full, (w - fw) / 2, 50, fw, fh, 0.0);
    let mut x = 40;
    for lh in [120, 54, 36, 24] {
        let (cw, ch) = wide(Logo::Compact, lh);
        place(Logo::Compact, x, 430 + (120 - ch) / 2, cw, ch, 0.0);
        x += cw + 28;
    }
    let mut x = 40;
    for size in [256, 128, 64, 48, 32, 16] {
        place(Logo::Mark, x, 590 + (256 - size) / 2, size, size, -0.5);
        x += size + 28;
    }
    let (cw, ch) = wide(Logo::Compact, 96);
    place(Logo::Compact, 940, 440, cw, ch, 0.0);
    let (mw, mh) = wide(Logo::Mark, 110);
    place(Logo::Mark, 1280, 433, mw, mh, 0.0);
    place(Logo::Mark, 960, 600, 200, 200, -0.5);
    place(Logo::Mark, 1200, 668, 64, 64, -0.5);
    place(Logo::Mark, 1300, 684, 32, 32, -0.5);
    image::save_buffer(path, &img, w, h, image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())
}
