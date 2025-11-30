use std::collections::HashMap;

use futures::FutureExt;

use crate::util::PinnedLocalFuture;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VirtualScreenPixelFormat {
    Rgb888,
    Bgr888,
    Rgba8888,
    Bgra8888,
    Argb8888,
    Abgr8888,
}

#[derive(Debug, Clone)]
pub struct ScreenOutputParameters {
    pub format: VirtualScreenPixelFormat,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub meta_data: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct EncoderParameters {
    pub width: u32,
    pub height: u32,
    pub bitrate: u32,
    pub fps: u32,
    pub input_parameters: ScreenOutputParameters,
}

pub trait Encoder {
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
