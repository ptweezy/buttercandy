//! The visualizer pipeline:
//!
//! ```text
//! preset (HDR, with feedback) ─┬─> bloom chain ─┐
//!                              └────────────────┴─> composite ─> frame ─> blurred backdrop
//! ```
//!
//! `frame` is the finished picture; the caller blits it to the screen and
//! draws the interface over it, sampling `backdrop` for frosted glass.

use bytemuck::{Pod, Zeroable};

use super::Gpu;
use super::driver::{self, DATA_H, DATA_W, Driver, PARAMS};
use super::presets::{self, PRESETS, Preset};
use crate::audio::analysis::{Analysis, BINS, HIST_H, HIST_W, ROWS};
use crate::palette::{PALETTE_ROWS, PALETTE_TEXELS, Palette};

const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const FRAME_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const BLOOM_LEVELS: usize = 6;
const BACKDROP_LEVELS: usize = 4;
const TRANSITION_SECS: f32 = 1.8;

/// What every preset is told each frame. Mirrors `Globals` in
/// `shaders/preset_header.wgsl`, which says what each field holds.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
    d: [f32; 4],
    e: [f32; 4],
    f: [f32; 4],
    beat_ages: [f32; 4],
    low_ages: [f32; 4],
    mid_ages: [f32; 4],
    high_ages: [f32; 4],
    g: [f32; 4],
    note_ages: [f32; 4],
    kick_hits: [f32; 4],
    snare_hits: [f32; 4],
    hat_hits: [f32; 4],
    note_hits: [f32; 4],
    h: [f32; 4],
    drive: [[f32; 4]; PARAMS],
}

/// Mirrors `Post` in `shaders/post.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Post {
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
    d: [f32; 4],
}

struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Texture + sampler, for passes that read this target.
    bind: wgpu::BindGroup,
}

/// A preset being drawn, with its feedback ping-pong pair.
struct Slot {
    preset: usize,
    targets: [Target; 2],
    front: usize,
    needs_clear: bool,
    /// For a preset that keeps state, `targets` hold the state and this the
    /// picture made from it. Made when such a preset is first drawn here.
    shown: Option<Target>,
}

pub struct Renderer {
    /// Bloom amount, 0..2.
    pub glow: f32,
    render_scale: f32,
    out_size: (u32, u32),
    scene_size: (u32, u32),

    tex_bgl: wgpu::BindGroupLayout,
    post_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// One of each to a slot: two presets drawn in the same frame (during a
    /// cross-dissolve) each see their own driver's numbers and table.
    globals_buf: [wgpu::Buffer; 2],
    globals_bind: [wgpu::BindGroup; 2],
    data_tex: [wgpu::Texture; 2],
    /// Each preset's driver, if it has one.
    drivers: Vec<Option<Box<dyn Driver>>>,
    post_buf: wgpu::Buffer,
    audio_tex: wgpu::Texture,
    hist_tex: wgpu::Texture,
    pal_tex: wgpu::Texture,

    preset_layout: wgpu::PipelineLayout,
    preset_pipelines: Vec<Option<wgpu::RenderPipeline>>,
    /// The second pass of the presets that keep state.
    show_pipelines: Vec<Option<wgpu::RenderPipeline>>,
    prefilter: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    bloom_down: wgpu::RenderPipeline,
    bloom_up: wgpu::RenderPipeline,
    blur_down: wgpu::RenderPipeline,
    blur_up: wgpu::RenderPipeline,
    resample: wgpu::RenderPipeline,
    present: wgpu::RenderPipeline,

    slots: [Slot; 2],
    bloom: Vec<Target>,
    frame: Target,
    backdrop: Vec<Target>,

    current: usize,
    transition: Option<f32>,
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn sampler(device: &wgpu::Device, u: wgpu::AddressMode, v: wgpu::AddressMode) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: u,
        address_mode_v: v,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    })
}

