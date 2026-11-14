//! A small immediate-mode UI toolkit drawn entirely on the GPU.
//!
//! Every frame the interface is rebuilt as a flat list of [`Quad`]s (rounded
//! rectangles, glass panels, shadows, glyphs, icons) which `render` draws in a
//! single instanced call. Layout is in logical pixels; [`Ui`] converts to
//! physical pixels and snaps text to the pixel grid.

pub mod atlas;
pub mod icons;
pub mod logo;
pub mod render;
pub mod text;
pub mod view;

use std::collections::HashMap;
use std::rc::Rc;

use bytemuck::{Pod, Zeroable};

use atlas::{Atlas, Sprite};
use icons::Icon;
use logo::Logo;
use text::{Shaped, Text};

/// Straight-alpha sRGB.
pub type Color = [f32; 4];

pub const CLEAR: Color = [0.0, 0.0, 0.0, 0.0];

/// `0xRRGGBB` plus alpha.
pub fn hex(rgb: u32, a: f32) -> Color {
    let c = crate::palette::rgb(rgb);
    [c[0], c[1], c[2], a]
}

pub fn white(a: f32) -> Color {
    [1.0, 1.0, 1.0, a]
}

pub fn alpha(c: Color, a: f32) -> Color {
    [c[0], c[1], c[2], c[3] * a]
}

pub fn mix(a: Color, b: Color, t: f32) -> Color {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn cx(&self) -> f32 {
        self.x + self.w * 0.5
    }

    pub fn cy(&self) -> f32 {
        self.y + self.h * 0.5
    }

    pub fn contains(&self, p: (f32, f32)) -> bool {
        p.0 >= self.x && p.0 < self.right() && p.1 >= self.y && p.1 < self.bottom()
    }

    /// Shrink (or, with a negative amount, grow) on every side.
    pub fn inset(&self, d: f32) -> Rect {
        Rect::new(
            self.x + d,
            self.y + d,
            (self.w - 2.0 * d).max(0.0),
            (self.h - 2.0 * d).max(0.0),
        )
    }
}

/// A glass material: how a pane drawn with [`Ui::glass`] looks. The shader
/// (`ui.wgsl`) treats the pane as a slab lying on the picture, flat on top
/// and rounding off towards its rim, and lights and refracts it accordingly.
#[derive(Clone, Copy, Debug)]
pub struct Glass {
    /// Colour laid over what shows through; its alpha is how strongly.
    pub tint: Color,
    /// The tint at the right-hand edge, for a gradient.
    pub tint_end: Color,
    /// Colour of the light the polished edge catches; its alpha is how much.
    pub rim: Color,
    /// 0 is clear glass, 0.5 lightly sanded, 1 opal.
    pub frost: f32,
    /// Width of the rounded lip around the edge, in logical pixels: the part
    /// that bends light. At most half the pane's shorter side.
    pub bevel: f32,
    /// How thick the slab is, in lip-widths, and so how far the lip bends
    /// what is behind it.
    pub bend: f32,
    /// Extra light on the surface, 0..1: for hover and press.
    pub glow: f32,
    /// How dark a shadow it casts on the picture, 0..1.
    pub shadow: f32,
    /// The tint is the colour of the piece itself (an accent button, a
    /// white knob) rather than a film over the picture, so the user's glass
    /// setting leaves it as strong as it is.
    pub solid: bool,
}

impl Glass {
    pub const fn new(tint: Color, rim: Color) -> Self {
        Self {
            tint,
            tint_end: tint,
            rim,
            frost: 0.3,
            bevel: 14.0,
            bend: 1.8,
            glow: 0.0,
            shadow: 0.0,
            solid: false,
        }
    }

    /// Tint running from the current colour on the left to `to` on the right.
    pub const fn gradient(mut self, to: Color) -> Self {
        self.tint_end = to;
        self
    }

    pub const fn frost(mut self, frost: f32) -> Self {
        self.frost = frost;
        self
    }

    pub const fn bevel(mut self, bevel: f32) -> Self {
        self.bevel = bevel;
        self
    }

    pub const fn bend(mut self, bend: f32) -> Self {
        self.bend = bend;
        self
    }

    pub const fn glow(mut self, glow: f32) -> Self {
        self.glow = glow;
        self
    }

    pub const fn shadow(mut self, shadow: f32) -> Self {
        self.shadow = shadow;
        self
    }

    /// See [`Glass::solid`].
    pub const fn solid(mut self) -> Self {
        self.solid = true;
        self
    }
}

