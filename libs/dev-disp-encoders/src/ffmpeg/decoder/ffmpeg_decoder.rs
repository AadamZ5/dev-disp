use dev_disp_core::util::PinnedLocalFuture;
use ffmpeg_next::{
    Dictionary, Frame, Packet,
    decoder::{Decoder, Video},
};
use futures::FutureExt;
use thiserror::Error;

use crate::{
    ffmpeg::decoder::{
        config_file::FfmpegDecoderConfiguration, configuration::FfmpegDecoderCodecConfiguration,
    },
    toolkit::{
        decoder::{Decoder as DevDispDecoder, DecoderAction, DecoderDirectAction},
        encoder::CodecOption,
    },
};

pub struct FfmpegDecoder {
    state: Option<FfmpegDecoderState>,
    configuration: FfmpegDecoderConfiguration,
}

impl FfmpegDecoder {
    pub fn new(configuration: FfmpegDecoderConfiguration) -> Self {
        Self {
            state: None,
            configuration,
        }
    }
}

pub struct FfmpegDecoderState {
    video: Video,
}

#[derive(Debug, Error)]
pub enum FfmpegDecoderError {
    #[error("Decoder not found: {0}")]
    DecoderNotFound(String),
    #[error("Failed to initialize the FFmpeg decoder")]
    DecoderInitializationFailed,
    #[error("Decoder not initialized before it was used")]
    DecoderNotInitialized,
    #[error("Failed to decode a packet")]
    DecodingFailed,
}

impl DevDispDecoder for FfmpegDecoder {
    type Error = FfmpegDecoderError;

    fn get_preferred_configurations<'s, 'd>(
        &'s mut self,
        possible_configurations: Vec<&'d CodecOption>,
    ) -> PinnedLocalFuture<'s, Vec<&'d CodecOption>>
    where
        'd: 's,
    {
        let configuration = self.configuration.clone();
        async move {
            // Filter the codecs based on what codecs we have configured
            let preferred_configurations: Vec<_> = possible_configurations
                .into_iter()
                .filter(|config| {
                    configuration
                        .codec_choices
                        .iter()
                        .find(|ours| ours.codec_family == config.codec.family())
                        .is_some_and(|ours| initialize_ffmpeg_decoder(ours).is_ok())
                })
                .collect();

            preferred_configurations
        }
        .boxed_local()
    }

    fn set_codec<'s>(
        &'s mut self,
        option: &'s CodecOption,
    ) -> PinnedLocalFuture<'s, Result<(), Self::Error>> {
        async move {
            let mut matching_configs = self
                .configuration
                .codec_choices
                .iter()
                .filter(|ours| ours.codec_family == option.codec.family());

            let video = loop {
                if let Some(config) = matching_configs.next() {
                    match initialize_ffmpeg_decoder(config) {
                        Ok(video) => break video,
                        Err(_) => continue,
                    }
                } else {
                    return Err(FfmpegDecoderError::DecoderInitializationFailed);
                }
            };

            self.state = Some(FfmpegDecoderState { video });

            Ok(())
        }
        .boxed_local()
    }

    fn decode<'s, 'e>(
        &'s mut self,
        encoded_data: &'e [u8],
    ) -> PinnedLocalFuture<'s, DecoderAction<Self::Error>>
    where
        'e: 's,
    {
        async move {
            let mut buf = Vec::new();
            match self.decode_into(encoded_data, &mut buf).await {
                DecoderDirectAction::Decoded => DecoderAction::Decode(buf),
                DecoderDirectAction::NoFrame => DecoderAction::NoFrame,
                DecoderDirectAction::Error(e) => DecoderAction::Error(e),
            }
        }
        .boxed_local()
    }

    fn decode_into<'s, 'e>(
        &'s mut self,
        encoded_data: &'e [u8],
        output_buffer: &'s mut [u8],
    ) -> PinnedLocalFuture<'s, DecoderDirectAction<Self::Error>>
    where
        'e: 's,
    {
        async move {
            let state = match self
                .state
                .as_mut()
                .ok_or(FfmpegDecoderError::DecoderNotInitialized)
            {
                Ok(state) => state,
                Err(e) => return DecoderDirectAction::Error(e),
            };

            let packet = Packet::copy(encoded_data);
            match state
                .video
                .send_packet(&packet)
                .map_err(|_| FfmpegDecoderError::DecodingFailed)
            {
                Ok(_) => {}
                Err(e) => return DecoderDirectAction::Error(e),
            }

            let mut frame = unsafe { Frame::empty() };

            let mut decoded_a_frame = false;
            loop {
                match state.video.receive_frame(&mut frame) {
                    Ok(_) => {
                        decoded_a_frame = true;
                    }
                    Err(e) => match e {
                        ffmpeg_next::util::error::Error::Other { errno } => {
                            if errno == ffmpeg_next::error::EAGAIN {
                                if decoded_a_frame == false {
                                    return DecoderDirectAction::NoFrame;
                                } else {
                                    return DecoderDirectAction::Decoded;
                                }
                            } else {
                                return DecoderDirectAction::Error(
                                    FfmpegDecoderError::DecodingFailed,
                                );
                            }
                        }
                        _other => {
                            return DecoderDirectAction::Error(FfmpegDecoderError::DecodingFailed);
                        }
                    },
                }
            }
        }
        .boxed_local()
    }
}

fn initialize_ffmpeg_decoder(
    decoder_config: &FfmpegDecoderCodecConfiguration,
) -> Result<Video, FfmpegDecoderError> {
    let codec = ffmpeg_next::decoder::find_by_name(&decoder_config.codec_name)
        .ok_or_else(|| FfmpegDecoderError::DecoderNotFound(decoder_config.codec_name.clone()))?;

    let params = Dictionary::from_iter(decoder_config.decoder_options.clone().into_iter());

    let decoder = ffmpeg_next::codec::Context::new()
        .decoder()
        .open_as_with(codec, params)
        .map_err(|_| FfmpegDecoderError::DecoderInitializationFailed)?;

    let video = decoder
        .video()
        .map_err(|_| FfmpegDecoderError::DecoderInitializationFailed)?;

    Ok(video)
}
