use crate::{
    coding::encoder::EncoderPossibleCodec, host::VirtualScreenPixelFormat, util::PinnedLocalFuture,
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
pub enum DecoderAction<'a> {
    /// The decoder has decoded the data and performed the necessary painting action.
    DirectPaint,
    /// The decoder has decoded the data but has not performed any painting action.
    Decode(&'a [u8]),
}

pub trait Decoder {
    type Error: std::error::Error + Send;

    /// Get the encodings that this client prefers, from the list of available configurations
    /// that the backend offered.
    fn get_preferred_configurations(
        &mut self,
        possible_configurations: Vec<&EncoderPossibleCodec>,
    ) -> PinnedLocalFuture<'_, Vec<&EncoderPossibleCodec>>;

    /// After the backend selects an encoding, this function is called to set the chosen codec for the decoder.
    fn set_codec(
        &mut self,
        codec: &EncoderPossibleCodec,
    ) -> PinnedLocalFuture<'_, Result<(), Self::Error>>;

    /// Perform the decoding action.
    fn decode<'s, 'e, 'd>(
        &'s mut self,
        encoded_data: &'e [u8],
    ) -> PinnedLocalFuture<'_, Result<DecoderAction<'d>, Self::Error>>;
}
