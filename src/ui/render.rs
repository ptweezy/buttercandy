//! Draws a frame's worth of [`Quad`]s in one instanced call.

use bytemuck::{Pod, Zeroable};

use super::Quad;
use crate::audio::player::ART_SIZE;
use crate::gfx::Gpu;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Screen {
    size: [f32; 2],
    srgb_out: f32,
    time: f32,
    mouse: [f32; 2],
    beat: f32,
    pad: f32,
}

/// What the interface is drawn over, and lit by, this frame.
pub struct Scene<'a> {
    pub time: f32,
    /// Where the pointer is, in pixels, if it is over the window. Glass
    /// catches more light near it.
    pub pointer: Option<(f32, f32)>,
    /// The beat pulse, 0..1: the highlights on glass flicker with it.
    pub beat: f32,
    pub atlas: &'a wgpu::TextureView,
    /// The finished visualizer frame for glass to show through: as it is,
    /// at half size (barely blurred), and heavily blurred.
    pub frame: &'a wgpu::TextureView,
    pub soft: &'a wgpu::TextureView,
    pub blurred: &'a wgpu::TextureView,
}

pub struct UiRenderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    screen: wgpu::Buffer,
    sampler: wgpu::Sampler,
    art: wgpu::Texture,
    art_view: wgpu::TextureView,
    instances: wgpu::Buffer,
    capacity: usize,
    count: u32,
    bind: Option<wgpu::BindGroup>,
    srgb_out: bool,
}

fn instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ui instances"),
        size: (capacity * size_of::<Quad>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

impl UiRenderer {
    pub fn new(gpu: &Gpu, out_format: wgpu::TextureFormat) -> Self {
        let device = &gpu.device;
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture(1),
                texture(2),
                texture(3),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture(5),
                texture(6),
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../gfx/shaders/ui.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![
            0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4,
            4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4,
        ];
        let premultiplied = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Quad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attributes,
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: out_format,
                    blend: Some(wgpu::BlendState {
                        color: premultiplied,
                        alpha: premultiplied,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let screen = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui screen"),
            size: size_of::<Screen>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let art = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cover art"),
            size: wgpu::Extent3d {
                width: ART_SIZE,
                height: ART_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let art_view = art.create_view(&Default::default());
        let capacity = 4096;
        Self {
            pipeline,
            layout,
            screen,
            sampler,
            art,
            art_view,
            instances: instance_buffer(device, capacity),
            capacity,
            count: 0,
            bind: None,
            srgb_out: out_format.is_srgb(),
        }
    }

    /// Replace the cover art (`ART_SIZE` square RGBA).
    pub fn set_art(&self, gpu: &Gpu, rgba: &[u8]) {
        if rgba.len() == (ART_SIZE * ART_SIZE * 4) as usize {
            crate::gfx::renderer::write_texture(&gpu.queue, &self.art, 4, rgba);
        }
    }

    /// Upload this frame's quads. `size` is the target size in pixels.
    pub fn prepare(&mut self, gpu: &Gpu, quads: &[Quad], size: (u32, u32), scene: &Scene) {
        if quads.len() > self.capacity {
            self.capacity = quads.len().next_power_of_two();
            self.instances = instance_buffer(&gpu.device, self.capacity);
        }
        gpu.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(quads));
        self.count = quads.len() as u32;
        let screen = Screen {
            size: [size.0 as f32, size.1 as f32],
            srgb_out: self.srgb_out as u32 as f32,
            time: scene.time,
            mouse: scene.pointer.map_or([-1.0; 2], |(x, y)| [x.max(0.0), y.max(0.0)]),
            beat: scene.beat,
            pad: 0.0,
        };
        gpu.queue.write_buffer(&self.screen, 0, bytemuck::bytes_of(&screen));
        // The backdrop views change whenever the window is resized, so the
        // bind group is simply rebuilt each frame.
        self.bind = Some(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.screen.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(scene.atlas),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(scene.blurred),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.art_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(scene.frame),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(scene.soft),
                },
            ],
        }));
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(bind) = &self.bind else { return };
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..4, 0..self.count);
    }
}
