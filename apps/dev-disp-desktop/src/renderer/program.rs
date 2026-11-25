//! AI Generated video program code
//!
//! LAYER 1: `Program` — lives in the widget tree, rebuilt every `view()` call.
//! Responsibility: decide *what* to draw this frame. No GPU access here.

use iced::widget::shader;
use iced::{mouse, Rectangle};

use crate::worker::WorkerRenderContext;

use super::VideoPrimitive;

pub enum VideoData<'a> {
    Frame {
        width: u32,
        height: u32,
        buf: &'a [u8],
    },
    NoUpdate,
    None,
}

/// Construct this fresh in `view()`, e.g.:
/// `shader(VideoFeed::new(self.worker_render_context.as_ref()))`
pub struct VideoFeed<'a> {
    render_context: VideoData<'a>,
    letterbox: bool,
}

impl<'a> VideoFeed<'a> {
    pub fn new(render_context: VideoData<'a>) -> Self {
        Self {
            render_context,
            letterbox: true,
        }
    }

    /// Toggles aspect-ratio-preserving letterbox/pillarbox bars. When `false`,
    /// the frame is stretched to fill the widget's bounds.
    pub fn letterbox(mut self, enabled: bool) -> Self {
        self.letterbox = enabled;
        self
    }
}

impl<Message> shader::Program<Message> for VideoFeed<'_> {
    /// No widget-local state needed; all real state lives in `ScreenHostApp`.
    type State = ();
    type Primitive = VideoPrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        match &self.render_context {
            VideoData::Frame { width, height, buf } => VideoPrimitive::Frame {
                width: *width,
                height: *height,
                letterbox: self.letterbox,
                rgba: buf[..(width * height * 4) as usize].to_vec(),
            },
            VideoData::NoUpdate => {
                return VideoPrimitive::Unchanged {
                    letterbox: self.letterbox,
                }
            }
            VideoData::None => return VideoPrimitive::Empty,
        }
    }
}
