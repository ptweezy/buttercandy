//! The built-in visualizer presets. Each is a WGSL fragment defining
//! `fn scene(uv: vec2f) -> vec3f`; see `shaders/preset_header.wgsl` for what
//! it can call.

/// A family of presets. The interface shows one genre at a time.
pub struct Genre {
    pub name: &'static str,
}

pub const GENRES: &[Genre] = &[Genre { name: "Classics" }, Genre { name: "Fractals" }];

pub const CLASSICS: usize = 0;
pub const FRACTALS: usize = 1;

pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    /// Which of `GENRES` it belongs to. A genre's presets sit together in
    /// `PRESETS`.
    pub genre: usize,
    pub source: &'static str,
    /// How many pixel rows along the top and the bottom edge of its frame
    /// the preset keeps as memory rather than picture. Memory means nothing
    /// at another resolution: when the render scale changes the picture is
    /// carried over and these rows start empty, as they do when the preset
    /// is first shown.
    pub memory: (u32, u32),
    /// A palette that shows it off, for `--sheet presets`.
    pub showcase: &'static str,
    /// It keeps hidden state: instead of `scene` it defines `state`, four
    /// numbers a pixel that are kept from frame to frame and never shown,
    /// and `show`, which makes the picture from them in a second pass.
    pub stateful: bool,
}

macro_rules! preset {
    ($genre:expr, $id:literal, $name:literal, $showcase:literal) => {
        preset!($genre, $id, $name, $showcase, (0, 0))
    };
    ($genre:expr, $id:literal, $name:literal, $showcase:literal, $memory:expr) => {
        Preset {
            id: $id,
            name: $name,
            genre: $genre,
            source: include_str!(concat!("shaders/presets/", $id, ".wgsl")),
            memory: $memory,
            showcase: $showcase,
            stateful: false,
        }
    };
}

pub const PRESETS: &[Preset] = &[
    preset!(CLASSICS, "taffy", "Taffy", "buttercandy", (1, 0)),
    preset!(CLASSICS, "skyline", "Skyline", "ember"),
    preset!(CLASSICS, "corona", "Corona", "aurora"),
    preset!(CLASSICS, "wormhole", "Wormhole", "lagoon"),
    preset!(CLASSICS, "synthgrid", "Synthgrid", "synthwave"),
    preset!(CLASSICS, "ribbon", "Ribbon", "toxic", (4, 5)),
    preset!(CLASSICS, "kaleido", "Kaleido", "synthwave"),
    preset!(CLASSICS, "sprinkles", "Sprinkles", "sherbet"),
    preset!(CLASSICS, "gumdrop", "Gumdrop", "buttercandy"),
    preset!(CLASSICS, "sugarplum", "Sugarplum", "jellybean"),
    Preset {
        stateful: true,
        ..preset!(FRACTALS, "jawbreaker", "Jawbreaker", "jellybean")
    },
    preset!(FRACTALS, "gumball", "Gumball", "jellybean"),
    preset!(FRACTALS, "gingerbread", "Gingerbread", "caramel"),
];

pub const FULLSCREEN: &str = include_str!("shaders/fullscreen.wgsl");
const HEADER: &str = include_str!("shaders/preset_header.wgsl");
const FRACTAL: &str = include_str!("shaders/fractal.wgsl");
const FOOTER: &str = include_str!("shaders/preset_footer.wgsl");
const MAIN: &str = include_str!("shaders/preset_main.wgsl");
const STATE: &str = include_str!("shaders/preset_state.wgsl");

/// The complete shader module source for a preset's body. The body goes
/// first (WGSL does not mind the order of declarations) so that the line
/// numbers in a compile error are the line numbers of the preset's own file.
/// The fractals also get what their genre shares, and a preset that keeps
/// state gets the two passes that go with it.
pub fn assemble(body: &str, preset: &Preset) -> String {
    let shared = if preset.genre == FRACTALS { FRACTAL } else { "" };
    let passes = if preset.stateful { STATE } else { MAIN };
    format!("{body}\n{FULLSCREEN}\n{HEADER}\n{shared}\n{FOOTER}\n{passes}")
}

pub fn index_of(id: &str) -> usize {
    PRESETS.iter().position(|p| p.id == id).unwrap_or(0)
}

/// The presets of a genre, as indices into `PRESETS`, in order.
pub fn in_genre(genre: usize) -> impl Iterator<Item = usize> + Clone {
    (0..PRESETS.len()).filter(move |&i| PRESETS[i].genre == genre)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse and validate a complete WGSL module, without a GPU.
    fn validate(name: &str, source: &str) {
        let module =
            naga::front::wgsl::parse_str(source).unwrap_or_else(|e| panic!("{name}:\n{}", e.emit_to_string(source)));
        let mut validator =
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty());
        if let Err(e) = validator.validate(&module) {
            panic!("{name}:\n{}", e.emit_to_string(source));
        }
    }

    #[test]
    fn every_preset_is_valid_wgsl() {
        for p in PRESETS {
            validate(p.id, &assemble(p.source, p));
        }
    }

    #[test]
    fn pipeline_shaders_are_valid_wgsl() {
        validate("post", &format!("{FULLSCREEN}\n{}", include_str!("shaders/post.wgsl")));
        validate("blur", &format!("{FULLSCREEN}\n{}", include_str!("shaders/blur.wgsl")));
        validate("ui", include_str!("shaders/ui.wgsl"));
    }

    /// The memory rows declared in the table are the ones the shaders keep.
    #[test]
    fn memory_rows_match_the_shaders() {
        let source = |id| PRESETS[index_of(id)].source;
        for p in PRESETS {
            let (top, bottom) = match p.id {
                // One pixel, in the top row.
                "taffy" => (1, 0),
                "ribbon" => (4, 5),
                _ => (0, 0),
            };
            assert_eq!(p.memory, (top, bottom), "{}", p.id);
        }
        assert!(source("taffy").contains("textureLoad(tPrev, vec2i(0, 0), 0)"));
        assert!(source("ribbon").contains("const TOP_ROWS: i32 = 4;"));
        assert!(source("ribbon").contains("const BOTTOM_ROWS: i32 = 5;"));
        // Nothing else reads its previous frame texel by texel, but for the
        // presets whose whole frame is state.
        for p in PRESETS.iter().filter(|p| p.memory == (0, 0) && !p.stateful) {
            assert!(!p.source.contains("textureLoad(tPrev"), "{} keeps memory", p.id);
        }
    }

    /// Every genre has presets, and they sit together, so that stepping
    /// through `PRESETS` never leaves a genre and comes back to it.
    #[test]
    fn genres_are_contiguous() {
        for (g, genre) in GENRES.iter().enumerate() {
            let members: Vec<usize> = in_genre(g).collect();
            assert!(!members.is_empty(), "genre {} is empty", genre.name);
            assert_eq!(
                members.last().unwrap() - members[0] + 1,
                members.len(),
                "{}",
                genre.name
            );
        }
        assert!(PRESETS.iter().all(|p| p.genre < GENRES.len()));
    }

    #[test]
    fn showcase_palettes_exist() {
        for p in PRESETS {
            assert!(
                crate::palette::PALETTES.iter().any(|pal| pal.id == p.showcase),
                "{} names palette {}",
                p.id,
                p.showcase
            );
        }
    }

    #[test]
    fn preset_ids_are_unique() {
        for (i, p) in PRESETS.iter().enumerate() {
            assert_eq!(index_of(p.id), i, "duplicate preset id {}", p.id);
        }
    }
}
