//! A shelf-packed texture atlas holding rasterised glyphs and icons.

pub const ATLAS_SIZE: u32 = 2048;

/// Where a glyph or icon lives in the atlas, and how to place it.
#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub uv: [f32; 4],
    pub w: f32,
    pub h: f32,
    /// Offset from the pen position to the bitmap's left edge.
    pub left: f32,
    /// Distance from the baseline up to the bitmap's top edge.
    pub top: f32,
    /// Full-colour bitmap (emoji) rather than a coverage mask.
    pub color: bool,
}

pub struct Atlas {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    queue: wgpu::Queue,
    x: u32,
    y: u32,
    row_h: u32,
    /// Set when an allocation failed; the owner clears and rebuilds.
    pub full: bool,
}

impl Atlas {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ui atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            texture,
            view,
            queue: queue.clone(),
            x: 1,
            y: 1,
            row_h: 0,
            full: false,
        }
    }

    /// Upload a `w x h` RGBA image and return its uv rectangle.
    pub fn add(&mut self, w: u32, h: u32, rgba: &[u8]) -> Option<[f32; 4]> {
        if w == 0 || h == 0 || w + 2 > ATLAS_SIZE || h + 2 > ATLAS_SIZE {
            return None;
        }
        // One texel of gutter keeps bilinear sampling from bleeding.
        if self.x + w + 1 > ATLAS_SIZE {
            self.x = 1;
            self.y += self.row_h + 1;
            self.row_h = 0;
        }
        if self.y + h + 1 > ATLAS_SIZE {
            self.full = true;
            return None;
        }
        let (x, y) = (self.x, self.y);
        self.x += w + 1;
        self.row_h = self.row_h.max(h);
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let s = ATLAS_SIZE as f32;
        Some([x as f32 / s, y as f32 / s, (x + w) as f32 / s, (y + h) as f32 / s])
    }

    /// Forget everything. Callers must drop any sprites they cached.
    pub fn clear(&mut self) {
        self.x = 1;
        self.y = 1;
        self.row_h = 0;
        self.full = false;
        // Wipe stale texels so gutters are transparent again.
        let zeros = vec![0u8; (ATLAS_SIZE * ATLAS_SIZE * 4) as usize];
        crate::gfx::renderer::write_texture(&self.queue, &self.texture, 4, &zeros);
    }
}