fn data_texture(device: &wgpu::Device, label: &str, w: u32, h: u32, format: wgpu::TextureFormat) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// Upload a whole texture's worth of tightly packed texels.
pub fn write_texture(queue: &wgpu::Queue, texture: &wgpu::Texture, bytes_per_texel: u32, data: &[u8]) {
    let (w, h) = (texture.width(), texture.height());
    queue.write_texture(
        texture.as_image_copy(),
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * bytes_per_texel),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
}

/// A pipeline that draws one fullscreen triangle with `fs_entry`.
pub fn fullscreen_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    fs_entry: &str,
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(fs_entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// Begin a single-target render pass.
pub fn begin<'a>(encoder: &'a mut wgpu::CommandEncoder, view: &wgpu::TextureView, clear: bool) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: if clear {
                    wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                } else {
                    wgpu::LoadOp::Load
                },
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

/// Compile a preset body into its pipeline (and, if the preset keeps state,
/// the pipeline of its second pass), or say why it would not compile.
fn compile_preset(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    preset: &Preset,
    body: &str,
) -> Result<(wgpu::RenderPipeline, Option<wgpu::RenderPipeline>), String> {
    let id = preset.id;
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(id),
        source: wgpu::ShaderSource::Wgsl(presets::assemble(body, preset).into()),
    });
    let pipeline = fullscreen_pipeline(device, id, layout, &module, "fs_main", SCENE_FORMAT, None);
    let show = preset
        .stateful
        .then(|| fullscreen_pipeline(device, id, layout, &module, "fs_show", SCENE_FORMAT, None));
    match pollster::block_on(scope.pop()) {
        None => Ok((pipeline, show)),
        Some(err) => Err(err.to_string()),
    }
}

fn smootherstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

