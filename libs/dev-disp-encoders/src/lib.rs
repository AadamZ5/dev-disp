//! This crate defines codec implementations for the dev-disp poject.
//!
//! Encoders here can be used within the framework to handle packing or unpacking visual data.

pub mod toolkit;

#[cfg(feature = "ffmpeg")]
pub mod ffmpeg;
pub(crate) mod util;
