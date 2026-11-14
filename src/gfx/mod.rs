//! GPU plumbing shared by the visualizer and the interface.

pub mod driver;
mod jawbreaker;
pub mod presets;
pub mod renderer;
#[cfg(test)]
mod sugarplum;

use std::sync::Arc;

use winit::window::Window;

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

fn request_device(adapter: &wgpu::Adapter) -> Result<Gpu, String> {
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("buttercandy"),
        required_features: wgpu::Features::empty(),
        // Nothing here needs more than the baseline WebGPU limits.
        required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
        ..Default::default()
    }))
    .map_err(|e| format!("could not open the GPU device: {e}"))?;
    let info = adapter.get_info();
    log::info!("gpu: {} ({:?})", info.name, info.backend);
    Ok(Gpu { device, queue })
}

/// Instance options: the defaults and whatever the environment asks for
/// (`WGPU_BACKEND=gl`, say), minus validation of indirect draws. We make
/// none, and it needs compute shaders, which an older OpenGL does not have.
fn instance(mut options: wgpu::InstanceDescriptor) -> wgpu::Instance {
    options.flags.remove(wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL);
    wgpu::Instance::new(options)
}

impl Gpu {
    /// A device plus a surface for `window`. The OpenGL backend needs the
    /// `display` the window lives on (on Wayland, at least); the others
    /// ignore it.
    pub fn for_window(
        window: Arc<Window>,
        display: winit::event_loop::OwnedDisplayHandle,
    ) -> Result<(Gpu, wgpu::Surface<'static>, wgpu::Adapter), String> {
        let instance = instance(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(
            display,
        )));
        let surface = instance
            .create_surface(window)
            .map_err(|e| format!("could not create a surface: {e}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| format!("no compatible GPU found: {e}"))?;
        Ok((request_device(&adapter)?, surface, adapter))
    }

    /// A device with no window, for offscreen rendering.
    pub fn headless() -> Result<Gpu, String> {
        let instance = instance(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU found: {e}"))?;
        request_device(&adapter)
    }

    /// Read an RGBA8/BGRA8 texture back as tightly packed RGBA bytes.
    pub fn read_texture(&self, texture: &wgpu::Texture) -> Result<Vec<u8>, String> {
        let (w, h) = (texture.width(), texture.height());
        let bgra = matches!(
            texture.format(),
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        // Rows in a copy must be padded to a multiple of 256 bytes.
        let stride = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (stride * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);

        let (tx, rx) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;

        let data = buffer.get_mapped_range(..).map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for row in data.chunks_exact(stride as usize) {
            for px in row[..(w * 4) as usize].as_chunks::<4>().0 {
                if bgra {
                    out.extend_from_slice(&[px[2], px[1], px[0], 255]);
                } else {
                    out.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
            }
        }
        Ok(out)
    }
}

/// Pick a surface format. We do our own gamma encoding, so prefer a format
/// that stores what we write verbatim.
pub fn pick_surface_format(caps: &wgpu::SurfaceCapabilities) -> wgpu::TextureFormat {
    use wgpu::TextureFormat::*;
    for want in [Bgra8Unorm, Rgba8Unorm] {
        if caps.formats.contains(&want) {
            return want;
        }
    }
    caps.formats
        .iter()
        .copied()
        .find(|f| !f.is_srgb())
        .unwrap_or(caps.formats[0])
}
