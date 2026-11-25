//! AI Generated video primitive code
//!
//! LAYER 2: `Primitive` — a fresh, plain-data snapshot built every frame.
//! Responsibility: carry *this frame's* bytes from `Program` to `Pipeline`.
//! No persistent state and no caching belongs here.

use iced::wgpu;
use iced::widget::shader::{self, Viewport};
use iced::Rectangle;

use super::VideoPipeline;

#[derive(Debug)]
pub enum VideoPrimitive {
    Empty,
    /// No new data since the last frame; skip the CPU copy/GPU upload, just
    /// re-run the (cheap) scale computation in case `bounds` changed.
    Unchanged { letterbox: bool },
    Frame {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        letterbox: bool,
    },
}

impl shader::Primitive for VideoPrimitive {
    type Pipeline = VideoPipeline;

    /// Runs during the renderer's `prepare()` pass, before any render pass is
    /// recorded. This is the only place with `&Device`/`&Queue` access, so the
    /// GPU texture upload must happen here.
    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        _viewport: &Viewport,
    ) {
        match self {
            VideoPrimitive::Frame {
                width,
                height,
                rgba,
                letterbox,
            } => pipeline.upload(device, queue, *width, *height, rgba, *bounds, *letterbox),
            VideoPrimitive::Unchanged { letterbox } => pipeline.refresh_scale(queue, *bounds, *letterbox),
            VideoPrimitive::Empty => pipeline.show_placeholder(queue),
        }
    }

    /// Records the draw commands into the given encoder/target. Keep this
    /// dumb; all GPU resource management lives in `Pipeline`.
    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        // Always draw: the pipeline keeps a placeholder texture bound even
        // when no frame has arrived yet, so the widget is never left blank.
        pipeline.draw(encoder, target, *clip_bounds);
    }
}
