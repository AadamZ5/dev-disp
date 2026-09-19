use std::collections::HashMap;

use futures::FutureExt;
use serde::{Deserialize, Serialize};

use crate::{
    coding::codecs::{Codec, CodecFamily, RawParameters},
    host::ScreenOutputParameters,
    util::{PinnedFuture, PinnedLocalFuture},
};

/// Represents the content parameters we will be encoding at. Used to prepare encoders and request what encodings
/// will be supported with these params.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderContentParameters {
    /// TODO: Will this ever differ from [encoder_input_parameters.width]
    pub width: u32,
    /// TODO: Will this ever differ from [encoder_input_parameters.height]
    pub height: u32,
    pub bitrate: u32,
    pub fps: u32,
    /// The parameters of the virtual screen generated output data, that will
    /// be sent to the encoder.
    pub encoder_input_parameters: ScreenOutputParameters,
}

/// A possible configuration for an encoder, including its name, supported resolution. Useful to send
/// to your client implementation.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EncoderPossibleCodec {
    /// Used during negotiation. This ID helps identify the specific encoder configuration.
    /// If not needed, just set to default or 0.
    pub id: u32,
    /// The name of the encoder, e.g., "h264_nvenc"
    pub display_name: String,
    /// A codec with configured parameters
    pub codec: Codec,
    /// What resolution the encoder is configured for
    pub encoded_resolution: (u32, u32),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EncoderPossibleCodecInternal<T> {
    pub display_name: String,
    pub codec: Codec,
    pub encoded_resolution: (u32, u32),
    pub data: T,
}

impl<T> From<EncoderPossibleCodecInternal<T>> for EncoderPossibleCodec {
    fn from(internal: EncoderPossibleCodecInternal<T>) -> Self {
        EncoderPossibleCodec {
            id: 0,
            display_name: internal.display_name,
            codec: internal.codec,
            encoded_resolution: internal.encoded_resolution,
        }
    }
}

impl<T> EncoderPossibleCodecInternal<T> {
    /// Clone my fields, and create me without the type data associated
    pub fn for_send(&self, id: u32) -> EncoderPossibleCodec {
        EncoderPossibleCodec {
            id,
            display_name: self.display_name.clone(),
            codec: self.codec.clone(),
            encoded_resolution: self.encoded_resolution.clone(),
        }
    }
}

/// Simple type to wrap info about the successfully set encoding
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EncoderCodecResult {
    pub display_name: String,
    pub codec: Codec,
    pub encoded_resolution: (u32, u32),
}

/// Given a collection of internal encoder configurations, this function maps them to their external representations.
/// Returns a tuple containing:
/// 1. A vector of ID'ed external encoder configurations (`EncoderPossibleCodec`).
/// 2. A hashmap mapping the unique ID to the corresponding internal configuration (`EncoderPossibleCodecInternal<D>`).
pub fn map_internal_to_external_configs<T, D>(
    internal_configs: T,
) -> (
    Vec<EncoderPossibleCodec>,
    HashMap<u32, EncoderPossibleCodecInternal<D>>,
)
where
    T: IntoIterator<Item = EncoderPossibleCodecInternal<D>>,
{
    let fold_state = (Vec::new(), HashMap::new());

    // Create a unique ID for each configuration, and create a mapping from the ID to the internal configuration
    internal_configs.into_iter().enumerate().fold(
        fold_state,
        |(mut vec, mut map), (index, config)| {
            let external = config.for_send(index as u32);
            map.insert(index as u32, config);
            vec.push(external);
            (vec, map)
        },
    )
}

pub fn map_external_to_internal_configs<T, D>(
    external_configs: T,
    internal_map: &HashMap<u32, EncoderPossibleCodecInternal<D>>,
) -> impl Iterator<Item = &EncoderPossibleCodecInternal<D>>
where
    T: IntoIterator<Item = EncoderPossibleCodec>,
{
    external_configs
        .into_iter()
        .filter_map(|external| internal_map.get(&external.id))
}

/// **Deprecated**: This trait may be removed in future versions.
/// Encoding is a responsibility of the transport, not a separate component.
pub trait Encoder {
    /// The type of the implementation-specific data associated with the encoder.
    type CodecData;

