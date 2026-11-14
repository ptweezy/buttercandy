//! Colour palettes. Each is a cyclic gradient the shaders sample with `pal(t)`,
//! plus a pair of accents that re-skin the interface to match.

pub struct Palette {
    pub id: &'static str,
    pub name: &'static str,
    /// sRGB stops, evenly spaced, wrapping from the last back to the first.
    pub stops: &'static [u32],
    /// The colour each voice of the music owns, in every preset: kick,
    /// snare, hats, bass line, melody. Chosen to be as far apart as the
    /// palette allows, so that what a thing is reacting to can be told by
    /// its colour.
    pub voices: [u32; 5],
    /// Interface accent gradient.
    pub accent: [u32; 2],
}

pub const PALETTES: &[Palette] = &[
    Palette {
        id: "buttercandy",
        name: "Buttercandy",
        stops: &[0xff4f9a, 0xff8c61, 0xffd166, 0xfff3b0, 0xffa8d8, 0xc77dff],
        voices: [0xff4f9a, 0xffd166, 0xfff3b0, 0xc77dff, 0xff8c61],
        accent: [0xffd166, 0xff4f9a],
    },
    Palette {
        id: "aurora",
        name: "Aurora",
        stops: &[0x00f5a0, 0x00d9f5, 0x4d7cff, 0xa855f7, 0x2ef0d0],
        voices: [0xa855f7, 0x00f5a0, 0xc8fff2, 0x4d7cff, 0x00d9f5],
        accent: [0x00f5a0, 0x4d7cff],
    },
    Palette {
        id: "synthwave",
        name: "Synthwave",
        stops: &[0xff2a6d, 0xff6ac1, 0xb967ff, 0x3d5afe, 0x05d9e8, 0x7a04eb],
        voices: [0xff2a6d, 0x05d9e8, 0xffc2ea, 0x3d5afe, 0xb967ff],
        accent: [0x05d9e8, 0xff2a6d],
    },
    Palette {
        id: "ember",
        name: "Ember",
        stops: &[0xff3c00, 0xff7b00, 0xffb300, 0xffe08a, 0xff5e3a, 0xc1121f],
        voices: [0xff3c00, 0xffb300, 0xffe08a, 0xc1121f, 0xff7b00],
        accent: [0xffb300, 0xff3c00],
    },
    Palette {
        id: "lagoon",
        name: "Lagoon",
        stops: &[0x0077ff, 0x00b4d8, 0x48f0c8, 0xcaf0f8, 0x5390ff, 0x3a0ca3],
        voices: [0x0077ff, 0x48f0c8, 0xcaf0f8, 0x7b4dff, 0x00b4d8],
        accent: [0x48f0c8, 0x0077ff],
    },
    Palette {
        id: "sherbet",
        name: "Sherbet",
        stops: &[0xff9aa2, 0xffb7b2, 0xffdac1, 0xe2f0cb, 0xb5ead7, 0xc7ceea],
        voices: [0xff9aa2, 0xb5ead7, 0xfff4e6, 0xc7ceea, 0xffdac1],
        accent: [0xffdac1, 0xc7ceea],
    },
    Palette {
        id: "toxic",
        name: "Toxic",
        stops: &[0xaaff00, 0x39ff14, 0x00ff87, 0xd4ff3a, 0x00c2a8],
        voices: [0x39ff14, 0xd4ff3a, 0xeaffb0, 0x00c2a8, 0x00ff87],
        accent: [0xaaff00, 0x00ff87],
    },
    Palette {
        id: "noir",
        name: "Noir",
        stops: &[0xffffff, 0xb8c0cc, 0xf5f7fa, 0x8d99ae, 0xe9ecef],
        voices: [0xffffff, 0x8d99ae, 0xe9ecef, 0x5c677d, 0xb8c0cc],
        accent: [0xffffff, 0x8d99ae],
    },
    // Pink and baby blue.
    Palette {
        id: "bubblegum",
        name: "Bubblegum",
        stops: &[0xff5fa2, 0xff9ecf, 0xffd6ec, 0x9ad8ff, 0x5aa9ff, 0xb48cff],
        voices: [0xff5fa2, 0x7fe0ff, 0xfff0f8, 0x4f7dff, 0xc79bff],
        accent: [0xff9ecf, 0x5aa9ff],
    },
    // Lemon, lime, orange and pink grapefruit.
    Palette {
        id: "citrus",
        name: "Citrus",
        stops: &[0xffe14d, 0xc6f23c, 0x4be07a, 0xffa62b, 0xff6b4a, 0xff4d8d],
        voices: [0xff9a1f, 0xffe14d, 0xfffbd1, 0x3ddc84, 0xff4d8d],
        accent: [0xffe14d, 0xff6b4a],
    },
    // Candy-cane red and white, with mint.
    Palette {
        id: "peppermint",
        name: "Peppermint",
        stops: &[0xff3b4e, 0xffd0d6, 0xffffff, 0x9af5d6, 0x20c997, 0xff7a88],
        voices: [0xff3b4e, 0xffffff, 0xd6fff2, 0x14a37f, 0xff8fa0],
        accent: [0xff3b4e, 0x20c997],
    },
    // Cream, butterscotch, toffee: one warm hue from light to dark.
    Palette {
        id: "caramel",
        name: "Caramel",
        stops: &[0xfff0d6, 0xf6c98a, 0xe2a45e, 0xc7772e, 0x9c5a2c, 0xffdcae],
        voices: [0xd9843a, 0xffe2b0, 0xfffaf0, 0x9c5a2c, 0xf6b85a],
        accent: [0xf6c98a, 0xc7772e],
    },
    // The whole rainbow, so the five voices are as far apart as colours get.
    Palette {
        id: "jellybean",
        name: "Jellybean",
        stops: &[0xff3355, 0xff8a1f, 0xffe01f, 0x3fe06a, 0x1fc4ff, 0x6a5cff, 0xe04dff],
        voices: [0xff3355, 0xffe01f, 0xeaffff, 0x5a6bff, 0x3fe06a],
        accent: [0xff8a1f, 0xe04dff],
    },
    // Night blues and silver, with the kick in gold.
    Palette {
        id: "moonlight",
        name: "Moonlight",
        stops: &[0x2f44b8, 0x4f6bff, 0x9fb4ff, 0xe8ecff, 0xffd98a, 0x6a4bd6],
        voices: [0xffc857, 0xf2f5ff, 0xbcd0ff, 0x3350e0, 0x9a7bff],
        accent: [0xffd98a, 0x4f6bff],
    },
    // Ice: white through pale blue to periwinkle.
    Palette {
        id: "frost",
        name: "Frost",
        stops: &[0xeafcff, 0xaee9ff, 0x6ecbff, 0x7f9bff, 0xc9c2ff, 0xffffff],
        voices: [0x4fb6ff, 0xffffff, 0xdff8ff, 0x6f7dff, 0x8ff2e3],
        accent: [0xaee9ff, 0x7f9bff],
    },
    // A thermal camera: purple through magenta and orange to pale yellow.
    Palette {
        id: "inferno",
        name: "Inferno",
        stops: &[0x5a1a9a, 0xa02c8f, 0xe34a6a, 0xff9c5e, 0xffe89a],
        voices: [0xe8436b, 0xffe89a, 0xfffbe6, 0x7a2bc4, 0xff9c5e],
        accent: [0xff9c5e, 0xa02c8f],
    },
];

