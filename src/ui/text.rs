//! Text shaping and glyph rasterisation via cosmic-text, using system fonts
//! (with script and emoji fallback) so any track title renders.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent, Weight, Wrap, fontdb,
};

use super::atlas::{Atlas, Sprite};

pub struct Glyph {
    pub sprite: Sprite,
    /// Top-left of the bitmap relative to the text's top-left, in pixels.
    pub x: f32,
    pub y: f32,
}

pub struct Shaped {
    pub width: f32,
    pub height: f32,
    pub glyphs: Vec<Glyph>,
}

struct Entry {
    shaped: Rc<Shaped>,
    last_used: u64,
}

pub struct Text {
    fonts: FontSystem,
    /// The full system font database, while it is still loading.
    pending: Option<Receiver<fontdb::Database>>,
    locale: String,
    swash: SwashCache,
    family: String,
    glyphs: HashMap<CacheKey, Option<Sprite>>,
    shaped: HashMap<(String, u32, u16), Entry>,
    frame: u64,
}

/// The interface typeface: the platform's own UI font where we can find it.
fn pick_family(fonts: &FontSystem) -> String {
    let wanted: &[&str] = if cfg!(target_os = "windows") {
        &["Segoe UI", "Arial"]
    } else if cfg!(target_os = "macos") {
        &[".SF NS", "SF Pro Text", "Helvetica Neue", "Helvetica"]
    } else {
        &[
            "Inter",
            "Cantarell",
            "Ubuntu",
            "Noto Sans",
            "DejaVu Sans",
            "Liberation Sans",
        ]
    };
    let db = fonts.db();
    for name in wanted {
        if db.faces().any(|f| f.families.iter().any(|(family, _)| family == name)) {
            return name.to_string();
        }
    }
    db.faces()
        .find_map(|f| f.families.first().map(|(family, _)| family.clone()))
        .unwrap_or_else(|| "sans-serif".into())
}

/// Font files that are enough to draw the interface straight away, where the
/// platform keeps its UI font in a known place.
fn quick_font_files() -> Vec<PathBuf> {
    if cfg!(target_os = "windows") {
        let root = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        ["segoeui.ttf", "seguisb.ttf", "segoeuib.ttf", "seguibl.ttf"]
            .iter()
            .map(|f| root.join("Fonts").join(f))
            .filter(|p| p.exists())
            .collect()
    } else {
        Vec::new()
    }
}

impl Text {
    /// Scanning every installed font can take seconds on a cold disk, so when
    /// we can, start with just the interface font and load the rest (needed
    /// for other scripts and emoji) in the background.
    pub fn new() -> Self {
        let started = std::time::Instant::now();
        let locale = sys_locale::get_locale().unwrap_or_else(|| "en-US".into());
        let mut quick = fontdb::Database::new();
        for file in quick_font_files() {
            let _ = quick.load_font_file(file);
        }
        let (fonts, pending) = if quick.is_empty() {
            (FontSystem::new(), None)
        } else {
            let (tx, rx) = mpsc::channel();
            let _ = std::thread::Builder::new().name("fonts".into()).spawn(move || {
                let mut db = fontdb::Database::new();
                db.load_system_fonts();
                let _ = tx.send(db);
            });
            (FontSystem::new_with_locale_and_db(locale.clone(), quick), Some(rx))
        };
        let family = pick_family(&fonts);
        log::info!(
            "interface font {family} ready in {:.0} ms",
            started.elapsed().as_secs_f32() * 1000.0
        );
        Self {
            fonts,
            pending,
            locale,
            swash: SwashCache::new(),
            family,
            glyphs: HashMap::new(),
            shaped: HashMap::new(),
            frame: 0,
        }
    }

    /// Switch to the full font database once it has loaded. Returns true when
    /// that happened; every cached glyph is then stale.
    pub fn poll_fonts(&mut self) -> bool {
        let Some(rx) = &self.pending else { return false };
        match rx.try_recv() {
            Ok(db) => {
                log::info!("system fonts loaded ({} faces)", db.len());
                self.fonts = FontSystem::new_with_locale_and_db(self.locale.clone(), db);
                self.family = pick_family(&self.fonts);
                self.swash = SwashCache::new();
                self.reset();
                self.pending = None;
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.pending = None;
                false
            }
        }
    }

    fn glyph(&mut self, atlas: &mut Atlas, key: CacheKey) -> Option<Sprite> {
        if let Some(sprite) = self.glyphs.get(&key) {
            return *sprite;
        }
        let sprite = self.swash.get_image_uncached(&mut self.fonts, key).and_then(|img| {
            let (w, h) = (img.placement.width, img.placement.height);
            let (rgba, color): (Vec<u8>, bool) = match img.content {
                SwashContent::Mask => (img.data.iter().flat_map(|&a| [255, 255, 255, a]).collect(), false),
                SwashContent::Color => (img.data.clone(), true),
                // Subpixel masks are not requested; fall back to their green channel.
                SwashContent::SubpixelMask => (
                    img.data
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .flat_map(|p| [255, 255, 255, p[1]])
                        .collect(),
                    false,
                ),
            };
            let uv = atlas.add(w, h, &rgba)?;
            Some(Sprite {
                uv,
                w: w as f32,
                h: h as f32,
                left: img.placement.left as f32,
                top: img.placement.top as f32,
                color,
            })
        });
        // A failed allocation must be retried after the atlas is rebuilt.
        if sprite.is_some() || !atlas.full {
            self.glyphs.insert(key, sprite);
        }
        sprite
    }

    /// Shape `text` at `px` pixels. Results are cached between frames.
    pub fn shape(&mut self, atlas: &mut Atlas, text: &str, px: f32, weight: u16) -> Rc<Shaped> {
        let key = (text.to_string(), (px * 4.0).round() as u32, weight);
        if let Some(entry) = self.shaped.get_mut(&key) {
            entry.last_used = self.frame;
            return entry.shaped.clone();
        }

        let line_height = (px * 1.3).ceil();
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(px, line_height));
        buffer.set_wrap(Wrap::None);
        buffer.set_size(None, None);
        let attrs = Attrs::new().family(Family::Name(&self.family)).weight(Weight(weight));
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.fonts, false);

        let mut width = 0.0f32;
        let mut placed = Vec::new();
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            for glyph in run.glyphs {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                placed.push((
                    physical.cache_key,
                    physical.x as f32,
                    run.line_y.round() + physical.y as f32,
                ));
            }
        }
        let mut glyphs = Vec::with_capacity(placed.len());
        let mut complete = true;
        for (cache_key, x, baseline) in placed {
            match self.glyph(atlas, cache_key) {
                Some(sprite) => glyphs.push(Glyph {
                    sprite,
                    x: x + sprite.left,
                    y: baseline - sprite.top,
                }),
                None => complete &= !atlas.full,
            }
        }
        let shaped = Rc::new(Shaped {
            width: width.ceil(),
            height: line_height,
            glyphs,
        });
        if complete {
            self.shaped.insert(
                key,
                Entry {
                    shaped: shaped.clone(),
                    last_used: self.frame,
                },
            );
        }
        shaped
    }

    /// Call once per frame: evicts text that has not been drawn for a while.
    pub fn end_frame(&mut self) {
        self.frame += 1;
        if self.frame.is_multiple_of(240) {
            let cutoff = self.frame.saturating_sub(600);
            self.shaped.retain(|_, e| e.last_used >= cutoff);
        }
    }

    /// Drop every cached sprite (after the atlas was cleared).
    pub fn reset(&mut self) {
        self.glyphs.clear();
        self.shaped.clear();
    }
}
