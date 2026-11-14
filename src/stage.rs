//! Everything that turns sound into pixels, independent of where the sound
//! comes from or where the pixels go: analysis, the visualizer, and the UI.
//! Shared by the windowed app and the headless screenshot mode.

use crate::audio::analysis::Analysis;
use crate::gfx::Gpu;
use crate::gfx::renderer::{self, Renderer};
use crate::palette::PALETTES;
use crate::ui::Ui;
use crate::ui::render::{Scene, UiRenderer};
use crate::ui::view::View;

pub struct Stage {
    pub analysis: Analysis,
    pub renderer: Renderer,
    pub ui: Ui,
    pub ui_renderer: UiRenderer,
    pub view: View,
    /// Seconds of animation time elapsed.
    pub time: f32,
}

impl Stage {
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat, size: (u32, u32), preset: usize, palette: usize) -> Self {
        Self {
            analysis: Analysis::new(),
            renderer: Renderer::new(gpu, format, size.0, size.1, preset, &PALETTES[palette]),
            ui: Ui::new(&gpu.device, &gpu.queue),
            ui_renderer: UiRenderer::new(gpu, format),
            view: View::new(),
            time: 0.0,
        }
    }

    /// Draw the visualizer, then the interface built this frame, into `target`.
    pub fn render(&mut self, gpu: &Gpu, target: &wgpu::TextureView, size: (u32, u32), dt: f32, with_ui: bool) {
        self.time += dt;
        self.ui.finish();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        self.renderer
            .render(gpu, &mut encoder, &mut self.analysis, self.time, dt);
        let quads = if with_ui { self.ui.quads.as_slice() } else { &[] };
        let (input, scale) = (&self.ui.input, self.ui.scale);
        let scene = Scene {
            time: self.time,
            pointer: input.inside.then_some((input.mouse.0 * scale, input.mouse.1 * scale)),
            beat: self.analysis.beat.pulse,
            atlas: &self.ui.atlas.view,
            frame: self.renderer.frame_view(),
            soft: self.renderer.backdrop_soft_view(),
            blurred: self.renderer.backdrop_view(),
        };
        self.ui_renderer.prepare(gpu, quads, size, &scene);
        {
            let mut pass = renderer::begin(&mut encoder, target, true);
            self.renderer.blit(&mut pass);
            self.ui_renderer.draw(&mut pass);
        }
        gpu.queue.submit([encoder.finish()]);
    }
}
