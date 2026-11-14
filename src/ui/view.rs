//! The buttercandy interface: everything drawn over the visualizer.
//!
//! [`View::build`] lays the whole thing out from a read-only [`Model`] each
//! frame and returns the [`Action`]s the user triggered.

use super::icons::Icon;
use super::logo::{self, Logo};
use super::{Align, BOLD, CLEAR, Color, Glass, Hit, MEDIUM, REGULAR, Rect, SEMIBOLD, Ui, alpha, hex, id, mix, white};
use crate::audio::Source;
use crate::gfx::presets::{self, GENRES, PRESETS};
use crate::palette::PALETTES;
use crate::settings::{MIN_TARGET_FPS, Settings, max_target_fps};

pub struct TrackRow {
    pub title: String,
    pub artist: String,
    pub duration: Option<f64>,
}

/// Everything the interface needs to know to draw one frame.
pub struct Model<'a> {
    pub source: Source,
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    pub title: &'a str,
    pub subtitle: &'a str,
    pub has_art: bool,
    pub tracks: &'a [TrackRow],
    pub current: Option<usize>,
    pub preset: usize,
    pub palette: usize,
    pub settings: &'a Settings,
    pub fullscreen: bool,
    pub fps: f32,
    /// The display's refresh rate in Hz, where the system says what it is.
    pub display_hz: Option<u32>,
    /// Log-frequency spectrum, one byte per bin.
    pub spectrum: &'a [u8],
    pub beat: f32,
    /// How strongly each voice the analyser listens for is sounding, 0..1:
    /// kick, snare, hats, bass line, tune.
    pub voices: [f32; 5],
    /// Files are being dragged over the window.
    pub dropping: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    SetPreset(usize),
    StepPreset(i32),
    /// Switch to the next (or previous) genre of presets.
    StepGenre(i32),
    SetPalette(usize),
    OpenFiles,
    StartSystem,
    StartMic,
    StartDemo,
    TogglePlay,
    Next,
    Prev,
    /// Seek to a fraction of the track.
    Seek(f32),
    SetVolume(f32),
    ToggleMute,
    ToggleFullscreen,
    ToggleAutoCycle,
    SetAutoCycle(u32),
    SetSensitivity(f32),
    /// How strongly the presets react to the music, 0..1.
    SetReactivity(f32),
    SetGlow(f32),
    /// How frosted the interface's glass is, 0 (clear) to 1.
    SetGlassFrost(f32),
    SetRenderScale(f32),
    /// Frames per second to hold the picture to; 0 for no target.
    SetTargetFps(u32),
    ToggleFps,
    PlayIndex(usize),
    RemoveIndex(usize),
    ClearQueue,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Popover {
    None,
    Palette,
    Settings,
    Help,
}

pub struct View {
    pub popover: Popover,
    pub queue_open: bool,
    /// Keep the chrome on screen regardless of pointer activity.
    pub pin: bool,
    /// Keep the chrome off screen (toggled with H).
    pub hidden: bool,
    queue_scroll: f32,
    toasts: Vec<(String, f32)>,
    idle: f32,
    last_mouse: (f32, f32),
    over_chrome: bool,
    /// The pointer should be hidden (chrome has faded out).
    pub hide_cursor: bool,
}

const TEXT: Color = [0.98, 0.97, 1.0, 1.0];
const INK: Color = [0.10, 0.06, 0.09, 1.0];
const TOAST_LIFE: f32 = 2.6;
/// The palette picker lays its palettes out in two columns: the width of
/// one, and the height of a row.
const PALETTE_COL: f32 = 236.0;
const PALETTE_ROW: f32 = 36.0;

fn dim(a: f32) -> Color {
    [0.98, 0.97, 1.0, a]
}

// ---- glass -------------------------------------------------------------------
// Everything in the interface is cut from one of these kinds of glass. They
// share one shape: flat on top, with a narrow rounded lip that does the
// bending, so the differences are only in how sanded and how tinted each is.

/// The big panes that carry text: the dock, the queue, the landing cards.
/// The film is a neutral grey a shade above black, so the pane stands clear
/// of a dark picture and still quietens a bright one.
fn pane() -> Glass {
    Glass::new(hex(0x26252c, 0.50), white(0.36))
        .frost(0.3)
        .bevel(16.0)
        .bend(2.1)
        .shadow(0.8)
}

/// Deeper and more sanded, for reading closely: popovers, help, toasts.
fn reading_pane() -> Glass {
    Glass::new(hex(0x222128, 0.64), white(0.36))
        .frost(0.42)
        .bevel(16.0)
        .bend(2.1)
        .shadow(0.9)
}

/// Clearer glass for controls: round buttons and the tab bar. It thickens a
/// little under the pointer (`hover`, 0..1), bending more, and flattens
/// when pressed (`press`).
fn lens(hover: f32, press: f32) -> Glass {
    Glass::new(
        mix(hex(0x2a2930, 0.40), hex(0x3c3b45, 0.44), hover),
        white(0.36 + 0.2 * hover),
    )
    .frost(0.16)
    .bevel(13.0)
    .bend(2.0 + 0.4 * hover - 0.9 * press)
    .glow(hover)
    .shadow(0.5)
}

/// Glass tinted with the palette's two accent colours, left to right.
fn stained(a1: Color, a2: Color) -> Glass {
    Glass::new(alpha(a1, 0.88), white(0.5))
        .gradient(alpha(a2, 0.88))
        .frost(0.3)
        .bevel(10.0)
        .bend(1.6)
        .solid()
}

/// White glass: slider knobs and the play button.
fn bead() -> Glass {
    Glass::new(white(0.92), white(0.8))
        .frost(0.3)
        .bevel(8.0)
        .bend(1.4)
        .shadow(0.6)
        .solid()
}