const KIND_RECT: f32 = 0.0;
const KIND_GLASS: f32 = 1.0;
const KIND_GLYPH: f32 = 2.0;
const KIND_IMAGE: f32 = 3.0;
const KIND_SHADOW: f32 = 4.0;
const KIND_ART: f32 = 5.0;

/// One instance for `ui.wgsl`. All positions are physical pixels.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
pub struct Quad {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
    pub color2: [f32; 4],
    pub grad: [f32; 4],
    pub clip: [f32; 4],
    /// Corner radius, border width, kind, extra.
    pub params: [f32; 4],
    pub border: [f32; 4],
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Input {
    /// Pointer position in logical pixels.
    pub mouse: (f32, f32),
    /// Whether the pointer is over the window at all.
    pub inside: bool,
    pub down: bool,
    /// Went down / came up this frame.
    pub pressed: bool,
    pub released: bool,
    /// Wheel movement this frame, in lines (positive = scroll up).
    pub scroll: f32,
}

#[derive(Clone, Copy, Default)]
pub struct Hit {
    pub hover: bool,
    pub held: bool,
    pub pressed: bool,
    pub clicked: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

pub const REGULAR: u16 = 400;
pub const MEDIUM: u16 = 500;
pub const SEMIBOLD: u16 = 600;
pub const BOLD: u16 = 700;

/// A stable widget id from a label and an index.
pub fn id(label: &str, index: usize) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in label.bytes().chain(index.to_le_bytes()) {
        h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

pub struct Ui {
    pub quads: Vec<Quad>,
    /// Physical pixels per logical pixel.
    pub scale: f32,
    /// Window size in logical pixels.
    pub size: (f32, f32),
    pub input: Input,
    /// Multiplies the opacity of everything drawn.
    pub alpha: f32,
    /// The user's glass setting, 0..1: 0 makes every pane clear, 1 clouds
    /// it over until it is nearly opaque, and half way leaves each material
    /// as it was designed.
    pub frosting: f32,
    pub dt: f32,
    /// The widget the pointer went down on, until it comes back up.
    pub active: Option<u64>,
    /// Set when the pointer is over something clickable.
    pub hovering: bool,
    /// Areas covered by an overlay; widgets underneath ignore the pointer.
    pub blockers: Vec<Rect>,
    /// Wheel movement not yet spent on moving a stepped slider, in lines.
    pub wheel: f32,

    clip: [f32; 4],
    anims: HashMap<u64, (f32, u64)>,
    text: Text,
    pub atlas: Atlas,
    icons: HashMap<(Icon, u32), Option<Sprite>>,
    logos: HashMap<(Logo, u32, u32), Option<Sprite>>,
    frame: u64,
}

impl Ui {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            quads: Vec::with_capacity(2048),
            scale: 1.0,
            size: (1.0, 1.0),
            input: Input::default(),
            alpha: 1.0,
            frosting: 0.5,
            dt: 1.0 / 60.0,
            active: None,
            hovering: false,
            blockers: Vec::new(),
            wheel: 0.0,
            clip: [0.0; 4],
            anims: HashMap::new(),
            text: Text::new(),
            atlas: Atlas::new(device, queue),
            icons: HashMap::new(),
            logos: HashMap::new(),
            frame: 0,
        }
    }

    /// Start a frame. `size_px` is the window size in physical pixels.
    pub fn begin(&mut self, size_px: (u32, u32), scale: f32, dt: f32, input: Input) {
        self.quads.clear();
        self.scale = scale.max(0.25);
        self.size = (size_px.0 as f32 / self.scale, size_px.1 as f32 / self.scale);
        self.dt = dt.clamp(0.0005, 0.1);
        self.input = input;
        self.alpha = 1.0;
        self.hovering = false;
        self.blockers.clear();
        self.clip = [0.0, 0.0, size_px.0 as f32, size_px.1 as f32];
        if self.text.poll_fonts() {
            self.atlas.full = true;
        }
        if self.atlas.full {
            // Out of room (or new fonts): start over. Glyphs are re-rasterised as they are drawn.
            self.atlas.clear();
            self.text.reset();
            self.icons.clear();
            self.logos.clear();
        }
    }

    /// End a frame: release the active widget and trim caches.
    pub fn finish(&mut self) {
        if !self.input.down {
            self.active = None;
        }
        self.frame += 1;
        self.text.end_frame();
        if self.frame.is_multiple_of(600) {
            let cutoff = self.frame.saturating_sub(600);
            self.anims.retain(|_, (_, used)| *used >= cutoff);
        }
    }

    // ---- animation -------------------------------------------------------

    /// A value that eases towards `target`; starts at `target` on first use.
    pub fn anim(&mut self, id: u64, target: f32, rate: f32) -> f32 {
        self.anim_from(id, target, target, rate)
    }