pub const PALETTE_TEXELS: usize = 256;
pub const PALETTE_ROWS: usize = 2;

/// `0xRRGGBB` to sRGB floats.
pub fn rgb(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 255) as f32 / 255.0,
        ((hex >> 8) & 255) as f32 / 255.0,
        (hex & 255) as f32 / 255.0,
    ]
}

impl Palette {
    /// sRGB colour at `t` (cyclic).
    pub fn color(&self, t: f32) -> [f32; 3] {
        let n = self.stops.len();
        let x = t.rem_euclid(1.0) * n as f32;
        let i = (x as usize).min(n - 1);
        let f = x - i as f32;
        // Ease between stops so each colour gets a little room to itself.
        let f = f * f * (3.0 - 2.0 * f);
        let (a, b) = (rgb(self.stops[i]), rgb(self.stops[(i + 1) % n]));
        [
            a[0] + (b[0] - a[0]) * f,
            a[1] + (b[1] - a[1]) * f,
            a[2] + (b[2] - a[2]) * f,
        ]
    }

    /// RGBA8 sRGB texels for the palette texture, `PALETTE_TEXELS` wide and
    /// `PALETTE_ROWS` high: the gradient, then the voice colours in five
    /// equal blocks.
    pub fn texels(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(PALETTE_TEXELS * PALETTE_ROWS * 4);
        let mut put = |c: [f32; 3]| {
            out.extend_from_slice(&[
                (c[0] * 255.0 + 0.5) as u8,
                (c[1] * 255.0 + 0.5) as u8,
                (c[2] * 255.0 + 0.5) as u8,
                255,
            ]);
        };
        for i in 0..PALETTE_TEXELS {
            // Sample at texel centres so the texture wraps seamlessly.
            put(self.color((i as f32 + 0.5) / PALETTE_TEXELS as f32));
        }
        for i in 0..PALETTE_TEXELS {
            put(rgb(self.voices[(i * 5 / PALETTE_TEXELS).min(4)]));
        }
        out
    }
}

pub fn index_of(id: &str) -> usize {
    PALETTES.iter().position(|p| p.id == id).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palettes_are_well_formed() {
        for (i, p) in PALETTES.iter().enumerate() {
            assert_eq!(index_of(p.id), i, "duplicate palette id {}", p.id);
            assert!(p.stops.len() >= 3, "{} needs a gradient", p.id);
            assert_eq!(p.texels().len(), PALETTE_TEXELS * PALETTE_ROWS * 4);
        }
    }

    /// How far apart two colours look, roughly: distance in sRGB, weighted
    /// towards green, on a 0..1 scale.
    fn apart(a: u32, b: u32) -> f32 {
        let (a, b) = (rgb(a), rgb(b));
        let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        (0.3 * d[0] * d[0] + 0.5 * d[1] * d[1] + 0.2 * d[2] * d[2]).sqrt()
    }

    #[test]
    fn each_voice_has_a_colour_of_its_own() {
        for p in PALETTES {
            for a in 0..5 {
                for b in a + 1..5 {
                    let d = apart(p.voices[a], p.voices[b]);
                    assert!(d > 0.045, "{}: voices {a} and {b} are {d:.3} apart", p.id);
                }
            }
        }
    }
}