fn fmt_time(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// Layout pass state: the toolkit, the model, and the actions collected.
struct Cx<'a> {
    ui: &'a mut Ui,
    m: &'a Model<'a>,
    out: Vec<Action>,
    a1: Color,
    a2: Color,
    tooltip: Option<(&'static str, Rect, bool)>,
}

impl Cx<'_> {
    fn hover_anim(&mut self, wid: u64, hit: &Hit) -> (f32, f32) {
        let h = self.ui.anim(wid ^ 0x11, hit.hover as u32 as f32, 18.0);
        let p = self.ui.anim(wid ^ 0x22, hit.held as u32 as f32, 28.0);
        (h, p)
    }

    /// Show `tip` near `r` once the pointer has rested there a moment.
    fn tip(&mut self, wid: u64, hit: &Hit, r: Rect, tip: &'static str, below: bool) {
        let t = self
            .ui
            .anim_from(wid ^ 0x33, 0.0, if hit.hover && !hit.held { 1.0 } else { 0.0 }, 3.0);
        if hit.hover && t > 0.7 && !tip.is_empty() {
            self.tooltip = Some((tip, r, below));
        }
        if !hit.hover {
            self.ui.reset_anim(wid ^ 0x33);
        }
    }

    /// A round icon button. `fill` picks the look: glass disc, bare, or accent.
    fn icon_button(
        &mut self,
        key: &str,
        cx: f32,
        cy: f32,
        size: f32,
        icon: Icon,
        tip: &'static str,
        style: Style,
        enabled: bool,
    ) -> bool {
        let wid = id(key, 0);
        let r = Rect::new(cx - size * 0.5, cy - size * 0.5, size, size);
        let hit = if enabled {
            self.ui.interact(wid, r)
        } else {
            Hit::default()
        };
        let (h, p) = self.hover_anim(wid, &hit);
        let grow = size * (0.03 * h - 0.05 * p);
        let rr = r.inset(-grow);
        let icon_size = (size * 0.52).round();
        let fade = if enabled { 1.0 } else { 0.32 };
        match style {
            Style::Glass => {
                self.ui.glass(rr, size, lens(h, p));
                self.ui.icon(icon, cx, cy, icon_size, dim((0.82 + 0.18 * h) * fade));
            }
            Style::Bare => {
                self.ui.rect(rr, size, white(0.10 * h + 0.05 * p));
                self.ui.icon(icon, cx, cy, icon_size, dim((0.72 + 0.28 * h) * fade));
            }
            Style::Accent => {
                self.ui.shadow(rr, size, 10.0, alpha(self.a2, 0.35 + 0.2 * h));
                self.ui.glass(rr, size, stained(self.a1, self.a2).glow(h));
                self.ui.icon(icon, cx, cy, icon_size, INK);
            }
            Style::Solid => {
                self.ui
                    .shadow(rr, size, 14.0, alpha(self.a2, 0.25 + 0.45 * self.m.beat));
                self.ui.glass(rr, size, bead().glow(h).shadow(0.0));
                self.ui.icon(icon, cx, cy, icon_size, INK);
            }
        }
        self.tip(wid, &hit, r, tip, cy < 120.0);
        hit.clicked
    }

    /// A horizontal slider over 0..1. Returns the new value while dragging.
    fn slider(&mut self, key: &str, r: Rect, value: f32) -> Option<f32> {
        self.stepped_slider(key, r, value, 0)
    }

    /// A slider that stops only at `steps` equal intervals (0 leaves it
    /// smooth). The wheel moves a stepped one a single step at a time, for
    /// landing on an exact value. Returns the new value when it changes.
    fn stepped_slider(&mut self, key: &str, r: Rect, value: f32, steps: u32) -> Option<f32> {
        let wid = id(key, 0);
        let hit = self
            .ui
            .interact(wid, Rect::new(r.x - 6.0, r.cy() - 12.0, r.w + 12.0, 24.0));
        let (h, p) = self.hover_anim(wid, &hit);
        let n = steps as f32;
        let snap = |v: f32| if steps > 0 { (v * n).round() / n } else { v };
        let mut out = None;
        let mut v = value.clamp(0.0, 1.0);
        let mut nv = v;
        if hit.held {
            nv = snap(((self.ui.input.mouse.0 - r.x) / r.w).clamp(0.0, 1.0));
        } else if hit.hover && steps > 0 {
            self.ui.wheel += self.ui.input.scroll;
            let notches = self.ui.wheel.trunc();
            self.ui.wheel -= notches;
            if notches != 0.0 {
                nv = (snap(v) + notches / n).clamp(0.0, 1.0);
            }
        }
        if (nv - v).abs() > 1e-4 {
            out = Some(nv);
            v = nv;
        }
        let track = Rect::new(r.x, r.cy() - 2.0, r.w, 4.0);
        self.ui.rect(track, 2.0, white(0.16));
        if v > 0.0 {
            self.ui.gradient(
                Rect::new(track.x, track.y, track.w * v, track.h),
                2.0,
                self.a1,
                self.a2,
                false,
            );
        }
        // The knob is a bead of glass, which swells when it is picked up.
        let k = 13.0 + 3.0 * h + 4.0 * p;
        let knob = Rect::new(r.x + r.w * v - k * 0.5, r.cy() - k * 0.5, k, k);
        self.ui.glass(knob, k, bead().glow(h));
        out
    }

    /// A row of mutually exclusive choices. Returns the index clicked.
    fn segmented(&mut self, key: &str, r: Rect, labels: &[&str], selected: Option<usize>) -> Option<usize> {
        self.ui.rect(r, r.h * 0.5, white(0.08));
        let w = r.w / labels.len() as f32;
        let mut clicked = None;
        for (i, label) in labels.iter().enumerate() {
            let cell = Rect::new(r.x + w * i as f32, r.y, w, r.h);
            let wid = id(key, i);
            let hit = self.ui.interact(wid, cell);
            let (h, _) = self.hover_anim(wid, &hit);
            let on = selected == Some(i);
            if on {
                self.ui
                    .glass(cell.inset(2.0), r.h, stained(self.a1, self.a2).shadow(0.35));
            } else if h > 0.0 {
                self.ui.rect(cell.inset(2.0), r.h, white(0.08 * h));
            }
            self.ui.label(
                label,
                cell,
                12.5,
                SEMIBOLD,
                if on { INK } else { dim(0.7 + 0.3 * h) },
                Align::Center,
            );
            if hit.clicked {
                clicked = Some(i);
            }
        }
        clicked
    }
}

#[derive(Clone, Copy)]
enum Style {
    Glass,
    Bare,
    Accent,
    Solid,
}

impl View {
    pub fn new() -> Self {
        Self {
            popover: Popover::None,
            queue_open: false,
            pin: false,
            hidden: false,
            queue_scroll: 0.0,
            toasts: Vec::new(),
            idle: 0.0,
            last_mouse: (-1.0, -1.0),
            over_chrome: false,
            hide_cursor: false,
        }
    }

    /// Show a short message above the dock.
    pub fn toast(&mut self, text: impl Into<String>) {
        let text = text.into();
        // Repeating the same message (e.g. holding a key) just refreshes it.
        if let Some(last) = self.toasts.last_mut()
            && last.0 == text
        {
            last.1 = last.1.min(0.2);
            return;
        }
        self.toasts.push((text, 0.0));
        if self.toasts.len() > 3 {
            self.toasts.remove(0);
        }
    }

    pub fn toggle(&mut self, p: Popover) {
        self.popover = if self.popover == p { Popover::None } else { p };
    }

    /// Any key press or pointer activity counts as "the user is here".
    pub fn wake(&mut self) {
        self.idle = 0.0;
    }

