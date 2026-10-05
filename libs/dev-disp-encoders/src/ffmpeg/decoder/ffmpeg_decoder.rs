use std::io::Write;

use dev_disp_core::{host::ScreenContentParameters, util::PinnedLocalFuture};
use ffmpeg_next::{
    Dictionary, Packet, decoder::Video, picture::Type::P,
    software::scaling::Context as ScalingContext,
};
use futures::FutureExt;
use log::{debug, error};
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

pub type FrameFormatParams = (u32, u32, ffmpeg_next::format::Pixel);

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
    scaler: Option<ScalingContext>,
    frame_params: Option<FrameFormatParams>,
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
        screen_parameters: &'d ScreenContentParameters,
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
        _screen_parameters: &'s ScreenContentParameters,
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

            self.state = Some(FfmpegDecoderState {
                video,
                scaler: None,
                frame_params: None,
            });

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
            // TODO: Size this buffer appropriately!
            let mut buf = vec![0; 4000 * 4000 * 4];
            match self.decode_into(encoded_data, &mut buf).await {
                DecoderDirectAction::Decoded => DecoderAction::Decode(buf),
                DecoderDirectAction::NoFrame => DecoderAction::NoFrame,
                DecoderDirectAction::Error(e) => DecoderAction::Error(e),
            }
        }
        .boxed_local()
    }

    /// TODO Give me a &Vec<u8> instead of a &mut [u8] so we can grow the buffer?
    fn decode_into<'s, 'e>(
        &'s mut self,
        encoded_data: &'e [u8],
        mut output_buffer: &'s mut [u8],
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

            let mut received_frame: Option<ffmpeg_next::frame::Video> = None;

            // Try to drain any frames. We only want the latest one tbh
            loop {
                let mut candidate_frame = ffmpeg_next::frame::Video::empty();
                match state.video.receive_frame(&mut candidate_frame) {
                    Ok(_) => {
                        received_frame = Some(candidate_frame);
                    }
                    Err(e) => match e {
                        ffmpeg_next::util::error::Error::Other { errno } => {
                            if errno == ffmpeg_next::error::EAGAIN {
                                if received_frame.is_none() {
                                    return DecoderDirectAction::NoFrame;
                                } else {
                                    break;
                                }
                            } else {
                                return DecoderDirectAction::Error(
                                    FfmpegDecoderError::DecodingFailed,
                                );
                            }
                        }
                        _other => {
                            error!("Unexpected error while receiving frame: {:?}", _other);
                            return DecoderDirectAction::Error(FfmpegDecoderError::DecodingFailed);
                        }
                    },
                };
            }

            let received_frame = match received_frame {
                Some(f) => f,
                None => return DecoderDirectAction::NoFrame,
            };

            if received_frame.width() == 0
                || received_frame.height() == 0
                || received_frame.format() == ffmpeg_next::format::Pixel::None
            {
                debug!(
                    "Video context still useless: {}x{} {:?}",
                    received_frame.width(),
                    received_frame.height(),
                    received_frame.format()
                );
                // Invalid frame, skip it
                return DecoderDirectAction::NoFrame;
            }

            let current_frame_format = (
                received_frame.width(),
                received_frame.height(),
                received_frame.format(),
            );

            // TODO Set up scaler.
            if state.scaler.is_none()
                || state.frame_params.is_none_or(|p| p != current_frame_format)
            {
                // Initialize scaler here
                debug!(
                    "Creating scaler with params {}x{} {:?} scaling to {}x{} {:?}",
                    received_frame.width(),
                    received_frame.height(),
                    received_frame.format(),
                    received_frame.width(),
                    received_frame.height(),
                    ffmpeg_next::format::Pixel::RGBA
                );
                let scaler = match ScalingContext::get(
                    received_frame.format(),
                    received_frame.width(),
                    received_frame.height(),
                    ffmpeg_next::format::Pixel::RGBA,
                    received_frame.width(),
                    received_frame.height(),
                    ffmpeg_next::software::scaling::flag::Flags::BILINEAR,
                ) {
                    Ok(s) => s,
                    Err(_e) => {
                        return DecoderDirectAction::Error(FfmpegDecoderError::DecodingFailed);
                    }
                };
                state.scaler = Some(scaler);
                state.frame_params = Some(current_frame_format);
            }

            // Above we should've just created it.
            let scaler = state.scaler.as_mut().unwrap();

            let mut scaled_frame = ffmpeg_next::frame::Video::new(
                ffmpeg_next::format::Pixel::RGBA,
                received_frame.width(),
                received_frame.height(),
            );

            match scaler.run(&received_frame, &mut scaled_frame) {
                Ok(_) => {}
                Err(e) => {
                    error!("Scaling failed: {:?}", e);
                    return DecoderDirectAction::Error(FfmpegDecoderError::DecodingFailed);
                }
            }

            let stride_bytes = scaled_frame.stride(0);
            let width_bytes = (scaled_frame.width() * 4) as usize;
            let height_pixels = scaled_frame.height() as usize;
            let data = &scaled_frame.data(0);

            // If data is neatly packed, just write that
            if stride_bytes == width_bytes {
                match output_buffer.write_all(&data[..width_bytes * height_pixels]) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Write failed: {:?}", e);
                        return DecoderDirectAction::Error(FfmpegDecoderError::DecodingFailed);
                    }
                }
            } else {
                for row in 0..height_pixels {
                    let row_start = row * stride_bytes;
                    let row_end = row_start + width_bytes;
                    match output_buffer.write_all(data[row_start..row_end].as_ref()) {
                        Ok(_) => {}
                        Err(e) => {
                            error!("Write failed: {:?}", e);
                            return DecoderDirectAction::Error(FfmpegDecoderError::DecodingFailed);
                        }
                    }
                }
            }

            DecoderDirectAction::Decoded
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
