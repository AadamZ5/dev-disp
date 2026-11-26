//! Custom `iced::widget::shader` program for painting decoded video frames
//! directly to a persistent GPU texture, bypassing `widget::image`'s
//! per-frame `Handle` churn.
//!
//! - `program`: what to draw this frame (reads the worker's triple buffer).
//! - `primitive`: per-frame data snapshot handed to the renderer.
//! - `pipeline`: persistent wgpu resources (texture, sampler, render pipeline).

mod pipeline;
mod primitive;
mod program;

pub use pipeline::VideoPipeline;
pub use primitive::VideoPrimitive;
pub use program::{VideoData, VideoFeed};