    pub fn build(&mut self, ui: &mut Ui, m: &Model) -> Vec<Action> {
        let (w, h) = ui.size;
        let dt = ui.dt;
        let pal = &PALETTES[m.palette.min(PALETTES.len() - 1)];
        let mut cx = Cx {
            ui,
            m,
            out: Vec::new(),
            a1: hex(pal.accent[0], 1.0),
            a2: hex(pal.accent[1], 1.0),
            tooltip: None,
        };
        cx.ui.frosting = m.settings.glass_frost;

        // ---- visibility: fade the chrome out while the music plays ---------
        let input = cx.ui.input;
        if input.mouse != self.last_mouse || input.down || input.scroll != 0.0 {
            self.idle = 0.0;
            self.last_mouse = input.mouse;
        } else {
            self.idle += dt;
        }
        let pinned = self.pin
            || m.source == Source::None
            || !m.playing
            || self.popover != Popover::None
            || self.queue_open
            || m.dropping
            || (self.over_chrome && input.inside);
        let show = (pinned || self.idle < 3.0) && !(self.hidden && self.popover == Popover::None);
        let vis = cx
            .ui
            .anim(id("chrome", 0), show as u32 as f32, if show { 14.0 } else { 3.0 });
        self.hide_cursor = !show && vis < 0.5;
        self.over_chrome = false;

        // ---- geometry shared between layers -------------------------------
        let pad = 20.0;
        let dock_w = (w - pad * 2.0).min(1080.0);
        let dock = Rect::new((w - dock_w) * 0.5, h - pad - 92.0, dock_w, 92.0);
        let actions_x = w - pad - 20.0; // centre of the right-most top button
        let queue = Rect::new(w - pad - 340.0, 78.0, 340.0, (dock.y - 14.0 - 78.0).max(120.0));
        let pop_w = if self.popover == Popover::Settings {
            300.0
        } else {
            8.0 + PALETTE_COL + 4.0 + PALETTE_COL + 8.0
        };
        let pop_h = if self.popover == Popover::Settings {
            476.0
        } else {
            16.0 + 26.0 + PALETTES.len().div_ceil(2) as f32 * PALETTE_ROW + 8.0
        };
        let pop = Rect::new(w - pad - pop_w, 70.0, pop_w, pop_h);

        // Clicking away from an open popover dismisses it.
        let top_buttons = Rect::new(actions_x - 48.0 * 3.0 - 24.0, 14.0, 48.0 * 4.0, 48.0);
        match self.popover {
            Popover::Palette | Popover::Settings => {
                if input.pressed && !pop.contains(input.mouse) && !top_buttons.contains(input.mouse) {
                    self.popover = Popover::None;
                }
            }
            Popover::Help => {
                if input.pressed {
                    self.popover = Popover::None;
                }
            }
            Popover::None => {}
        }
        match self.popover {
            Popover::Palette | Popover::Settings => cx.ui.blockers.push(pop),
            Popover::Help => cx.ui.blockers.push(Rect::new(0.0, 0.0, w, h)),
            Popover::None => {}
        }

        cx.ui.alpha = vis;
        if vis > 0.004 {
            // Scrims keep the chrome legible over bright visuals.
            cx.ui
                .gradient(Rect::new(0.0, 0.0, w, 150.0), 0.0, [0.0, 0.0, 0.0, 0.5], CLEAR, true);
            cx.ui.gradient(
                Rect::new(0.0, h - 210.0, w, 210.0),
                0.0,
                CLEAR,
                [0.0, 0.0, 0.0, 0.55],
                true,
            );

            let landing = m.source == Source::None && !self.queue_open;
            if landing {
                // Quieten the visuals behind the landing screen.
                cx.ui.rect(Rect::new(0.0, 0.0, w, h), 0.0, [0.02, 0.01, 0.04, 0.34]);
            }
            self.top_bar(&mut cx, w, pad, actions_x);
            if landing {
                self.hero(&mut cx, w, dock.y);
            }
            self.dock(&mut cx, dock);
            if self.queue_open {
                self.queue(&mut cx, queue);
            }
            if m.settings.show_fps {
                let label = format!("{:.0} fps", m.fps);
                cx.ui.text(&label, pad + 2.0, 66.0, 12.0, MEDIUM, dim(0.55));
            }

            cx.ui.blockers.clear();
            match self.popover {
                Popover::Palette => self.palette_popover(&mut cx, pop),
                Popover::Settings => self.settings_popover(&mut cx, pop),
                Popover::Help => self.help(&mut cx, w, h),
                Popover::None => {}
            }
            if let Some((tip, r, below)) = cx.tooltip.take() {
                tooltip(cx.ui, tip, r, below, w);
            }
        }
        cx.ui.blockers.clear();

        // Chrome the pointer is resting on should not fade away.
        let m_pos = input.mouse;
        self.over_chrome =
            m_pos.1 < 76.0 || dock.inset(-10.0).contains(m_pos) || (self.queue_open && queue.contains(m_pos));

        // ---- always-visible layers ------------------------------------------
        voices_key(&mut cx, dock.y);
        cx.ui.alpha = 1.0;
        self.draw_toasts(&mut cx, w, dock.y, dt);
        if m.dropping {
            drop_overlay(&mut cx, w, h);
        }
        cx.out
    }

    // ---------------------------------------------------------------------

    fn top_bar(&mut self, cx: &mut Cx, w: f32, pad: f32, actions_x: f32) {
        let cy = 38.0;
        let m = cx.m;

        // Brand: the butter-stick sweet.
        let (a1, a2) = (cx.a1, cx.a2);
        let brand_h = 36.0;
        let brand = Rect::new(
            pad - 4.0,
            cy - brand_h * 0.5,
            brand_h * logo::aspect(Logo::Compact),
            brand_h,
        );
        cx.ui.logo(Logo::Compact, brand);
        let brand_right = brand.right();

        // Right-hand buttons.
        let auto_on = m.settings.auto_cycle > 0;
        let xs = [actions_x - 144.0, actions_x - 96.0, actions_x - 48.0, actions_x];
        if cx.icon_button(
            "auto",
            xs[0],
            cy,
            40.0,
            Icon::Sparkles,
            "Auto-cycle presets (C)",
            if auto_on { Style::Accent } else { Style::Glass },
            true,
        ) {
            cx.out.push(Action::ToggleAutoCycle);
        }
        let pal_on = self.popover == Popover::Palette;
        if cx.icon_button(
            "palette",
            xs[1],
            cy,
            40.0,
            Icon::Swatch,
            if pal_on { "" } else { "Palette" },
            if pal_on { Style::Accent } else { Style::Glass },
            true,
        ) {
            self.toggle(Popover::Palette);
        }
        let set_on = self.popover == Popover::Settings;
        if cx.icon_button(
            "settings",
            xs[2],
            cy,
            40.0,
            Icon::Sliders,
            if set_on { "" } else { "Settings" },
            if set_on { Style::Accent } else { Style::Glass },
            true,
        ) {
            self.toggle(Popover::Settings);
        }
        let fs_icon = if m.fullscreen { Icon::Shrink } else { Icon::Expand };
        if cx.icon_button(
            "fullscreen",
            xs[3],
            cy,
            40.0,
            fs_icon,
            "Fullscreen (F)",
            Style::Glass,
            true,
        ) {
            cx.out.push(Action::ToggleFullscreen);
        }
        let actions_left = xs[0] - 20.0;

        // The genre on screen: a button that steps to the next genre, then
        // that genre's presets as tabs. Centred; the tabs collapse to a
        // stepper when they do not fit.
        let shown = m.preset.min(PRESETS.len() - 1);
        let genre = PRESETS[shown].genre;
        let members: Vec<usize> = presets::in_genre(genre).collect();
        let slot = members.iter().position(|&p| p == shown).unwrap_or(0);
        let genre_name = GENRES[genre].name;
        let genre_w = cx.ui.text_width(genre_name, 13.0, SEMIBOLD) + 46.0;
        let gap = 8.0;
        let widths: Vec<f32> = members
            .iter()
            .map(|&p| cx.ui.text_width(PRESETS[p].name, 13.0, SEMIBOLD) + 26.0)
            .collect();
        let tabs_w: f32 = widths.iter().sum::<f32>() + 8.0;
        let total = genre_w + gap + tabs_w;
        let side = (brand_right.max(w - actions_left)) + 18.0;
        if total <= w - side * 2.0 {
            let x0 = (w - total) * 0.5;
            genre_button(cx, Rect::new(x0, cy - 20.0, genre_w, 40.0), genre_name);
            let bar = Rect::new(x0 + genre_w + gap, cy - 20.0, tabs_w, 40.0);
            cx.ui.glass(bar, 20.0, lens(0.0, 0.0).frost(0.22));
            let mut x = bar.x + 4.0;
            let mut cells = Vec::with_capacity(members.len());
            for wd in &widths {
                cells.push(Rect::new(x, bar.y + 4.0, *wd, 32.0));
                x += wd;
            }
            // The highlight is a drop of stained glass that flows between
            // tabs: its leading edge sets off first and the rest catches up,
            // so it stretches on the way and gathers itself on arrival.
            let target = cells[slot];
            let (lx, rx) = (target.x, target.right());
            let (lead, trail) = (26.0, 13.0);
            let going_right = cx.ui.anim(id("tab-mid", 0), target.cx(), trail) < target.cx();
            let (lrate, rrate) = if going_right { (trail, lead) } else { (lead, trail) };
            let x0 = cx.ui.anim(id("tab-l", 0), lx, lrate);
            let x1 = cx.ui.anim(id("tab-r", 0), rx, rrate);
            let pill = Rect::new(x0, target.y, (x1 - x0).max(target.h), target.h);
            cx.ui.shadow(pill, 16.0, 12.0, alpha(a2, 0.4));
            cx.ui.glass(pill, 16.0, stained(a1, a2));
            for (cell, &preset) in cells.iter().zip(&members) {
                let wid = id("tab", preset);
                let hit = cx.ui.interact(wid, *cell);
                let (hv, _) = cx.hover_anim(wid, &hit);
                // Ink on the highlight, light elsewhere.
                let overlap = ((cell.right().min(pill.right()) - cell.x.max(pill.x)) / cell.w).clamp(0.0, 1.0);
                let color = mix(dim(0.66 + 0.34 * hv), INK, overlap);
                cx.ui
                    .label(PRESETS[preset].name, *cell, 13.0, SEMIBOLD, color, Align::Center);
                if hit.clicked && preset != m.preset {
                    cx.out.push(Action::SetPreset(preset));
                }
            }
        } else {
            let bar_w = 212.0f32.min(w - 2.0 * (pad + 40.0) - 180.0).max(150.0);
            let bar = Rect::new(((w - bar_w) * 0.5).max(brand_right + 12.0), cy - 20.0, bar_w, 40.0);
            // The genre button goes to the left of the stepper while there
            // is room for it; the G key works either way.
            let genre_x = bar.x - gap - genre_w;
            if genre_x >= brand_right + 12.0 {
                genre_button(cx, Rect::new(genre_x, cy - 20.0, genre_w, 40.0), genre_name);
            }
            cx.ui.glass(bar, 20.0, lens(0.0, 0.0).frost(0.22));
            if cx.icon_button(
                "preset-prev",
                bar.x + 20.0,
                cy,
                32.0,
                Icon::ChevronLeft,
                "",
                Style::Bare,
                true,
            ) {
                cx.out.push(Action::StepPreset(-1));
            }
            if cx.icon_button(
                "preset-next",
                bar.right() - 20.0,
                cy,
                32.0,
                Icon::ChevronRight,
                "",
                Style::Bare,
                true,
            ) {
                cx.out.push(Action::StepPreset(1));
            }
            cx.ui.label(
                PRESETS[shown].name,
                Rect::new(bar.x + 38.0, bar.y, bar.w - 76.0, bar.h),
                13.5,
                SEMIBOLD,
                TEXT,
                Align::Center,
            );
        }
    }