    /// Like [`anim`](Self::anim), but starting from `initial` on first use.
    pub fn anim_from(&mut self, id: u64, initial: f32, target: f32, rate: f32) -> f32 {
        let k = 1.0 - (-self.dt * rate).exp();
        let entry = self.anims.entry(id).or_insert((initial, 0));
        entry.0 += (target - entry.0) * k;
        if (entry.0 - target).abs() < 0.0015 {
            entry.0 = target;
        }
        entry.1 = self.frame;
        entry.0
    }

    /// Forget an animation so it restarts from its initial value.
    pub fn reset_anim(&mut self, id: u64) {
        self.anims.remove(&id);
    }

    // ---- interaction -----------------------------------------------------

    /// Is the pointer over `r`, and not covered by an overlay or clipped away?
    pub fn hovered(&self, r: Rect) -> bool {
        let m = self.input.mouse;
        let mp = (m.0 * self.scale, m.1 * self.scale);
        self.input.inside
            && r.contains(m)
            && mp.0 >= self.clip[0]
            && mp.1 >= self.clip[1]
            && mp.0 < self.clip[2]
            && mp.1 < self.clip[3]
            && !self.blockers.iter().any(|b| b.contains(m))
    }

    /// Standard button behaviour for the area `r`.
    pub fn interact(&mut self, id: u64, r: Rect) -> Hit {
        let free = self.active.is_none() || self.active == Some(id);
        let hover = free && self.hovered(r);
        let mut hit = Hit {
            hover,
            ..Default::default()
        };
        if hover && self.input.pressed && self.active.is_none() {
            self.active = Some(id);
            hit.pressed = true;
        }
        if self.active == Some(id) {
            hit.held = self.input.down;
            hit.clicked = self.input.released && hover;
        }
        if hover {
            self.hovering = true;
        }
        hit
    }

    // ---- clipping --------------------------------------------------------

    /// Restrict drawing (and hit-testing) to `r`, or lift the restriction.
    pub fn set_clip(&mut self, r: Option<Rect>) {
        self.clip = match r {
            Some(r) => [
                r.x * self.scale,
                r.y * self.scale,
                r.right() * self.scale,
                r.bottom() * self.scale,
            ],
            None => [0.0, 0.0, self.size.0 * self.scale, self.size.1 * self.scale],
        };
    }

    // ---- shapes ----------------------------------------------------------

    fn push(
        &mut self,
        r: Rect,
        kind: f32,
        radius: f32,
        extra: f32,
        color: Color,
        border_w: f32,
        border: Color,
    ) -> &mut Quad {
        let s = self.scale;
        self.quads.push(Quad {
            rect: [r.x * s, r.y * s, r.w * s, r.h * s],
            uv: [0.0; 4],
            color: alpha(color, self.alpha),
            color2: alpha(color, self.alpha),
            grad: [0.0; 4],
            clip: self.clip,
            params: [radius * s, border_w * s, kind, extra],
            border: alpha(border, self.alpha),
        });
        self.quads.last_mut().unwrap()
    }

    /// A filled rounded rectangle.
    pub fn rect(&mut self, r: Rect, radius: f32, color: Color) {
        self.push(r, KIND_RECT, radius, 0.0, color, 0.0, CLEAR);
    }

    /// A rounded rectangle with a border.
    pub fn rect_bordered(&mut self, r: Rect, radius: f32, color: Color, border_w: f32, border: Color) {
        self.push(r, KIND_RECT, radius, 0.0, color, border_w, border);
    }

    /// A rounded rectangle filled with a linear gradient, left to right or
    /// top to bottom.
    pub fn gradient(&mut self, r: Rect, radius: f32, from: Color, to: Color, vertical: bool) {
        let (s, a) = (self.scale, self.alpha);
        let q = self.push(r, KIND_RECT, radius, 0.0, from, 0.0, CLEAR);
        q.color2 = alpha(to, a);
        q.grad = if vertical {
            [r.x * s, r.y * s, r.x * s, r.bottom() * s]
        } else {
            [r.x * s, r.y * s, r.right() * s, r.y * s]
        };
    }

