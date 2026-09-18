//! A module providing an ffmpeg codec implementation for the dev-disp project.

mod ffmpeg_encoder;

pub mod config_file;
pub mod configurations;
pub use ffmpeg_encoder::*;