    /// Implementation-specific code to understand what configurations this local machine supports.
    /// Takes in [EncoderContentParameters] that contain values pertaining to the created virtual screen,
    /// so you can determine what encoders are compatible with that. Take your time!
    ///
    /// TODO: Better error type
    /// TODO: This should live as a function of the encoder provider maybe
    fn get_supported_configurations(
        &mut self,
        parameters: &EncoderContentParameters,
    ) -> PinnedLocalFuture<'_, Result<Vec<EncoderPossibleCodecInternal<Self::CodecData>>, String>>;

    /// Called first, to initialize the encoder with the given parameters.
    /// Must return the successfully initialized encoder configuration.
    ///
    /// The codecs should be tried in order they are supplied (if at all).
    ///
    /// Arguments:
    ///  - `parameters`: The content parameters for the encoder.
    ///  - `preferred_encoders`: A list of the client-preferred encoder configurations
    ///  - `offered_encoders`: A list of the encoder configurations returned by your [Self::get_supported_configurations] implementation.
    ///
    /// TODO: Better error type
    /// TODO: This should also potentially live as a function of the encoder provider.
    fn set_codec<'s, 'p>(
        &'s mut self,
        parameters: &'p EncoderContentParameters,
        preferred_encoders: Option<Vec<&'p EncoderPossibleCodecInternal<Self::CodecData>>>,
        offered_encoders: Vec<&'p EncoderPossibleCodecInternal<Self::CodecData>>,
    ) -> PinnedLocalFuture<'s, Result<&'p EncoderPossibleCodecInternal<Self::CodecData>, String>>
    where
        'p: 's;

    /// Encodes a frame of raw data, returning the encoded data.
    /// TODO: Better error type
    /// TODO: Consider changing the future to be non-boxed if possible for performance
    fn encode<'s, 'a>(
        &'s mut self,
        raw_data: &'a [u8],
    ) -> PinnedLocalFuture<'s, Result<&'s [u8], String>>
    where
        'a: 's;
}

pub trait EncoderProvider {
    type EncoderType: Encoder + 'static;

    fn init(&mut self) -> PinnedLocalFuture<'_, Result<(), String>> {
        async move { Ok(()) }.boxed_local()
    }

    // TODO: Better error type, async!
    fn create_encoder(&self) -> PinnedLocalFuture<'_, Result<Self::EncoderType, String>>;
}

pub struct RawEncoder;

impl Encoder for RawEncoder {
    type CodecData = ();

    fn get_supported_configurations(
        &mut self,
        screen_parameters: &EncoderContentParameters,
    ) -> PinnedLocalFuture<'_, Result<Vec<EncoderPossibleCodecInternal<Self::CodecData>>, String>>
    {
        let width = screen_parameters.encoder_input_parameters.width;
        let height = screen_parameters.encoder_input_parameters.height;
        let stride = screen_parameters.encoder_input_parameters.stride;
        let pixel_format = screen_parameters.encoder_input_parameters.format;
        async move {
            Ok(vec![EncoderPossibleCodecInternal::<Self::CodecData> {
                display_name: "raw".to_string(),
                encoded_resolution: (width, height),
                codec: Codec::Raw(RawParameters {
                    height,
                    width,
                    stride,
                    pixel_format,
                }),
                data: (),
            }])
        }
        .boxed_local()
    }

    fn set_codec<'s, 'p>(
        &'s mut self,
        _screen_parameters: &'p EncoderContentParameters,
        _preferred_encoders: Option<Vec<&'p EncoderPossibleCodecInternal<Self::CodecData>>>,
        offered_encoders: Vec<&'p EncoderPossibleCodecInternal<Self::CodecData>>,
    ) -> PinnedLocalFuture<'s, Result<&'p EncoderPossibleCodecInternal<Self::CodecData>, String>>
    where
        'p: 's,
    {
        async move {
            // For raw encoder, we should assert that our offered encoders contains the single raw
            // encoding option. We assume that here.
            offered_encoders
                .iter()
                .find(|enc| enc.codec.family() == CodecFamily::Raw)
                .map(|enc| *enc)
                .ok_or("Raw encoder not found in offered codecs!".to_string())
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
