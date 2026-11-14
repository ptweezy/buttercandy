//! buttercandy: a music visualizer.
//!
//! Copyright (C) 2026 Parker Loflin. Free software under the GNU Affero
//! General Public License, version 3: see LICENSE.
//! SPDX-License-Identifier: AGPL-3.0-only

// Release builds are a GUI app: no console window on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
// Drawing calls take a rectangle, a few metrics and a couple of colours;
// bundling those into structs would only obscure the call sites.
#![allow(clippy::too_many_arguments)]

mod app;
mod audio;
#[cfg(target_os = "linux")]
mod desktop;
mod gfx;
mod library;
mod palette;
mod settings;
mod shot;
mod stage;
mod ui;

use std::path::PathBuf;

const USAGE: &str = "\
buttercandy [options] [files or folders...]

  --preset <id>        start on a preset (taffy, skyline, corona, wormhole,
                       synthgrid, ribbon, kaleido, sprinkles, gumdrop,
                       sugarplum; or a fractal: jawbreaker, gumball,
                       gingerbread)
  --palette <id>       start with a palette: buttercandy, aurora, synthwave,
                       ember, lagoon, sherbet, toxic, noir, bubblegum, citrus,
                       peppermint, caramel, jellybean, moonlight, frost,
                       inferno
  --system             visualize system audio (what happens by default)
  --demo               start the built-in demo track instead
  --mic                start visualizing the default input device instead
  --idle               start with nothing playing, on the landing screen
  --volume <0..1>      playback volume
  --reactivity <0..1>  how hard the visuals react to the music (the - and =
                       keys change it while running)
  --target-fps <n>     hold the picture to n frames a second; 0, or a rate
                       the display is too slow for, leaves the pace to the
                       display
  --fullscreen         start fullscreen
  --version            print the version, and exit
  --exit-after <secs>  quit after a while (for smoke tests)
  --window-shot <png>  with --exit-after: save the last frame the window
                       presented, interface included

Headless rendering (no window, no audio device):
  --shot <file.png>    render the demo track offscreen and save one frame
  --size <WxH>         image size (default 1600x900)
  --scale <factor>     interface scale factor (default 1)
  --seconds <n>        seconds to render before the capture (default 4)
  --skip <n>           seconds of music to skip first (default 12)
  --ui <state>         interface to draw: none, hero, demo, system, player,
                       queue, palette, settings, help, drop, key (default
                       none)
  --mouse <x,y>        pointer position, in logical pixels
  --toast <text>       show a toast
  --silent             render without audio (the idle animation)
  --preset-file <wgsl> render this preset file instead of the built-in preset
                       of the same name (or the one given with --preset);
                       a compile error is printed and exits non-zero
  --sheet <COLSxROWS>  save a contact sheet of successive frames instead of
                       one, each --size, so a single image shows motion
  --every <secs>       time between the frames of a sheet (default 0.8)
  --fps <n>            frame rate to simulate (default 60): the picture should
                       not depend on it
  --motion             also measure how much the picture changes from frame
                       to frame over the --seconds before the capture
  --sheet palettes     a sheet of the same moment in every palette
  --sheet presets      a sheet of the same moment in every preset of the
                       chosen preset's genre
  --solo <voice>       play only one voice of the demo track: kick, snare,
                       hats, bass or tune
  --sheet voices       a sheet with one column per voice, each playing alone,
                       and four moments --every apart down each column
  --logo-sheet <png>   save the logo at every size it is used, and exit
  --icon <png>         save the app icon, a square as wide as the smaller
                       side of --size, and exit (packaging builds the macOS
                       icon from it)

Linux:
  --install-desktop    add buttercandy to the applications menu, with its
                       icon (without this the dock shows a generic one)
  --uninstall-desktop  remove it again
";

