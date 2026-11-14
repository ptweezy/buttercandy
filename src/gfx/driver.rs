//! CPU companions to presets.
//!
//! A preset is a fragment shader, and a fragment shader has 32-bit floats and
//! no memory of its own. A preset that needs more than that (64-bit
//! arithmetic, a path worked out ahead of time) has a driver: a piece of Rust
//! that runs once a frame, just before the preset is drawn, and hands the
//! shader forty numbers (`uDrive`) and a table (`driveData`). The
//! headless renderer runs drivers too, so a preset with one can be shot and
//! tested like any other.

use crate::audio::analysis::Analysis;

/// The table is a texture this many texels wide and high, four floats each.
pub const DATA_W: u32 = 1024;
pub const DATA_H: u32 = 16;
/// Floats in the whole table.
pub const DATA_LEN: usize = (DATA_W * DATA_H * 4) as usize;
/// `uDrive` is this many vec4s.
pub const PARAMS: usize = 10;

pub trait Driver {
    /// Advance by `dt` seconds of the music in `analysis`. Called once for
    /// every frame the preset is drawn in, and not otherwise: a preset that
    /// is not on screen stands still.
    fn update(&mut self, analysis: &Analysis, dt: f32);

    /// What the shader reads as `uDrive[0]` to `uDrive[9]` this frame.
    fn params(&self) -> [[f32; 4]; PARAMS];

    /// The whole table (`DATA_LEN` floats, row by row), if it has changed
    /// since it was last taken.
    fn take_data(&mut self) -> Option<&[f32]>;

    /// The preset is about to be drawn somewhere that has not seen it: the
    /// table is wanted again in full, and whatever state the shader kept is
    /// gone.
    fn resend(&mut self);

    /// Take a different path from the one every run starts on. The windowed
    /// app calls this once; headless runs do not, so they repeat exactly.
    fn shuffle(&mut self, _seed: u64) {}
}

/// The driver of the preset called `id`, if it has one.
pub fn for_preset(id: &str) -> Option<Box<dyn Driver>> {
    match id {
        "jawbreaker" => Some(Box::new(super::jawbreaker::Dive::new())),
        _ => None,
    }
}
