use crate::toolkit::encoder::CodecOption;
use dev_disp_core::{
    host::{ScreenContentParameters, VirtualScreenPixelFormat},
    util::PinnedLocalFuture,
};

pub struct DecodedFrame {
    pub data: Vec<u8>,
    pub pixel_format: VirtualScreenPixelFormat,
}

/// Decoders may hand off to a decoding engine that performs the decoding **and** also
/// the painting of the data, meaning we may not need to manage the data after it is
/// decoded.
///
/// These are the possible actions a decoder can take.
pub enum DecoderAction<E> {
    /// The decoder has decoded the data and performed the necessary painting action.
    DirectPaint,
    /// The decoder has decoded the data but has not performed any painting action.
    Decode(Vec<u8>),
    /// The data was ingested, but no frame was produced.
    NoFrame,
    /// An error occurred during decoding.
    Error(E),
}

pub enum DecoderDirectAction<E> {
    /// Frame(s) were produced,
    Decoded,

    /// The data was ingested, but no frame was produced.
    NoFrame,

    /// An error occurred during decoding.
    Error(E),
}

pub trait Decoder {
    type Error: std::error::Error + Send;

    /// Get the encodings that this client prefers, from the list of available configurations
    /// that the backend offered.
    fn get_preferred_configurations<'s, 'd>(
        &'s mut self,
        screen_parameters: &'d ScreenContentParameters,
        possible_configurations: Vec<&'d CodecOption>,
    ) -> PinnedLocalFuture<'s, Vec<&'d CodecOption>>
    where
        'd: 's;

    /// After the backend selects an encoding, this function is called to set the chosen codec for the decoder.
    fn set_codec<'s>(
        &'s mut self,
        screen_parameters: &'s ScreenContentParameters,
        codec: &'s CodecOption,
    ) -> PinnedLocalFuture<'s, Result<(), Self::Error>>;

    /// Perform the decoding action.
    fn decode<'s, 'e>(
        &'s mut self,
        encoded_data: &'e [u8],
    ) -> PinnedLocalFuture<'s, DecoderAction<Self::Error>>
    where
        'e: 's;

    /// Decodes into the specified buffer instead of allocating a new one.
    fn decode_into<'s, 'e>(
        &'s mut self,
        encoded_data: &'e [u8],
        output_buffer: &'s mut [u8],
    ) -> PinnedLocalFuture<'s, DecoderDirectAction<Self::Error>>
    where
        'e: 's;
}