pub struct Opts {
    // Windowed.
    pub files: Vec<PathBuf>,
    pub preset_arg: Option<String>,
    pub palette_arg: Option<String>,
    pub source: Option<String>,
    pub volume: Option<f32>,
    pub reactivity: Option<f32>,
    pub target_fps: Option<u32>,
    pub fullscreen: bool,
    pub exit_after: Option<f32>,
    pub window_shot: Option<String>,
    // Headless.
    pub shot: Option<String>,
    pub preset: String,
    pub palette: String,
    pub size: (u32, u32),
    pub scale: f32,
    pub seconds: f32,
    pub skip: f32,
    pub ui: String,
    pub mouse: Option<(f32, f32)>,
    pub toast: String,
    pub silent: bool,
    pub preset_file: Option<String>,
    pub sheet: Option<shot::Sheet>,
    pub every: f32,
    pub fps: f32,
    pub motion: bool,
    pub solo: Option<usize>,
    pub logo_sheet: Option<String>,
    /// Save the app icon here, and exit.
    pub icon: Option<String>,
    /// Install (`true`) or remove (`false`) the launcher entry, and exit.
    pub desktop_entry: Option<bool>,
}

fn pair<T: std::str::FromStr>(s: &str, sep: char) -> Option<(T, T)> {
    let (a, b) = s.split_once(sep)?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

fn parse() -> Result<Opts, String> {
    let mut o = Opts {
        files: Vec::new(),
        preset_arg: None,
        palette_arg: None,
        source: None,
        volume: None,
        reactivity: None,
        target_fps: None,
        fullscreen: false,
        exit_after: None,
        window_shot: None,
        shot: None,
        preset: "taffy".into(),
        palette: "buttercandy".into(),
        size: (1600, 900),
        scale: 1.0,
        seconds: 4.0,
        skip: 12.0,
        ui: "none".into(),
        mouse: None,
        toast: String::new(),
        silent: false,
        preset_file: None,
        sheet: None,
        every: 0.8,
        fps: 60.0,
        motion: false,
        solo: None,
        logo_sheet: None,
        icon: None,
        desktop_entry: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        let number = |name: &str, v: String| v.parse::<f32>().map_err(|_| format!("{name}: '{v}' is not a number"));
        match arg.as_str() {
            "-h" | "--help" => return Err(USAGE.into()),
            "-V" | "--version" => {
                // A release build is stamped with its release's version.
                println!("buttercandy {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--preset" => {
                let v = value("--preset")?;
                o.preset = v.clone();
                o.preset_arg = Some(v);
            }
            "--palette" => {
                let v = value("--palette")?;
                o.palette = v.clone();
                o.palette_arg = Some(v);
            }
            "--demo" => o.source = Some("demo".into()),
            "--system" => o.source = Some("system".into()),
            "--mic" => o.source = Some("mic".into()),
            "--idle" => o.source = Some("idle".into()),
            "--volume" => o.volume = Some(number("--volume", value("--volume")?)?),
            "--reactivity" => o.reactivity = Some(number("--reactivity", value("--reactivity")?)?.clamp(0.0, 1.0)),
            "--target-fps" => o.target_fps = Some(number("--target-fps", value("--target-fps")?)?.max(0.0) as u32),
            "--fullscreen" => o.fullscreen = true,
            "--exit-after" => o.exit_after = Some(number("--exit-after", value("--exit-after")?)?),
            "--window-shot" => o.window_shot = Some(value("--window-shot")?),
            "--shot" => o.shot = Some(value("--shot")?),
            "--size" => o.size = pair(&value("--size")?, 'x').ok_or("--size expects WIDTHxHEIGHT")?,
            "--scale" => o.scale = number("--scale", value("--scale")?)?,
            "--seconds" => o.seconds = number("--seconds", value("--seconds")?)?,
            "--skip" => o.skip = number("--skip", value("--skip")?)?,
            "--ui" => o.ui = value("--ui")?,
            "--mouse" => o.mouse = Some(pair(&value("--mouse")?, ',').ok_or("--mouse expects X,Y")?),
            "--toast" => o.toast = value("--toast")?,
            "--silent" => o.silent = true,
            "--preset-file" => o.preset_file = Some(value("--preset-file")?),
            "--sheet" => {
                o.sheet = Some(match value("--sheet")?.as_str() {
                    "palettes" => shot::Sheet::Palettes,
                    "presets" => shot::Sheet::Presets,
                    "voices" => shot::Sheet::Voices,
                    grid => {
                        let (cols, rows) =
                            pair(grid, 'x').ok_or("--sheet expects COLSxROWS, 'palettes', 'presets' or 'voices'")?;
                        shot::Sheet::Times { cols, rows }
                    }
                })
            }
            "--every" => o.every = number("--every", value("--every")?)?,
            "--fps" => o.fps = number("--fps", value("--fps")?)?,
            "--motion" => o.motion = true,
            "--solo" => {
                let v = value("--solo")?;
                let voice = shot::VOICES.iter().position(|name| *name == v);
                o.solo = Some(voice.ok_or(format!("--solo: '{v}' is not one of {}", shot::VOICES.join(", ")))?);
            }
            "--logo-sheet" => o.logo_sheet = Some(value("--logo-sheet")?),
            "--icon" => o.icon = Some(value("--icon")?),
            "--install-desktop" => o.desktop_entry = Some(true),
            "--uninstall-desktop" => o.desktop_entry = Some(false),
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}\n\n{USAGE}")),
            file => o.files.push(PathBuf::from(file)),
        }
    }
    Ok(o)
}

/// Release builds on Windows are GUI-subsystem programs with no console of
/// their own. When started from a terminal, borrow that terminal so `--help`,
/// errors and `--shot` output are still visible.
#[cfg(all(windows, not(debug_assertions)))]
fn attach_parent_console() {
    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    // SAFETY: a plain Win32 call with no pointers; failure (no parent
    // console, or output already redirected) is harmless and ignored.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn attach_parent_console() {}

#[cfg(target_os = "linux")]
fn desktop_entry(install: bool) -> Result<(), String> {
    if install {
        desktop::install()
    } else {
        desktop::uninstall()
    }
}

#[cfg(not(target_os = "linux"))]
fn desktop_entry(_install: bool) -> Result<(), String> {
    Err("--install-desktop is for Linux; other systems take the icon from the window".into())
}

/// Save the app icon as a square PNG, `side` pixels wide: the same drawing
/// the window and the Linux launcher entry use. The macOS bundle's icon is
/// built from this, so it cannot drift from the logo.
fn save_icon(path: &str, side: u32) -> Result<(), String> {
    use ui::logo::{self, Logo};
    if side == 0 {
        return Err("--icon needs a --size of at least 1x1".into());
    }
    let rgba = logo::rasterize(Logo::Mark, side, side, -0.5);
    image::save_buffer(path, &rgba, side, side, image::ExtendedColorType::Rgba8).map_err(|e| format!("{path}: {e}"))
}

fn main() {
    attach_parent_console();
    // The PulseAudio client logs an error each time a stream is closed.
    let filter = "warn,buttercandy=info,pulseaudio::client::reactor=off";
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(filter)).init();
    let opts = match parse() {
        Ok(o) => o,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };
    let result = if let Some(install) = opts.desktop_entry {
        desktop_entry(install)
    } else if let Some(path) = opts.icon.clone() {
        save_icon(&path, opts.size.0.min(opts.size.1))
    } else if let Some(path) = opts.logo_sheet.clone() {
        shot::logo_sheet(&path)
    } else if let Some(path) = opts.shot.clone() {
        shot::run(&opts, &path)
    } else {
        app::run(opts)
    };
    if let Err(e) = result {
        eprintln!("buttercandy: {e}");
        std::process::exit(1);
    }
}