    /// A pane of glass over the visualizer, and the shadow it casts.
    pub fn glass(&mut self, r: Rect, radius: f32, glass: Glass) {
        if glass.shadow > 0.0 {
            // Soft, wide and a little below: the pane floats above the picture.
            let blur = (r.w.min(r.h) * 0.3).clamp(5.0, 26.0);
            let cast = Rect::new(r.x, r.y + blur * 0.4, r.w, r.h).inset(blur * 0.2);
            self.shadow(cast, radius, blur, [0.0, 0.0, 0.0, 0.42 * glass.shadow]);
        }
        // The user's setting moves every material along the same road, from
        // clear to clouded: the haze behind it, and how strongly a film
        // covers the picture. A solid piece keeps its colour.
        let v = self.frosting.clamp(0.0, 1.0);
        let haze = if v < 0.5 { 2.0 * v } else { 1.0 + (v - 0.5) * 2.6 };
        let frost = (glass.frost * haze).min(1.0);
        let cover = if glass.solid { 1.0 } else { 0.5 + v };
        let film = |c: Color| [c[0], c[1], c[2], (c[3] * cover).min(0.96)];

        let (s, a) = (self.scale, self.alpha);
        // The shader needs the size of a logical pixel where other shapes
        // keep a border width.
        let q = self.push(r, KIND_GLASS, radius, a, glass.tint, 1.0, glass.rim);
        // For glass the tint strength is not an opacity; undo the global fade
        // on it (the pane's own opacity carries the fade instead).
        q.color = film(glass.tint);
        q.color2 = film(glass.tint_end);
        q.border = glass.rim;
        q.uv = [frost, glass.bevel, glass.bend, glass.glow];
        if glass.tint_end != glass.tint {
            q.grad = [r.x * s, r.y * s, r.right() * s, r.y * s];
        }
    }

    /// A soft shadow or glow behind `r`.
    pub fn shadow(&mut self, r: Rect, radius: f32, blur: f32, color: Color) {
        let s = self.scale;
        self.push(r, KIND_SHADOW, radius, blur * s, color, 0.0, CLEAR);
    }

    /// The current track's cover art.
    pub fn art(&mut self, r: Rect, radius: f32, opacity: f32) {
        let q = self.push(r, KIND_ART, radius, 0.0, white(opacity), 0.0, CLEAR);
        q.uv = [0.0, 0.0, 1.0, 1.0];
    }

    // ---- logo ------------------------------------------------------------

    /// The buttercandy logo, filling `r` (use [`logo::aspect`] for its shape).
    /// It is rasterised at exactly the pixel size it is drawn at.
    pub fn logo(&mut self, logo: Logo, r: Rect) {
        let w = (r.w * self.scale).round().max(1.0) as u32;
        let h = (r.h * self.scale).round().max(1.0) as u32;
        let atlas = &mut self.atlas;
        let sprite = *self.logos.entry((logo, w, h)).or_insert_with(|| {
            let uv = atlas.add(w, h, &logo::rasterize(logo, w, h, 0.0))?;
            Some(Sprite {
                uv,
                w: w as f32,
                h: h as f32,
                left: 0.0,
                top: 0.0,
                color: true,
            })
        });
        let Some(sprite) = sprite else { return };
        self.quads.push(Quad {
            rect: [
                (r.x * self.scale).round(),
                (r.y * self.scale).round(),
                w as f32,
                h as f32,
            ],
            uv: sprite.uv,
            color: white(self.alpha),
            color2: white(self.alpha),
            grad: [0.0; 4],
            clip: self.clip,
            params: [0.0, 0.0, KIND_IMAGE, 0.0],
            border: CLEAR,
        });
    }

    /// Light text and icons sit on glass with the picture moving behind
    /// them, so they carry a faint dark shadow to stay readable:
    /// how far below to draw it, in pixels, and its colour.
    fn lift(&self, ink: Color) -> Option<(f32, Color)> {
        let light = 0.299 * ink[0] + 0.587 * ink[1] + 0.114 * ink[2] > 0.6;
        light.then(|| (self.scale.round().max(1.0), [0.0, 0.0, 0.0, 0.3 * ink[3] * self.alpha]))
    }

    // ---- icons -----------------------------------------------------------

    fn icon_sprite(&mut self, icon: Icon, px: u32) -> Option<Sprite> {
        let atlas = &mut self.atlas;
        *self.icons.entry((icon, px)).or_insert_with(|| {
            let uv = atlas.add(px, px, &icons::rasterize(icon, px))?;
            Some(Sprite {
                uv,
                w: px as f32,
                h: px as f32,
                left: 0.0,
                top: 0.0,
                color: false,
            })
        })
    }

    /// An icon centred on `(cx, cy)`, `size` logical pixels across.
    pub fn icon(&mut self, icon: Icon, cx: f32, cy: f32, size: f32, color: Color) {
        self.icon_gradient(icon, cx, cy, size, color, color);
    }