    fn hero(&mut self, cx: &mut Cx, w: f32, dock_top: f32) {
        let (a1, a2) = (cx.a1, cx.a2);

        // The three places music comes from. The demo track is not one of
        // them: it is the quiet link underneath, for when there is nothing
        // to play.
        let cards: [(Icon, &str, &str, &str, Action); 3] = [
            (
                Icon::Music,
                "Open music",
                "Play files from your library",
                "O",
                Action::OpenFiles,
            ),
            (
                Icon::Monitor,
                "System audio",
                "Visualize whatever is playing",
                "A",
                Action::StartSystem,
            ),
            (Icon::Mic, "Microphone", "React to the room", "M", Action::StartMic),
        ];
        // One row, even in the smallest window.
        let (ch, gap) = (132.0, 14.0);
        let n = cards.len() as f32;
        let cw = 204.0f32.min((w - 60.0 - gap * (n - 1.0)) / n);
        let grid_w = cw * n + gap * (n - 1.0);
        let grid_h = ch;

        // Everything has to fit between the top bar and the dock. A short
        // window first shrinks the logo, then closes the gap above the
        // cards, and at last gives up the line of tips. The demo link stays.
        let mut logo_h = (w * 0.105).clamp(84.0, 168.0);
        let (mut lead, mut tips) = (34.0, true);
        let mut short = logo_h + 48.0 + lead + grid_h + 68.0 - (dock_top - 94.0);
        if short > 0.0 {
            let cut = short.min(logo_h - 64.0);
            logo_h -= cut;
            short -= cut;
            let cut = short.min(lead - 18.0);
            lead -= cut;
            short -= cut;
            tips = short <= 0.0;
        }
        let logo_w = logo_h * logo::aspect(Logo::Full);
        let block_h = logo_h + 48.0 + lead + grid_h + if tips { 68.0 } else { 44.0 };
        let top = ((76.0 + dock_top - block_h) * 0.5).max(84.0);

        // The logo, with a warm glow behind it.
        let mark = Rect::new((w - logo_w) * 0.5, top, logo_w, logo_h);
        cx.ui.shadow(
            Rect::new(
                mark.x + logo_w * 0.16,
                mark.y + logo_h * 0.3,
                logo_w * 0.68,
                logo_h * 0.4,
            ),
            logo_h,
            logo_h * 0.7,
            hex(0xf8db60, 0.26),
        );
        cx.ui.logo(Logo::Full, mark);

        let tag = Rect::new(0.0, top + logo_h + 14.0, w, 28.0);
        cx.ui.label(
            "Ear candy for your eyes. Pick something to listen to.",
            tag,
            17.0,
            REGULAR,
            dim(0.74),
            Align::Center,
        );

        let gy = tag.bottom() + lead;
        let gx = (w - grid_w) * 0.5;
        for (i, (icon, title, hint, key, action)) in cards.into_iter().enumerate() {
            let base = Rect::new(gx + (cw + gap) * i as f32, gy, cw, ch);
            let wid = id("hero-card", i);
            let hit = cx.ui.interact(wid, base);
            let (hv, pr) = cx.hover_anim(wid, &hit);
            let r = Rect::new(base.x, base.y - 4.0 * hv + 2.0 * pr, base.w, base.h);
            cx.ui.shadow(r, 26.0, 24.0, alpha(a2, 0.28 * hv));
            cx.ui.glass(r, 26.0, pane().glow(hv));
            let badge = Rect::new(r.x + 18.0, r.y + 18.0, 40.0, 40.0);
            cx.ui.glass(badge, 20.0, stained(a1, a2).glow(hv));
            cx.ui.icon(icon, badge.cx(), badge.cy(), 22.0, INK);
            // Keyboard hint.
            let kr = Rect::new(r.right() - 18.0 - 24.0, r.y + 18.0, 24.0, 22.0);
            cx.ui.rect_bordered(kr, 6.0, white(0.06), 1.0, white(0.14));
            cx.ui.label(key, kr, 11.5, SEMIBOLD, dim(0.6), Align::Center);
            cx.ui.label(
                title,
                Rect::new(r.x + 18.0, r.y + 70.0, r.w - 36.0, 22.0),
                15.5,
                SEMIBOLD,
                TEXT,
                Align::Left,
            );
            cx.ui.label(
                hint,
                Rect::new(r.x + 18.0, r.y + 93.0, r.w - 36.0, 20.0),
                12.5,
                REGULAR,
                dim(0.6),
                Align::Left,
            );
            if hit.clicked {
                cx.out.push(action);
            }
        }
        let mut y = gy + grid_h + 22.0;
        if tips {
            cx.ui.label(
                "Drop audio files anywhere  ·  press ? for shortcuts",
                Rect::new(0.0, y, w, 20.0),
                12.5,
                REGULAR,
                dim(0.45),
                Align::Center,
            );
            y += 24.0;
        }

        // The demo track: a text link, as quiet as the line above it until
        // the pointer finds it.
        let (ask, link, size) = ("Nothing to play?", "Try the demo track", 12.5);
        let ask_w = cx.ui.text_width(ask, size, REGULAR);
        let link_w = cx.ui.text_width(link, size, MEDIUM);
        let x = (w - ask_w - 5.0 - link_w) * 0.5;
        cx.ui
            .label(ask, Rect::new(x, y, ask_w, 20.0), size, REGULAR, dim(0.45), Align::Left);
        let lr = Rect::new(x + ask_w + 5.0, y, link_w, 20.0);
        let wid = id("hero-demo", 0);
        let hit = cx.ui.interact(wid, lr.inset(-4.0));
        let (hv, _) = cx.hover_anim(wid, &hit);
        cx.ui.label(link, lr, size, MEDIUM, dim(0.66 + 0.34 * hv), Align::Left);
        // A hairline underline that takes the palette's colours on hover.
        let rule = Rect::new(lr.x, lr.cy() + 8.0, lr.w, 1.0);
        cx.ui.rect(rule, 0.0, white(0.24 * (1.0 - hv)));
        cx.ui.gradient(rule, 0.0, alpha(a1, hv), alpha(a2, hv), false);
        cx.tip(wid, &hit, lr, "Demo track (D)", true);
        if hit.clicked {
            cx.out.push(Action::StartDemo);
        }
    }

