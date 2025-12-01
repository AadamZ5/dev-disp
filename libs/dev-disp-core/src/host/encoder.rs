use std::collections::HashMap;

use futures::FutureExt;
use serde::{Deserialize, Serialize};

use crate::util::PinnedLocalFuture;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VirtualScreenPixelFormat {
    Rgb888,
    Bgr888,
    Rgba8888,
    Bgra8888,
    Argb8888,
    Abgr8888,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenOutputParameters {
    /// Our intermediate pixel format representation.
    pub format: VirtualScreenPixelFormat,

    /// Width in pixels
    pub width: u32,

    /// Height in pixels
    pub height: u32,

    /// Stride in bytes
    ///
    /// This is the amount of bytes between the start of one row of pixels and the start of the next row.
    /// This may be more than width * bytes_per_pixel due to padding or alignment requirements.
    ///
    /// In this example, notice the padding at the end of each row to align to the stride:
    /// ```text
    /// [ <----- ... Stride bytes ... -----> ]
    ///
    /// [P1][P2][P3][P4]...[Pn][PAD][PAD][PAD]  <- Row 1
    /// [P1][P2][P3][P4]...[Pn][PAD][PAD][PAD]  <- Row 2
    /// ...
    /// [P1][P2][P3][P4]...[Pn][PAD][PAD][PAD]  <- Row h
    ///
    /// [ <-- Pixel Data ---> ][ < Padding > ]
    /// ```
    pub stride: u32,

    /// Any additional meta data associated with the screen output
    pub meta_data: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderParameters {
    pub width: u32,
    pub height: u32,
    pub bitrate: u32,
    pub fps: u32,
    pub input_parameters: ScreenOutputParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderPossibleConfiguration {
    /// The name of the encoder, e.g., "h264_nvenc"
    pub encoder_name: String,
    /// Something like "hevc", "h264", "vp8", etc.
    /// Or "raw" for raw uncompressed output.
    pub encoder_family: String,
    /// Key-value pairs of encoder parameters and their values.
    pub parameters: HashMap<String, String>,
}

pub trait Encoder {
    fn get_supported_configurations(
        &mut self,
        parameters: &EncoderParameters,
    ) -> Result<Vec<EncoderPossibleConfiguration>, String>;

    /// Called first, to initialize the encoder with the given parameters.
    /// TODO: Better error type
    fn init(&mut self, parameters: EncoderParameters) -> PinnedLocalFuture<'_, Result<(), String>>;

    /// Encodes a frame of raw data, returning the encoded data.
    /// TODO: Better error type
    fn encode<'s, 'a>(
        &'s mut self,
        raw_data: &'a [u8],
    ) -> PinnedLocalFuture<'s, Result<&'s [u8], String>>
    where
        'a: 's;
}

pub trait EncoderProvider {
    type EncoderType: Encoder + 'static;

    // TODO: Implement negotiation protocol here!

    // TODO: Better error type
    fn create_encoder(&self) -> Result<Self::EncoderType, String>;
}

pub struct RawEncoder;

impl Encoder for RawEncoder {
    fn get_supported_configurations(
        &mut self,
        _parameters: &EncoderParameters,
    ) -> Result<Vec<EncoderPossibleConfiguration>, String> {
        Ok(vec![EncoderPossibleConfiguration {
            encoder_name: "raw".to_string(),
            encoder_family: "raw".to_string(),
            parameters: HashMap::new(),
        }])
    }

    fn init(
        &mut self,
        _parameters: EncoderParameters,
    ) -> PinnedLocalFuture<'_, Result<(), String>> {
        async move {
            // No initialization needed for raw encoder
            Ok(())
        }
        .boxed_local()
    }

    fn encode<'s, 'a>(
        &'s mut self,
        raw_data: &'a [u8],
    ) -> PinnedLocalFuture<'s, Result<&'s [u8], String>>
    where
        'a: 's,
    {
        async move {
            // For raw encoder, just return the input data as is
            Ok(raw_data)
        }
        .boxed_local()
    }
}
