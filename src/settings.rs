//! User preferences, persisted as JSON in the platform config directory.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The slowest and fastest frame rates the picture can be held to.
pub const MIN_TARGET_FPS: u32 = 15;
pub const MAX_TARGET_FPS: u32 = 1000;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub preset: String,
    pub palette: String,
    pub volume: f32,
    pub muted: bool,
    /// Gain applied to the analysis, on top of automatic gain control.
    pub sensitivity: f32,
    /// How hard the visuals react to the music, 0 (gentle) to 1 (every hit).
    pub reactivity: f32,
    /// Bloom amount.
    pub glow: f32,
    /// How frosted the interface's glass is: 0 is clear, 1 clouded and
    /// nearly opaque. Half way is each pane as it was designed.
    pub glass_frost: f32,
    /// Seconds between automatic preset changes; 0 disables cycling.
    pub auto_cycle: u32,
    /// Graphics quality: the internal render resolution as a fraction of
    /// the window's.
    pub render_scale: f32,
    /// Frames per second to hold the picture to. 0 sets no target, which
    /// leaves the pace to the display, as does a target the display is too
    /// slow for (see [`max_target_fps`]).
    pub target_fps: u32,
    pub show_fps: bool,
    /// Show the voices key: one lamp per thing the analyser listens for.
    pub show_key: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preset: "taffy".into(),
            palette: "buttercandy".into(),
            volume: 0.8,
            muted: false,
            sensitivity: 1.0,
            reactivity: 0.5,
            glow: 0.7,
            glass_frost: 0.5,
            auto_cycle: 0,
            render_scale: 1.0,
            target_fps: 0,
            show_fps: false,
            show_key: false,
        }
    }
}

/// The fastest frame rate that can be held on a display refreshing at `hz`.
///
/// The display hands out one turn to draw per refresh, and a frame that is
/// ready a moment after its turn waits a whole refresh for the next. A timer
/// running only a little slower than the display keeps landing there, and
/// the picture falls well short of the target: asked for 230 on a 240 Hz
/// display it ran at 208, and asked for 239, at 180. So a target stops at
/// 85% of the display's rate, with frames at least 0.75 ms longer than the
/// display's; 200 on that display, and anything slower, is held exactly.
pub fn max_target_fps(hz: u32) -> u32 {
    let hz = hz.max(1);
    let by_share = hz * 85 / 100;
    let by_time = (1.0 / (1.0 / hz as f64 + 0.00075)) as u32;
    by_share.min(by_time).max(MIN_TARGET_FPS)
}

/// A frame rate target brought into range, leaving 0 (no target) alone.
pub fn clamp_target_fps(fps: u32) -> u32 {
    if fps == 0 {
        0
    } else {
        fps.clamp(MIN_TARGET_FPS, MAX_TARGET_FPS)
    }
}

fn path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("buttercandy").join("settings.json"))
}

impl Settings {
    pub fn load() -> Self {
        let loaded = path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<Settings>(&s).ok());
        let mut s = loaded.unwrap_or_default();
        s.volume = s.volume.clamp(0.0, 1.0);
        s.sensitivity = s.sensitivity.clamp(0.25, 3.0);
        s.reactivity = s.reactivity.clamp(0.0, 1.0);
        s.glow = s.glow.clamp(0.0, 2.0);
        s.glass_frost = s.glass_frost.clamp(0.0, 1.0);
        s.render_scale = s.render_scale.clamp(0.25, 1.0);
        s.target_fps = clamp_target_fps(s.target_fps);
        s
    }

    pub fn save(&self) {
        let Some(path) = path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    log::warn!("could not save settings to {}: {e}", path.display());
                }
            }
            Err(e) => log::warn!("could not serialise settings: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_stop_short_of_the_display() {
        assert_eq!(max_target_fps(60), 51);
        assert_eq!(max_target_fps(144), 122);
        // Here it is the 0.75 ms that counts.
        assert_eq!(max_target_fps(240), 203);
        // Never below the slowest target there is.
        assert_eq!(max_target_fps(10), MIN_TARGET_FPS);
        assert_eq!(max_target_fps(0), MIN_TARGET_FPS);
    }

    #[test]
    fn no_target_stays_no_target() {
        assert_eq!(clamp_target_fps(0), 0);
        assert_eq!(clamp_target_fps(5), MIN_TARGET_FPS);
        assert_eq!(clamp_target_fps(60), 60);
        assert_eq!(clamp_target_fps(u32::MAX), MAX_TARGET_FPS);
    }
}