    fn dock(&mut self, cx: &mut Cx, dock: Rect) {
        let m = cx.m;
        let (a1, a2) = (cx.a1, cx.a2);
        cx.ui.glass(dock, 30.0, pane());

        // ---- seek row -------------------------------------------------------
        let bar = Rect::new(dock.x + 64.0, dock.y + 12.0, dock.w - 128.0, 18.0);
        let left = Rect::new(dock.x + 12.0, bar.y, 44.0, bar.h);
        let right = Rect::new(dock.right() - 56.0, bar.y, 44.0, bar.h);
        match m.source {
            Source::Player => {
                let seekable = m.duration > 0.0;
                let wid = id("seek", 0);
                let hit = if seekable {
                    cx.ui.interact(wid, bar.inset(-5.0))
                } else {
                    Hit::default()
                };
                let (hv, _) = cx.hover_anim(wid, &hit);
                let mut frac = if seekable {
                    (m.position / m.duration).clamp(0.0, 1.0) as f32
                } else {
                    0.0
                };
                if hit.held || hit.clicked {
                    // Scrub: preview while dragging, commit on release.
                    frac = ((cx.ui.input.mouse.0 - bar.x) / bar.w).clamp(0.0, 1.0);
                    if hit.clicked || cx.ui.input.released {
                        cx.out.push(Action::Seek(frac));
                    }
                }
                let th = 4.0 + 2.0 * hv;
                let track = Rect::new(bar.x, bar.cy() - th * 0.5, bar.w, th);
                cx.ui.rect(track, th, white(0.16));
                if frac > 0.0 {
                    cx.ui
                        .gradient(Rect::new(track.x, track.y, track.w * frac, th), th, a1, a2, false);
                }
                if hv > 0.01 {
                    let k = 14.0 * hv;
                    let knob = Rect::new(bar.x + bar.w * frac - k * 0.5, bar.cy() - k * 0.5, k, k);
                    cx.ui.glass(knob, k, bead().glow(hv));
                }
                let shown = if seekable { frac as f64 * m.duration } else { m.position };
                cx.ui
                    .label(&fmt_time(shown), left, 12.0, MEDIUM, dim(0.7), Align::Right);
                if seekable {
                    cx.ui
                        .label(&fmt_time(m.duration), right, 12.0, MEDIUM, dim(0.7), Align::Left);
                }
            }
            Source::None => {
                cx.ui
                    .rect(Rect::new(bar.x, bar.cy() - 2.0, bar.w, 4.0), 4.0, white(0.10));
            }
            _ => {
                // Live sources have no timeline: show a miniature spectrum instead.
                let n = ((bar.w / 7.0) as usize).clamp(16, 160);
                let step = bar.w / n as f32;
                for i in 0..n {
                    let f = i as f32 / n as f32;
                    let bin = ((0.02 + f * 0.9) * m.spectrum.len() as f32) as usize;
                    let v = m.spectrum.get(bin).copied().unwrap_or(0) as f32 / 255.0;
                    let bh = 2.0 + v * 15.0;
                    let r = Rect::new(
                        bar.x + step * i as f32 + step * 0.2,
                        bar.cy() - bh * 0.5,
                        step * 0.6,
                        bh,
                    );
                    cx.ui.rect(r, 2.0, alpha(mix(a1, a2, f), 0.55 + 0.45 * v));
                }
                let tag = if m.source == Source::Demo { "DEMO" } else { "LIVE" };
                cx.ui.label(tag, left, 10.5, BOLD, alpha(a1, 0.9), Align::Right);
            }
        }

        let cy = dock.y + 61.0;

        // ---- transport (centre) ----------------------------------------------
        let has_queue = m.tracks.len() > 1 || (m.source != Source::Player && !m.tracks.is_empty());
        if cx.icon_button(
            "prev",
            dock.cx() - 52.0,
            cy,
            36.0,
            Icon::Prev,
            "Previous (B)",
            Style::Bare,
            has_queue,
        ) {
            cx.out.push(Action::Prev);
        }
        let live = matches!(m.source, Source::System | Source::Mic);
        let play_icon = if live {
            Icon::Stop
        } else if m.playing {
            Icon::Pause
        } else {
            Icon::Play
        };
        let play_tip = if live {
            "Stop listening (Space)"
        } else if m.playing {
            "Pause (Space)"
        } else {
            "Play (Space)"
        };
        if cx.icon_button("play", dock.cx(), cy, 46.0, play_icon, play_tip, Style::Solid, true) {
            cx.out.push(Action::TogglePlay);
        }
        if cx.icon_button(
            "next",
            dock.cx() + 52.0,
            cy,
            36.0,
            Icon::Next,
            "Next (N)",
            Style::Bare,
            has_queue,
        ) {
            cx.out.push(Action::Next);
        }

        // ---- sources, volume, queue (right) -----------------------------------
        let mut x = dock.right() - 16.0 - 17.0;
        if cx.icon_button(
            "queue",
            x,
            cy,
            34.0,
            Icon::List,
            "Queue (Q)",
            if self.queue_open { Style::Accent } else { Style::Bare },
            true,
        ) {
            self.queue_open = !self.queue_open;
        }
        x -= 17.0 + 12.0;
        let compact = dock.w < 900.0;
        if !compact {
            let vol = Rect::new(x - 84.0, cy - 9.0, 84.0, 18.0);
            let shown = if m.settings.muted { 0.0 } else { m.settings.volume };
            if let Some(v) = cx.slider("volume", vol, shown) {
                cx.out.push(Action::SetVolume(v));
            }
            x -= 84.0 + 10.0;
        }
        x -= 17.0;
        let vol_icon = if m.settings.muted || m.settings.volume <= 0.001 {
            Icon::Mute
        } else {
            Icon::Volume
        };
        if cx.icon_button("mute", x, cy, 34.0, vol_icon, "Mute", Style::Bare, true) {
            cx.out.push(Action::ToggleMute);
        }
        x -= 17.0 + 9.0;
        cx.ui.rect(Rect::new(x, cy - 11.0, 1.0, 22.0), 0.0, white(0.14));
        x -= 9.0 + 17.0;
        // The demo track has no button here: the landing screen offers it,
        // and D starts it from anywhere.
        let sources = [
            ("src-mic", Icon::Mic, "Microphone (M)", Source::Mic, Action::StartMic),
            (
                "src-system",
                Icon::Monitor,
                "System audio (A)",
                Source::System,
                Action::StartSystem,
            ),
            (
                "src-files",
                Icon::Music,
                "Open music (O)",
                Source::Player,
                Action::OpenFiles,
            ),
        ];
        for (key, icon, tip, source, action) in sources {
            let style = if m.source == source { Style::Accent } else { Style::Bare };
            if cx.icon_button(key, x, cy, 34.0, icon, tip, style, true) {
                cx.out.push(action);
            }
            x -= 38.0;
        }
        let right_edge = x + 38.0 - 17.0 - 12.0;

        // ---- now playing (left) ------------------------------------------------
        let art = Rect::new(dock.x + 16.0, cy - 21.0, 42.0, 42.0);
        if m.has_art && m.source == Source::Player {
            cx.ui.art(art, 10.0, 1.0);
            cx.ui.rect_bordered(art, 10.0, CLEAR, 1.0, white(0.14));
        } else {
            cx.ui.glass(art, 10.0, stained(a1, a2));
            let icon = match m.source {
                Source::System => Icon::Monitor,
                Source::Mic => Icon::Mic,
                Source::Demo => Icon::Wave,
                _ => Icon::Music,
            };
            cx.ui.icon(icon, art.cx(), art.cy(), 22.0, INK);
        }
        let text_x = art.right() + 12.0;
        let text_w = (right_edge.min(dock.cx() - 86.0) - text_x).max(40.0);
        let single = m.subtitle.is_empty();
        let ty = if single { cy - 10.0 } else { cy - 19.0 };
        cx.ui.label(
            m.title,
            Rect::new(text_x, ty, text_w, 20.0),
            14.0,
            SEMIBOLD,
            TEXT,
            Align::Left,
        );
        if !single {
            cx.ui.label(
                m.subtitle,
                Rect::new(text_x, cy + 1.0, text_w, 18.0),
                12.5,
                REGULAR,
                dim(0.62),
                Align::Left,
            );
        }
    }

