//! This module holds structures that record configuration data for decoders in FFmpeg.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::toolkit::codecs::CodecFamily;

/// A particular FFmpeg decoder configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FfmpegDecoderCodecConfiguration {
    /// An exact FFmpeg decoder name, e.g., "libx264".
    pub codec_name: String,
    pub codec_family: CodecFamily,
    /// Encoder options for ffmpeg, these are not the internal codec parameters.
    pub decoder_options: HashMap<String, String>,
}