impl Renderer {
    pub fn new(
        gpu: &Gpu,
        out_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        preset: usize,
        palette: &Palette,
    ) -> Self {
        let device = &gpu.device;
        let started = std::time::Instant::now();

        let tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture + sampler"),
            entries: &[texture_entry(0), sampler_entry(1)],
        });
        let globals_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("preset globals"),
            entries: &[
                uniform_entry(0),
                texture_entry(1),
                texture_entry(2),
                texture_entry(3),
                sampler_entry(4),
                sampler_entry(5),
                sampler_entry(6),
                // A driver's table: 32-bit floats, read texel by texel.
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let post_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post"),
            entries: &[
                uniform_entry(0),
                texture_entry(1),
                texture_entry(2),
                texture_entry(3),
                sampler_entry(4),
            ],
        });

        let clamp = wgpu::AddressMode::ClampToEdge;
        let repeat = wgpu::AddressMode::Repeat;
        let sampler_clamp = sampler(device, clamp, clamp);
        let sampler_hist = sampler(device, clamp, repeat);
        let sampler_pal = sampler(device, repeat, clamp);

        let audio_tex = data_texture(device, "audio", BINS as u32, ROWS as u32, wgpu::TextureFormat::R8Unorm);
        let hist_tex = data_texture(
            device,
            "history",
            HIST_W as u32,
            HIST_H as u32,
            wgpu::TextureFormat::R8Unorm,
        );
        let pal_tex = data_texture(
            device,
            "palette",
            PALETTE_TEXELS as u32,
            PALETTE_ROWS as u32,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );

        let uniform = |label, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let post_buf = uniform("post", size_of::<Post>() as u64);

        let view = |t: &wgpu::Texture| t.create_view(&Default::default());
        let globals_buf = [0, 1].map(|_| uniform("globals", size_of::<Globals>() as u64));
        let data_tex =
            [0, 1].map(|_| data_texture(device, "driver table", DATA_W, DATA_H, wgpu::TextureFormat::Rgba32Float));
        let globals_bind = [0, 1].map(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("preset globals"),
                layout: &globals_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: globals_buf[i].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view(&audio_tex)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&view(&hist_tex)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&view(&pal_tex)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&sampler_clamp),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&sampler_hist),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::Sampler(&sampler_pal),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: wgpu::BindingResource::TextureView(&view(&data_tex[i])),
                    },
                ],
            })
        });

        let layout = |label, groups: &[Option<&wgpu::BindGroupLayout>]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: groups,
                immediate_size: 0,
            })
        };
        let preset_layout = layout("preset", &[Some(&globals_bgl), Some(&tex_bgl)]);
        let post_layout = layout("post", &[Some(&post_bgl)]);
        let tex_layout = layout("texture", &[Some(&tex_bgl)]);

        let module = |label, body: &str| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(format!("{}\n{body}", presets::FULLSCREEN).into()),
            })
        };
        let post_module = module("post", include_str!("shaders/post.wgsl"));
        let blur_module = module("blur", include_str!("shaders/blur.wgsl"));

        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };
        let prefilter = fullscreen_pipeline(
            device,
            "bloom prefilter",
            &post_layout,
            &post_module,
            "fs_prefilter",
            SCENE_FORMAT,
            None,
        );
        let composite = fullscreen_pipeline(
            device,
            "composite",
            &post_layout,
            &post_module,
            "fs_composite",
            FRAME_FORMAT,
            None,
        );
        let bloom_down = fullscreen_pipeline(
            device,
            "bloom down",
            &tex_layout,
            &blur_module,
            "fs_down",
            SCENE_FORMAT,
            None,
        );
        let bloom_up = fullscreen_pipeline(
            device,
            "bloom up",
            &tex_layout,
            &blur_module,
            "fs_up",
            SCENE_FORMAT,
            Some(additive),
        );
        let blur_down = fullscreen_pipeline(
            device,
            "backdrop down",
            &tex_layout,
            &blur_module,
            "fs_down",
            FRAME_FORMAT,
            None,
        );
        let blur_up = fullscreen_pipeline(
            device,
            "backdrop up",
            &tex_layout,
            &blur_module,
            "fs_up",
            FRAME_FORMAT,
            None,
        );
        let resample = fullscreen_pipeline(
            device,
            "resample",
            &tex_layout,
            &blur_module,
            "fs_copy",
            SCENE_FORMAT,
            None,
        );
        let present_entry = if out_format.is_srgb() {
            "fs_blit_linear"
        } else {
            "fs_blit"
        };
        let present = fullscreen_pipeline(
            device,
            "present",
            &tex_layout,
            &blur_module,
            present_entry,
            out_format,
            None,
        );

        // Compile every preset up front; a broken one is skipped, not fatal.
        let (preset_pipelines, show_pipelines) = PRESETS
            .iter()
            .map(|p| {
                compile_preset(device, &preset_layout, p, p.source)
                    .inspect_err(|err| log::error!("preset '{}' failed to compile:\n{err}", p.id))
                    .map_or((None, None), |(main, show)| (Some(main), show))
            })
            .unzip();

        let (slots, bloom, frame, backdrop, scene_size) =
            Self::build_targets(device, &tex_bgl, &sampler_clamp, (width, height), 1.0, preset);

        let r = Self {
            glow: 0.7,
            render_scale: 1.0,
            out_size: (width.max(1), height.max(1)),
            scene_size,
            tex_bgl,
            post_bgl,
            sampler: sampler_clamp,
            globals_buf,
            globals_bind,
            data_tex,
            drivers: PRESETS.iter().map(|p| driver::for_preset(p.id)).collect(),
            post_buf,
            audio_tex,
            hist_tex,
            pal_tex,
            preset_layout,
            preset_pipelines,
            show_pipelines,
            prefilter,
            composite,
            bloom_down,
            bloom_up,
            blur_down,
            blur_up,
            resample,
            present,
            slots,
            bloom,
            frame,
            backdrop,
            current: 0,
            transition: None,
        };
        r.set_palette(gpu, palette);
        log::info!(
            "renderer ready in {:.0} ms ({} presets)",
            started.elapsed().as_secs_f32() * 1000.0,
            PRESETS.len()
        );
        r
    }

    fn target(
        device: &wgpu::Device,
        bgl: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        label: &str,
        w: u32,
        h: u32,
        format: wgpu::TextureFormat,
    ) -> Target {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: w.max(1),
                height: h.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        Target { texture, view, bind }
    }

    /// The targets drawn at the render scale: each slot's feedback pair and
    /// the bloom chain, with the size they came out at.
    fn scene_targets(
        device: &wgpu::Device,
        bgl: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        out: (u32, u32),
        scale: f32,
        preset: usize,
    ) -> ([Slot; 2], Vec<Target>, (u32, u32)) {
        let sw = ((out.0.max(1) as f32 * scale).round() as u32).max(1);
        let sh = ((out.1.max(1) as f32 * scale).round() as u32).max(1);
        let make = |label: &str, w, h, format| Self::target(device, bgl, sampler, label, w, h, format);
        let slot = |preset| Slot {
            preset,
            targets: [make("scene", sw, sh, SCENE_FORMAT), make("scene", sw, sh, SCENE_FORMAT)],
            front: 0,
            needs_clear: true,
            shown: None,
        };
        let mut bloom = Vec::new();
        for i in 0..BLOOM_LEVELS {
            let (w, h) = (sw >> (i + 1), sh >> (i + 1));
            if i > 0 && w.min(h) < 4 {
                break;
            }
            bloom.push(make("bloom", w, h, SCENE_FORMAT));
        }
        ([slot(preset), slot(preset)], bloom, (sw, sh))
    }

    #[allow(clippy::type_complexity)]
    fn build_targets(
        device: &wgpu::Device,
        bgl: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        out: (u32, u32),
        scale: f32,
        preset: usize,
    ) -> ([Slot; 2], Vec<Target>, Target, Vec<Target>, (u32, u32)) {
        let (ow, oh) = (out.0.max(1), out.1.max(1));
        let (slots, bloom, scene_size) = Self::scene_targets(device, bgl, sampler, out, scale, preset);
        let make = |label: &str, w, h, format| Self::target(device, bgl, sampler, label, w, h, format);
        let frame = make("frame", ow, oh, FRAME_FORMAT);
        let backdrop = (0..BACKDROP_LEVELS)
            .map(|i| make("backdrop", ow >> (i + 1), oh >> (i + 1), FRAME_FORMAT))
            .collect();
        (slots, bloom, frame, backdrop, scene_size)
    }

    fn rebuild(&mut self, gpu: &Gpu) {
        let showing = self.preset();
        let (slots, bloom, frame, backdrop, scene_size) = Self::build_targets(
            &gpu.device,
            &self.tex_bgl,
            &self.sampler,
            self.out_size,
            self.render_scale,
            showing,
        );
        self.slots = slots;
        self.bloom = bloom;
        self.frame = frame;
        self.backdrop = backdrop;
        self.scene_size = scene_size;
        self.current = 0;
        self.transition = None;
        self.resend(showing);
    }

    /// The preset is about to be drawn into targets that have not seen it.
    fn resend(&mut self, preset: usize) {
        if let Some(driver) = &mut self.drivers[preset] {
            driver.resend();
        }
    }

    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        let size = (width.max(1), height.max(1));
        if size != self.out_size {
            self.out_size = size;
            self.rebuild(gpu);
        }
    }

    /// Change the internal resolution. The picture is carried over to the
    /// new targets, so a feedback preset keeps its trails and a dissolve
    /// carries on while the quality slider is dragged.
    pub fn set_render_scale(&mut self, gpu: &Gpu, scale: f32) {
        let scale = scale.clamp(0.25, 1.0);
        if scale == self.render_scale {
            return;
        }
        self.render_scale = scale;
        let (mut slots, bloom, scene_size) =
            Self::scene_targets(&gpu.device, &self.tex_bgl, &self.sampler, self.out_size, scale, 0);
        if scene_size == self.scene_size {
            return;
        }
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        for (i, (new, old)) in slots.iter_mut().zip(&self.slots).enumerate() {
            new.preset = old.preset;
            // Only a slot on screen has a picture worth keeping.
            let showing = i == self.current || self.transition.is_some();
            if showing && !old.needs_clear {
                // All but the rows the preset keeps as memory, which are
                // left as empty as the clear makes them.
                let (top, bottom) = PRESETS[old.preset].memory;
                let rows = scene_size.1.saturating_sub(top + bottom);
                let mut pass = begin(&mut encoder, &new.targets[new.front].view, true);
                if rows > 0 {
                    pass.set_scissor_rect(0, top, scene_size.0, rows);
                    pass.set_pipeline(&self.resample);
                    pass.set_bind_group(0, &old.targets[old.front].bind, &[]);
                    pass.draw(0..3, 0..1);
                }
                new.needs_clear = false;
            }
        }
        gpu.queue.submit([encoder.finish()]);
        for preset in [slots[0].preset, slots[1].preset] {
            self.resend(preset);
        }
        self.slots = slots;
        self.bloom = bloom;
        self.scene_size = scene_size;
    }

    pub fn set_palette(&self, gpu: &Gpu, palette: &Palette) {
        write_texture(&gpu.queue, &self.pal_tex, 4, &palette.texels());
    }

    /// The preset on screen, or the one being faded in.
    pub fn preset(&self) -> usize {
        let slot = if self.transition.is_some() {
            1 - self.current
        } else {
            self.current
        };
        self.slots[slot].preset
    }

    /// Replace a preset's shader with `body`, the text of a preset file. On
    /// a compile error the preset is left as it was.
    pub fn load_preset(&mut self, gpu: &Gpu, index: usize, body: &str) -> Result<(), String> {
        let (pipeline, show) = compile_preset(&gpu.device, &self.preset_layout, &PRESETS[index], body)?;
        self.preset_pipelines[index] = Some(pipeline);
        self.show_pipelines[index] = show;
        Ok(())
    }

    /// Let every driver take its own path from here on (see `Driver::shuffle`).
    pub fn shuffle_drivers(&mut self, seed: u64) {
        for (i, driver) in self.drivers.iter_mut().enumerate() {
            if let Some(driver) = driver {
                driver.shuffle(seed.wrapping_add(i as u64));
            }
        }
    }

    pub fn preset_available(&self, index: usize) -> bool {
        self.preset_pipelines.get(index).is_some_and(|p| p.is_some())
    }

    /// Switch presets, cross-dissolving unless `instant`.
    pub fn set_preset(&mut self, index: usize, instant: bool) {
        if index >= PRESETS.len() || index == self.preset() {
            return;
        }
        // If a transition is already running, land it first.
        if self.transition.take().is_some() {
            self.current = 1 - self.current;
        }
        let incoming = 1 - self.current;
        self.slots[incoming].preset = index;
        self.slots[incoming].needs_clear = true;
        self.resend(index);
        if instant {
            self.current = incoming;
        } else {
            self.transition = Some(0.0);
        }
    }

    /// Give a slot's preset this frame's numbers: the globals, and whatever
    /// its driver has to add.
    fn feed_slot(&mut self, gpu: &Gpu, analysis: &Analysis, dt: f32, globals: &mut Globals, index: usize) {
        globals.drive = [[0.0; 4]; PARAMS];
        if let Some(driver) = &mut self.drivers[self.slots[index].preset] {
            driver.update(analysis, dt);
            globals.drive = driver.params();
            if let Some(table) = driver.take_data() {
                write_texture(&gpu.queue, &self.data_tex[index], 16, bytemuck::cast_slice(table));
            }
        }
        gpu.queue
            .write_buffer(&self.globals_buf[index], 0, bytemuck::bytes_of(globals));
    }

    /// Give a slot the target its preset's picture goes to, if the preset
    /// keeps state, and take it away if it does not.
    fn fit_slot(&mut self, gpu: &Gpu, index: usize) {
        let stateful = PRESETS[self.slots[index].preset].stateful;
        if stateful && self.slots[index].shown.is_none() {
            let (w, h) = self.scene_size;
            self.slots[index].shown = Some(Self::target(
                &gpu.device,
                &self.tex_bgl,
                &self.sampler,
                "shown",
                w,
                h,
                SCENE_FORMAT,
            ));
        } else if !stateful {
            self.slots[index].shown = None;
        }
    }

    fn draw_slot(&mut self, encoder: &mut wgpu::CommandEncoder, index: usize) {
        let slot = &mut self.slots[index];
        if slot.needs_clear {
            // Feedback presets must not inherit another preset's last frame.
            for t in &slot.targets {
                drop(begin(encoder, &t.view, true));
            }
            slot.needs_clear = false;
        }
        let back = 1 - slot.front;
        {
            let mut pass = begin(encoder, &slot.targets[back].view, true);
            if let Some(pipeline) = &self.preset_pipelines[slot.preset] {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &self.globals_bind[index], &[]);
                pass.set_bind_group(1, &slot.targets[slot.front].bind, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        // A preset that keeps state: now make the picture from it.
        if let (Some(shown), Some(pipeline)) = (&slot.shown, &self.show_pipelines[slot.preset]) {
            let mut pass = begin(encoder, &shown.view, true);
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.globals_bind[index], &[]);
            pass.set_bind_group(1, &slot.targets[back].bind, &[]);
            pass.draw(0..3, 0..1);
        }
        slot.front = back;
    }

    fn post_bind(
        &self,
        gpu: &Gpu,
        a: &wgpu::TextureView,
        b: &wgpu::TextureView,
        bloom: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post"),
            layout: &self.post_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.post_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(a),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(b),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(bloom),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// Render one frame of the visualizer into `frame` and refresh the backdrop.
    pub fn render(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        analysis: &mut Analysis,
        time: f32,
        dt: f32,
    ) {
        let dt = dt.clamp(0.0005, 0.05);
        write_texture(&gpu.queue, &self.audio_tex, 1, &analysis.tex);
        if analysis.hist_dirty {
            write_texture(&gpu.queue, &self.hist_tex, 1, &analysis.hist);
            analysis.hist_dirty = false;
        }
        let (sw, sh) = self.scene_size;
        let [low, mid, high] = &analysis.onsets;
        // A pulse is as tall as the picture is asked to react.
        let react = analysis.reactivity.clamp(0.0, 1.0);
        let beat = analysis.beat.pulse * react;
        let mut globals = Globals {
            a: [sw as f32, sh as f32, time, dt],
            b: [analysis.bass, analysis.mid, analysis.treb, analysis.level],
            c: [beat, analysis.phase, analysis.beat.count, analysis.hist_head],
            d: [dt * 60.0, analysis.centroid, analysis.idle, analysis.energy],
            e: [
                low.pulse * react,
                mid.pulse * react,
                high.pulse * react,
                analysis.harmony,
            ],
            f: [low.count, mid.count, high.count, analysis.tonal],
            beat_ages: analysis.beat.ages,
            low_ages: low.ages,
            mid_ages: mid.ages,
            high_ages: high.ages,
            g: [react, analysis.bassline, analysis.melody, analysis.pitch],
            note_ages: analysis.note.ages,
            kick_hits: low.hits,
            snare_hits: mid.hits,
            hat_hits: high.hits,
            note_hits: analysis.note.hits,
            h: [analysis.note.pulse * react, analysis.note.count, 0.0, 0.0],
            drive: [[0.0; 4]; PARAMS],
        };

        let mut mix = 0.0;
        if let Some(t) = self.transition {
            let t = t + dt / TRANSITION_SECS;
            if t >= 1.0 {
                self.current = 1 - self.current;
                self.transition = None;
            } else {
                self.transition = Some(t);
                mix = smootherstep(t);
            }
        }
        let post = Post {
            a: [mix, 0.75, 0.5, self.glow],
            b: [beat, time, 0.005, 0.028],
            c: [0.55, 0.85, 1.0 / self.bloom.len() as f32, 0.0],
            d: [self.out_size.0 as f32, self.out_size.1 as f32, 0.0, 0.0],
        };
        gpu.queue.write_buffer(&self.post_buf, 0, bytemuck::bytes_of(&post));

        self.fit_slot(gpu, self.current);
        self.feed_slot(gpu, analysis, dt, &mut globals, self.current);
        self.draw_slot(encoder, self.current);
        if self.transition.is_some() {
            self.fit_slot(gpu, 1 - self.current);
            self.feed_slot(gpu, analysis, dt, &mut globals, 1 - self.current);
            self.draw_slot(encoder, 1 - self.current);
        }

        fn front(s: &Slot) -> &wgpu::TextureView {
            match &s.shown {
                Some(shown) => &shown.view,
                None => &s.targets[s.front].view,
            }
        }
        let a = front(&self.slots[self.current]);
        let b = if self.transition.is_some() {
            front(&self.slots[1 - self.current])
        } else {
            a
        };

        // Bloom: threshold + downsample, then accumulate back up the chain.
        {
            let bind = self.post_bind(gpu, a, b, a);
            let mut pass = begin(encoder, &self.bloom[0].view, true);
            pass.set_pipeline(&self.prefilter);
            pass.set_bind_group(0, &bind, &[]);
            pass.draw(0..3, 0..1);
        }
        for i in 1..self.bloom.len() {
            let mut pass = begin(encoder, &self.bloom[i].view, true);
            pass.set_pipeline(&self.bloom_down);
            pass.set_bind_group(0, &self.bloom[i - 1].bind, &[]);
            pass.draw(0..3, 0..1);
        }
        for i in (0..self.bloom.len() - 1).rev() {
            let mut pass = begin(encoder, &self.bloom[i].view, false);
            pass.set_pipeline(&self.bloom_up);
            pass.set_bind_group(0, &self.bloom[i + 1].bind, &[]);
            pass.draw(0..3, 0..1);
        }

        {
            let bind = self.post_bind(gpu, a, b, &self.bloom[0].view);
            let mut pass = begin(encoder, &self.frame.view, true);
            pass.set_pipeline(&self.composite);
            pass.set_bind_group(0, &bind, &[]);
            pass.draw(0..3, 0..1);
        }

        // Backdrop: blur the finished frame for the glass panels.
        for i in 0..self.backdrop.len() {
            let src = if i == 0 {
                &self.frame.bind
            } else {
                &self.backdrop[i - 1].bind
            };
            let mut pass = begin(encoder, &self.backdrop[i].view, true);
            pass.set_pipeline(&self.blur_down);
            pass.set_bind_group(0, src, &[]);
            pass.draw(0..3, 0..1);
        }
        for i in (1..self.backdrop.len() - 1).rev() {
            let mut pass = begin(encoder, &self.backdrop[i].view, true);
            pass.set_pipeline(&self.blur_up);
            pass.set_bind_group(0, &self.backdrop[i + 1].bind, &[]);
            pass.draw(0..3, 0..1);
        }
    }

    /// Draw the finished frame into the caller's (screen) render pass.
    pub fn blit(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.present);
        pass.set_bind_group(0, &self.frame.bind, &[]);
        pass.draw(0..3, 0..1);
    }

    /// The blurred copy of the frame, for frosted-glass panels.
    pub fn backdrop_view(&self) -> &wgpu::TextureView {
        &self.backdrop[1].view
    }

    /// The frame at half size, which blurs it only very slightly: what
    /// lightly frosted glass builds its haze from.
    pub fn backdrop_soft_view(&self) -> &wgpu::TextureView {
        &self.backdrop[0].view
    }

    /// The finished frame as it is, for clear glass to refract.
    pub fn frame_view(&self) -> &wgpu::TextureView {
        &self.frame.view
    }

    /// The finished frame, without any interface drawn over it.
    pub fn frame_texture(&self) -> &wgpu::Texture {
        &self.frame.texture
    }
}