    fn queue(&mut self, cx: &mut Cx, base: Rect) {
        let m = cx.m;
        let (a1, a2) = (cx.a1, cx.a2);
        let t = cx.ui.anim_from(id("queue-open", 0), 0.0, 1.0, 16.0);
        let r = Rect::new(base.x + (1.0 - t) * 24.0, base.y, base.w, base.h);
        let outer = cx.ui.alpha;
        cx.ui.alpha = outer * t;
        cx.ui.glass(r, 28.0, pane());

        cx.ui.text("Queue", r.x + 18.0, r.y + 14.0, 15.5, SEMIBOLD, TEXT);
        if !m.tracks.is_empty() {
            let count = format!("{} track{}", m.tracks.len(), if m.tracks.len() == 1 { "" } else { "s" });
            cx.ui.text(&count, r.x + 74.0, r.y + 17.0, 12.5, REGULAR, dim(0.55));
        }
        if cx.icon_button(
            "queue-add",
            r.right() - 30.0,
            r.y + 26.0,
            30.0,
            Icon::Plus,
            "Add files (O)",
            Style::Bare,
            true,
        ) {
            cx.out.push(Action::OpenFiles);
        }
        if !m.tracks.is_empty() {
            let clear = Rect::new(r.right() - 102.0, r.y + 13.0, 50.0, 26.0);
            let wid = id("queue-clear", 0);
            let hit = cx.ui.interact(wid, clear);
            let (hv, _) = cx.hover_anim(wid, &hit);
            cx.ui.rect(clear, 13.0, white(0.06 + 0.08 * hv));
            cx.ui
                .label("Clear", clear, 12.0, SEMIBOLD, dim(0.65 + 0.35 * hv), Align::Center);
            if hit.clicked {
                cx.out.push(Action::ClearQueue);
            }
        }

        let list = Rect::new(r.x + 8.0, r.y + 50.0, r.w - 16.0, r.h - 60.0);
        if m.tracks.is_empty() {
            cx.ui.icon(Icon::Music, list.cx(), list.cy() - 30.0, 34.0, dim(0.3));
            cx.ui.label(
                "Nothing queued",
                Rect::new(list.x, list.cy() - 4.0, list.w, 22.0),
                14.5,
                SEMIBOLD,
                dim(0.75),
                Align::Center,
            );
            cx.ui.label(
                "Drop audio files here, or press O",
                Rect::new(list.x, list.cy() + 18.0, list.w, 20.0),
                12.5,
                REGULAR,
                dim(0.5),
                Align::Center,
            );
            cx.ui.alpha = outer;
            return;
        }

        let row_h = 46.0;
        let content = m.tracks.len() as f32 * row_h;
        let max_scroll = (content - list.h).max(0.0);
        if cx.ui.hovered(list) {
            self.queue_scroll -= cx.ui.input.scroll * 60.0;
        }
        self.queue_scroll = self.queue_scroll.clamp(0.0, max_scroll);
        let scroll = cx.ui.anim(id("queue-scroll", 0), self.queue_scroll, 22.0);

        cx.ui.set_clip(Some(list));
        for (i, track) in m.tracks.iter().enumerate() {
            let row = Rect::new(list.x, list.y + i as f32 * row_h - scroll, list.w, row_h);
            if row.bottom() < list.y || row.y > list.bottom() {
                continue;
            }
            let current = m.current == Some(i);
            // The remove button is interacted with first so it wins the click.
            let rm_c = (row.right() - 20.0, row.cy());
            let row_hover = cx.ui.hovered(row);
            let mut removed = false;
            if row_hover {
                let rm = Rect::new(rm_c.0 - 13.0, rm_c.1 - 13.0, 26.0, 26.0);
                let hit = cx.ui.interact(id("queue-rm", i), rm);
                cx.ui.rect(rm, 13.0, white(if hit.hover { 0.14 } else { 0.0 }));
                cx.ui.icon(
                    Icon::Close,
                    rm_c.0,
                    rm_c.1,
                    14.0,
                    dim(if hit.hover { 1.0 } else { 0.55 }),
                );
                removed = hit.clicked;
            }
            let wid = id("queue-row", i);
            let hit = cx.ui.interact(wid, row);
            let (hv, _) = cx.hover_anim(wid, &hit);
            if current {
                cx.ui
                    .gradient(row.inset(1.0), 12.0, alpha(a1, 0.16), alpha(a2, 0.16), false);
            } else if hv > 0.0 {
                cx.ui.rect(row.inset(1.0), 12.0, white(0.06 * hv));
            }
            let num = Rect::new(row.x + 6.0, row.y, 26.0, row.h);
            if current {
                // A tiny equaliser marks the playing track.
                for b in 0..3 {
                    let bin = m.spectrum.len() * (b * 2 + 1) / 12;
                    let v = if m.playing {
                        m.spectrum.get(bin).copied().unwrap_or(0) as f32 / 255.0
                    } else {
                        0.15
                    };
                    let bh = 3.0 + v * 12.0;
                    cx.ui.rect(
                        Rect::new(num.cx() - 6.0 + b as f32 * 5.0, num.cy() + 7.0 - bh, 3.0, bh),
                        1.5,
                        mix(a1, a2, b as f32 * 0.5),
                    );
                }
            } else {
                cx.ui
                    .label(&(i + 1).to_string(), num, 12.0, MEDIUM, dim(0.42), Align::Center);
            }
            let tx = row.x + 38.0;
            let tw = row.w - 38.0 - 52.0;
            let color = if current { TEXT } else { dim(0.86) };
            if track.artist.is_empty() {
                cx.ui.label(
                    &track.title,
                    Rect::new(tx, row.y, tw, row.h),
                    13.5,
                    MEDIUM,
                    color,
                    Align::Left,
                );
            } else {
                cx.ui.label(
                    &track.title,
                    Rect::new(tx, row.y + 5.0, tw, 19.0),
                    13.5,
                    MEDIUM,
                    color,
                    Align::Left,
                );
                cx.ui.label(
                    &track.artist,
                    Rect::new(tx, row.y + 23.0, tw, 17.0),
                    12.0,
                    REGULAR,
                    dim(0.52),
                    Align::Left,
                );
            }
            if !row_hover && let Some(d) = track.duration {
                cx.ui.label(
                    &fmt_time(d),
                    Rect::new(row.right() - 50.0, row.y, 42.0, row.h),
                    12.0,
                    REGULAR,
                    dim(0.5),
                    Align::Right,
                );
            }
            if removed {
                cx.out.push(Action::RemoveIndex(i));
            } else if hit.clicked {
                cx.out.push(Action::PlayIndex(i));
            }
        }
        cx.ui.set_clip(None);

        // Scrollbar.
        if max_scroll > 0.0 {
            let th = (list.h * list.h / content).max(28.0);
            let ty = list.y + (list.h - th) * (scroll / max_scroll);
            cx.ui.rect(Rect::new(r.right() - 6.0, ty, 3.0, th), 2.0, white(0.22));
        }
        cx.ui.alpha = outer;
    }

    fn popover_frame(&mut self, cx: &mut Cx, key: &str, base: Rect) -> Rect {
        let t = cx.ui.anim_from(id(key, 0), 0.0, 1.0, 20.0);
        let r = Rect::new(base.x, base.y - (1.0 - t) * 8.0, base.w, base.h);
        cx.ui.alpha *= t;
        cx.ui.glass(r, 26.0, reading_pane());
        r
    }