    pub fn icon_gradient(&mut self, icon: Icon, cx: f32, cy: f32, size: f32, from: Color, to: Color) {
        let px = (size * self.scale).round().max(4.0);
        let Some(sprite) = self.icon_sprite(icon, px as u32) else {
            return;
        };
        let x = (cx * self.scale - px * 0.5).round();
        let y = (cy * self.scale - px * 0.5).round();
        if let Some((dy, shade)) = self.lift(from) {
            self.quads.push(Quad {
                rect: [x, y + dy, px, px],
                uv: sprite.uv,
                color: shade,
                color2: shade,
                grad: [0.0; 4],
                clip: self.clip,
                params: [0.0, 0.0, KIND_GLYPH, 0.0],
                border: CLEAR,
            });
        }
        self.quads.push(Quad {
            rect: [x, y, px, px],
            uv: sprite.uv,
            color: alpha(from, self.alpha),
            color2: alpha(to, self.alpha),
            grad: if from == to { [0.0; 4] } else { [x, y, x + px, y + px] },
            clip: self.clip,
            params: [0.0, 0.0, KIND_GLYPH, 0.0],
            border: CLEAR,
        });
    }

    // ---- text ------------------------------------------------------------

    fn shape(&mut self, s: &str, size: f32, weight: u16) -> Rc<Shaped> {
        self.text.shape(&mut self.atlas, s, size * self.scale, weight)
    }

    /// Width of `s` in logical pixels.
    pub fn text_width(&mut self, s: &str, size: f32, weight: u16) -> f32 {
        self.shape(s, size, weight).width / self.scale
    }

    /// Emit glyph quads at a physical-pixel origin, up to `limit` pixels wide.
    /// Returns the x just past the last glyph drawn.
    fn emit(&mut self, shaped: &Shaped, ox: f32, oy: f32, limit: f32, from: Color, to: Color, grad: [f32; 4]) -> f32 {
        let mut end = ox;
        let lift = self.lift(from);
        for g in &shaped.glyphs {
            if g.x + g.sprite.w > limit {
                break;
            }
            let color = g.sprite.color;
            if let Some((dy, shade)) = lift
                && !color
            {
                self.quads.push(Quad {
                    rect: [ox + g.x, oy + g.y + dy, g.sprite.w, g.sprite.h],
                    uv: g.sprite.uv,
                    color: shade,
                    color2: shade,
                    grad: [0.0; 4],
                    clip: self.clip,
                    params: [0.0, 0.0, KIND_GLYPH, 0.0],
                    border: CLEAR,
                });
            }
            self.quads.push(Quad {
                rect: [ox + g.x, oy + g.y, g.sprite.w, g.sprite.h],
                uv: g.sprite.uv,
                color: alpha(if color { white(from[3]) } else { from }, self.alpha),
                color2: alpha(to, self.alpha),
                grad: if color { [0.0; 4] } else { grad },
                clip: self.clip,
                params: [0.0, 0.0, if color { KIND_IMAGE } else { KIND_GLYPH }, 0.0],
                border: CLEAR,
            });
            end = ox + g.x + g.sprite.w;
        }
        end
    }

    /// Draw `s` with its line box's top-left at `(x, y)`. Returns its width.
    pub fn text(&mut self, s: &str, x: f32, y: f32, size: f32, weight: u16, color: Color) -> f32 {
        let shaped = self.shape(s, size, weight);
        let (ox, oy) = ((x * self.scale).round(), (y * self.scale).round());
        self.emit(&shaped, ox, oy, f32::MAX, color, color, [0.0; 4]);
        shaped.width / self.scale
    }

    /// Draw `s` vertically centred in `r`, aligned horizontally, and
    /// truncated with an ellipsis if it does not fit.
    pub fn label(&mut self, s: &str, r: Rect, size: f32, weight: u16, color: Color, align: Align) {
        let shaped = self.shape(s, size, weight);
        let scale = self.scale;
        let avail = r.w * scale;
        let oy = (r.cy() * scale - shaped.height * 0.5).round();
        if shaped.width <= avail + 0.5 {
            let ox = match align {
                Align::Left => r.x * scale,
                Align::Center => r.cx() * scale - shaped.width * 0.5,
                Align::Right => r.right() * scale - shaped.width,
            };
            self.emit(&shaped, ox.round(), oy, f32::MAX, color, color, [0.0; 4]);
        } else {
            let dots = self.shape("…", size, weight);
            let ox = (r.x * scale).round();
            let end = self.emit(&shaped, ox, oy, (avail - dots.width).max(0.0), color, color, [0.0; 4]);
            self.emit(&dots, end.round(), oy, f32::MAX, color, color, [0.0; 4]);
        }
    }
}