    fn palette_popover(&mut self, cx: &mut Cx, base: Rect) {
        let outer = cx.ui.alpha;
        let r = self.popover_frame(cx, "pop-palette", base);
        cx.ui.text("Palette", r.x + 16.0, r.y + 13.0, 13.0, SEMIBOLD, dim(0.6));
        // Two columns, filled one after the other, so the first half of the
        // list keeps its place on the left however many palettes there are.
        let rows = PALETTES.len().div_ceil(2);
        // Names line up after the longest string of beads.
        let most = PALETTES.iter().map(|p| p.stops.len()).max().unwrap_or(0);
        for (i, pal) in PALETTES.iter().enumerate() {
            let (col, line) = ((i / rows) as f32, (i % rows) as f32);
            let row = Rect::new(
                r.x + 8.0 + col * (PALETTE_COL + 4.0),
                r.y + 42.0 + line * PALETTE_ROW,
                PALETTE_COL,
                PALETTE_ROW - 2.0,
            );
            let wid = id("palette-row", i);
            let hit = cx.ui.interact(wid, row);
            let (hv, _) = cx.hover_anim(wid, &hit);
            let on = i == cx.m.palette;
            cx.ui.rect(row, 12.0, white(if on { 0.10 } else { 0.07 * hv }));
            // The palette as a string of beads.
            for (j, stop) in pal.stops.iter().enumerate().rev() {
                let d = 18.0;
                let bx = row.x + 10.0 + j as f32 * 11.0;
                let bead = Rect::new(bx, row.cy() - d * 0.5, d, d);
                cx.ui.rect_bordered(bead, d, hex(*stop, 1.0), 1.5, hex(0x1a1424, 1.0));
            }
            let lx = row.x + 10.0 + most as f32 * 11.0 + 16.0;
            cx.ui.label(
                pal.name,
                Rect::new(lx, row.y, row.right() - lx - 34.0, row.h),
                13.5,
                if on { SEMIBOLD } else { MEDIUM },
                dim(if on { 1.0 } else { 0.8 + 0.2 * hv }),
                Align::Left,
            );
            if on {
                cx.ui.icon(Icon::Check, row.right() - 20.0, row.cy(), 16.0, cx.a1);
            }
            if hit.clicked && !on {
                cx.out.push(Action::SetPalette(i));
            }
        }
        cx.ui.alpha = outer;
    }

    fn settings_popover(&mut self, cx: &mut Cx, base: Rect) {
        let outer = cx.ui.alpha;
        let r = self.popover_frame(cx, "pop-settings", base);
        let s = cx.m.settings;
        let (x, w) = (r.x + 18.0, r.w - 36.0);
        let mut y = r.y + 16.0;

        let heading = |cx: &mut Cx, y: f32, label: &str, value: &str| {
            cx.ui.text(label, x, y, 13.0, SEMIBOLD, dim(0.86));
            if !value.is_empty() {
                cx.ui
                    .label(value, Rect::new(x, y, w, 17.0), 12.5, MEDIUM, dim(0.55), Align::Right);
            }
        };

        heading(cx, y, "Sensitivity", &format!("{:.1}×", s.sensitivity));
        if let Some(v) = cx.slider(
            "sensitivity",
            Rect::new(x, y + 24.0, w, 18.0),
            (s.sensitivity - 0.25) / 2.75,
        ) {
            cx.out.push(Action::SetSensitivity(0.25 + v * 2.75));
        }
        y += 56.0;

        heading(cx, y, "Reactivity", &format!("{:.0}%", s.reactivity * 100.0));
        if let Some(v) = cx.slider("reactivity", Rect::new(x, y + 24.0, w, 18.0), s.reactivity) {
            cx.out.push(Action::SetReactivity(v));
        }
        y += 56.0;

        heading(cx, y, "Glow", &format!("{:.0}%", s.glow * 100.0));
        if let Some(v) = cx.slider("glow", Rect::new(x, y + 24.0, w, 18.0), s.glow / 2.0) {
            cx.out.push(Action::SetGlow(v * 2.0));
        }
        y += 56.0;

        // From clear glass at one end to clouded, nearly opaque at the other.
        heading(cx, y, "Glass frost", &format!("{:.0}%", s.glass_frost * 100.0));
        if let Some(v) = cx.slider("glass-frost", Rect::new(x, y + 24.0, w, 18.0), s.glass_frost) {
            cx.out.push(Action::SetGlassFrost(v));
        }
        y += 56.0;

        heading(cx, y, "Auto-cycle presets", "");
        let cycles = [0u32, 15, 30, 60];
        let sel = cycles.iter().position(|c| *c == s.auto_cycle);
        if let Some(i) = cx.segmented(
            "cycle",
            Rect::new(x, y + 24.0, w, 30.0),
            &["Off", "15 s", "30 s", "60 s"],
            sel,
        ) {
            cx.out.push(Action::SetAutoCycle(cycles[i]));
        }
        y += 66.0;

        // The render resolution, from a quarter of the window's up to all of
        // it, in steps of 5%.
        heading(cx, y, "Graphics quality", &format!("{:.0}%", s.render_scale * 100.0));
        let at = (s.render_scale - 0.25) / 0.75;
        if let Some(v) = cx.stepped_slider("quality", Rect::new(x, y + 24.0, w, 18.0), at, 15) {
            cx.out.push(Action::SetRenderScale(0.25 + v * 0.75));
        }
        y += 56.0;

        // One step per frame a second, up to the fastest rate that can be
        // held on this display, and then one more: the top of the slider
        // sets no target at all, and the display paces the picture.
        let most = cx.m.display_hz.map_or(240, max_target_fps);
        let held = s.target_fps > 0 && s.target_fps <= most;
        let value = match cx.m.display_hz {
            _ if held => format!("{} fps", s.target_fps),
            Some(hz) => format!("Display · {hz} Hz"),
            None => "Display rate".into(),
        };
        heading(cx, y, "Target frame rate", &value);
        let steps = most - MIN_TARGET_FPS + 1;
        let at = if held {
            (s.target_fps - MIN_TARGET_FPS) as f32 / steps as f32
        } else {
            1.0
        };
        if let Some(v) = cx.stepped_slider("target-fps", Rect::new(x, y + 24.0, w, 18.0), at, steps) {
            let fps = MIN_TARGET_FPS + (v * steps as f32).round() as u32;
            cx.out.push(Action::SetTargetFps(if fps <= most { fps } else { 0 }));
        }
        y += 58.0;

        // FPS toggle switch.
        heading(cx, y + 3.0, "Show frame rate", "");
        let sw = Rect::new(r.right() - 18.0 - 40.0, y, 40.0, 24.0);
        let wid = id("fps-switch", 0);
        let hit = cx.ui.interact(wid, sw);
        let on = cx.ui.anim(wid ^ 0x44, s.show_fps as u32 as f32, 18.0);
        cx.ui.rect(sw, 12.0, white(0.14));
        if on > 0.0 {
            let (a1, a2) = (alpha(cx.a1, on), alpha(cx.a2, on));
            cx.ui.gradient(sw, 12.0, a1, a2, false);
        }
        let knob = Rect::new(sw.x + 3.0 + 16.0 * on, sw.y + 3.0, 18.0, 18.0);
        cx.ui.glass(knob, 9.0, bead());
        if hit.clicked {
            cx.out.push(Action::ToggleFps);
        }
        cx.ui.alpha = outer;
    }

    fn help(&mut self, cx: &mut Cx, w: f32, h: f32) {
        let outer = cx.ui.alpha;
        let t = cx.ui.anim_from(id("help-open", 0), 0.0, 1.0, 16.0);
        cx.ui.alpha = outer * t;
        cx.ui.rect(Rect::new(0.0, 0.0, w, h), 0.0, [0.0, 0.0, 0.0, 0.5]);
        let keys: [(&str, &str); 19] = [
            ("Space", "Play / pause"),
            ("← →", "Previous / next preset"),
            ("1 – 9, 0", "Jump to a preset"),
            ("G", "Next genre of presets"),
            ("[ ]", "Previous / next palette"),
            ("C", "Auto-cycle presets"),
            ("-  =", "Reactivity"),
            ("↑ ↓", "Volume"),
            ("N  B", "Next / previous track"),
            ("O", "Open music files"),
            ("A", "Capture system audio"),
            ("M", "Microphone"),
            ("D", "Demo track"),
            ("Q", "Show the queue"),
            ("F", "Fullscreen"),
            ("H", "Hide / show controls"),
            ("K", "Voices key"),
            ("S", "Save a screenshot"),
            ("Esc", "Close / leave fullscreen"),
        ];
        let rows = keys.len().div_ceil(2);
        let cw = 560.0f32.min(w - 40.0);
        let chh = 74.0 + rows as f32 * 34.0 + 18.0;
        let r = Rect::new((w - cw) * 0.5, (h - chh) * 0.5 - (1.0 - t) * 10.0, cw, chh);
        cx.ui.glass(r, 30.0, reading_pane());
        cx.ui
            .text("Keyboard shortcuts", r.x + 26.0, r.y + 22.0, 19.0, BOLD, TEXT);
        let col_w = (r.w - 52.0) * 0.5;
        for (i, (key, what)) in keys.iter().enumerate() {
            let (col, row) = (i / rows, i % rows);
            let x = r.x + 26.0 + col as f32 * col_w;
            let y = r.y + 70.0 + row as f32 * 34.0;
            let kw = cx.ui.text_width(key, 12.0, SEMIBOLD) + 18.0;
            let kr = Rect::new(x, y, kw.max(30.0), 24.0);
            cx.ui.rect_bordered(kr, 7.0, white(0.07), 1.0, white(0.16));
            cx.ui.label(key, kr, 12.0, SEMIBOLD, dim(0.9), Align::Center);
            cx.ui.label(
                what,
                Rect::new(x + 76.0, y, col_w - 84.0, 24.0),
                13.0,
                REGULAR,
                dim(0.72),
                Align::Left,
            );
        }
        cx.ui.alpha = outer;
    }

    fn draw_toasts(&mut self, cx: &mut Cx, w: f32, dock_top: f32, dt: f32) {
        for t in &mut self.toasts {
            t.1 += dt;
        }
        self.toasts.retain(|t| t.1 < TOAST_LIFE);
        let mut y = dock_top - 16.0;
        for (i, (text, age)) in self.toasts.iter().enumerate().rev() {
            let fade = (age / 0.18).min(1.0) * ((TOAST_LIFE - age) / 0.45).clamp(0.0, 1.0);
            let slide = cx.ui.anim_from(id(text, 0xbeef + i), 10.0, 0.0, 18.0);
            let tw = cx.ui.text_width(text, 13.5, MEDIUM);
            let r = Rect::new((w - tw) * 0.5 - 18.0, y - 38.0 + slide, tw + 36.0, 38.0);
            cx.ui.alpha = fade;
            cx.ui.glass(r, 19.0, reading_pane());
            cx.ui.label(text, r, 13.5, MEDIUM, TEXT, Align::Center);
            y -= 46.0;
        }
        cx.ui.alpha = 1.0;
    }
}

/// The voices key: a lamp for each thing the analyser listens for, in the
/// colour every preset draws that voice in, lit by how strongly it is
/// sounding. It teaches the colour code, and shows at a glance whether the
/// analyser is hearing the music right. It sits at the left edge, clear of
/// the middle of the picture, and stays up when the rest of the interface
/// fades away.
fn voices_key(cx: &mut Cx, dock_top: f32) {
    const NAMES: [&str; 5] = ["Kick", "Snare", "Hats", "Bass", "Tune"];
    let shown = cx
        .ui
        .anim(id("voices-key", 0), cx.m.settings.show_key as u32 as f32, 14.0);
    if shown < 0.004 {
        return;
    }
    cx.ui.alpha = shown;
    let colors = PALETTES[cx.m.palette].voices;
    let (row, pad) = (27.0, 13.0);
    let (w, h) = (96.0, row * NAMES.len() as f32 + pad * 2.0);
    let r = Rect::new(
        16.0 - (1.0 - shown) * 12.0,
        ((76.0 + dock_top - h) * 0.5).max(84.0),
        w,
        h,
    );
    cx.ui.glass(r, 24.0, pane());
    for (i, name) in NAMES.iter().enumerate() {
        let level = cx.m.voices[i].clamp(0.0, 1.0);
        let color = hex(colors[i], 1.0);
        let cy = r.y + pad + row * (i as f32 + 0.5);
        // The lamp swells and blooms as it lights.
        let d = 10.0 + 3.0 * level;
        let lamp = Rect::new(r.x + pad + 7.0 - d * 0.5, cy - d * 0.5, d, d);
        cx.ui.shadow(lamp, d, 9.0, alpha(color, 0.85 * level));
        cx.ui
            .rect(lamp, d, mix(alpha(color, 0.28), mix(color, white(1.0), 0.3), level));
        cx.ui.label(
            name,
            Rect::new(r.x + pad + 24.0, cy - 9.0, w - pad * 2.0 - 24.0, 18.0),
            12.5,
            MEDIUM,
            dim(0.58 + 0.42 * level),
            Align::Left,
        );
    }
}

/// The button that names the genre of presets on screen and steps to the
/// next one.
fn genre_button(cx: &mut Cx, r: Rect, name: &str) {
    let wid = id("genre", 0);
    let hit = cx.ui.interact(wid, r);
    let (hv, pr) = cx.hover_anim(wid, &hit);
    cx.ui.glass(r, 20.0, lens(hv, pr).frost(0.22));
    cx.ui.label(
        name,
        Rect::new(r.x + 16.0, r.y, r.w - 46.0, r.h),
        13.0,
        SEMIBOLD,
        dim(0.78 + 0.22 * hv),
        Align::Center,
    );
    cx.ui.icon(
        Icon::ChevronRight,
        r.right() - 19.0,
        r.cy(),
        15.0,
        dim(0.55 + 0.45 * hv),
    );
    cx.tip(wid, &hit, r, "Next genre (G)", true);
    if hit.clicked {
        cx.out.push(Action::StepGenre(1));
    }
}

fn tooltip(ui: &mut Ui, tip: &str, anchor: Rect, below: bool, w: f32) {
    let tw = ui.text_width(tip, 12.0, MEDIUM);
    let r_w = tw + 20.0;
    let x = (anchor.cx() - r_w * 0.5).clamp(8.0, w - r_w - 8.0);
    let y = if below {
        anchor.bottom() + 10.0
    } else {
        anchor.y - 10.0 - 28.0
    };
    let r = Rect::new(x, y, r_w, 28.0);
    ui.glass(r, 11.0, reading_pane().bevel(8.0).shadow(0.7));
    ui.label(tip, r, 12.0, MEDIUM, dim(0.92), Align::Center);
}

fn drop_overlay(cx: &mut Cx, w: f32, h: f32) {
    cx.ui.rect(Rect::new(0.0, 0.0, w, h), 0.0, [0.02, 0.01, 0.05, 0.5]);
    // One great sheet of glass to drop the files onto, rimmed in the accent.
    let r = Rect::new(28.0, 28.0, w - 56.0, h - 56.0);
    let sheet = Glass::new(hex(0x26252c, 0.4), alpha(cx.a1, 0.6))
        .frost(0.34)
        .bevel(22.0)
        .bend(2.0);
    cx.ui.glass(r, 34.0, sheet);
    cx.ui
        .icon_gradient(Icon::Music, w * 0.5, h * 0.5 - 40.0, 64.0, cx.a1, cx.a2);
    cx.ui.label(
        "Drop to play",
        Rect::new(0.0, h * 0.5 + 6.0, w, 40.0),
        28.0,
        BOLD,
        TEXT,
        Align::Center,
    );
    cx.ui.label(
        "Audio files are added to the queue",
        Rect::new(0.0, h * 0.5 + 48.0, w, 24.0),
        14.0,
        REGULAR,
        dim(0.65),
        Align::Center,
    );
}
