use std::{fmt::Debug, time::{Duration, Instant}};

use dev_disp_core::{
    host::ScreenContentParameters, util::{PinnedFuture, PinnedLocalFuture},
};
use ffmpeg_next::{
    self as ffmpeg, Dictionary, codec::{encoder::video::Encoder as VideoEncoder}, format::Pixel,
    frame::Video, software::scaling::Context as ScalingContext,
};
use futures::FutureExt;
use log::{debug, info, trace};
use thiserror::Error;

use crate::{
    toolkit::{encoder::{Encoder as DevDispEncoder, CodecOptionInternal, EncoderProvider}},
    ffmpeg::{config_file::FfmpegConfiguration, configurations::{
        FfmpegEncoderBruteForceIterator, FfmpegEncoderConfiguration, get_codec_params
    }, util::ffmpeg_format_from_internal_format}
};

struct FfmpegEncoderState {
    encoder: VideoEncoder,
    scaler: Option<ScalingContext>,
    encoder_fmt: Pixel,
    given_params: ScreenContentParameters,
    frame_index: u64,
    out_buf: Vec<u8>,
}

impl Debug for FfmpegEncoderState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HevcEncoderState")
            .field("encoder_fmt", &self.encoder_fmt)
            .field("given_params", &self.given_params)
            .field("frame_index", &self.frame_index)
            .field("encoder", &format!("video::Encoder@{:p}", &self.encoder))
            .field("scaler", &format!("scaling::Context@{:p}", &self.scaler))
            .finish()
    }
}

#[derive(Error, Debug)]
pub enum FfmpegEncoderError {
    #[error("Unknown error")]
    Unknown,
    #[error("Failed to initialize ffmpeg: {0}")]
    FfmpegInitError(ffmpeg_next::util::error::Error),
    #[error("Unknown encoder: {0}")]
    UnknownEncoder(String),
    #[error("Failed to initialize ffmpeg codec: {0}")]
    CodecInitError(ffmpeg_next::util::error::Error),
    #[error("Failed to initialize scaler: {0}")]
    ScalerInitError(ffmpeg_next::util::error::Error),
    #[error("Failed to negotiate a codec")]
    CodecNegotiationFailure(Vec<CodecOptionInternal<FfmpegEncoderConfiguration>>),
    #[error("Encoder not initialized before trying to encode")]
    EncoderNotInitialized,
    #[error("Input buffer too small. Expected {expected}, got {got}")]
    InputBufferTooSmall {
        expected: usize,
        got: usize,
    },
    #[error("Failed to send frame to encoder: {0}")]
    EncoderSendFailed(ffmpeg_next::util::error::Error),
    #[error("Failed to scale frame: {0}")]
    ScaleError(ffmpeg_next::util::error::Error),
}

#[derive(Debug, Default)]
pub struct FfmpegEncoder {
    state: Option<FfmpegEncoderState>,
    configuration: FfmpegConfiguration,
}

pub fn setup_ffmpeg_encoder(
    parameters: &ScreenContentParameters,
    configuration: &FfmpegEncoderConfiguration,
) -> Result<VideoEncoder, FfmpegEncoderError> {
    let codec = ffmpeg::encoder::find_by_name(&configuration.codec_name)
        .ok_or_else(|| FfmpegEncoderError::UnknownEncoder(configuration.codec_name.clone()))?;

    debug!("Initializing ffmpeg encoder: {}", codec.name(),);

    let mut context = ffmpeg::codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()
        .map_err(FfmpegEncoderError::CodecInitError)?;

    context.set_height(parameters.height);
    context.set_width(parameters.width);
    context.set_format(configuration.pixel_format);
    context.set_time_base((1, parameters.fps as i32));
    context.set_flags(ffmpeg::codec::flag::Flags::LOW_DELAY);


    let options = Dictionary::from_iter(configuration.encoder_options.clone().into_iter());
    context
        .open_with(options)
        .map_err(FfmpegEncoderError::CodecInitError)
}

impl FfmpegEncoder {

    fn new(configuration: FfmpegConfiguration) -> Self {
        FfmpegEncoder {
            state: None,
            configuration,
        }
    }

    fn try_init(
        &mut self,
        parameters: &ScreenContentParameters,
        configuration: &FfmpegEncoderConfiguration,
    ) -> Result<FfmpegEncoderState, FfmpegEncoderError> {
        let encoder = setup_ffmpeg_encoder(&parameters, configuration)?;

        let src_format =
            ffmpeg_format_from_internal_format(&parameters.virtual_screen_format_parameters.format);
        let dst_format = configuration.pixel_format;

        // If the source format matches the encoder's required format, no
        // scaling required (since we are not changing resolution here).
        let scaler = if dst_format == src_format {
            None
        } else {
            Some(
                ScalingContext::get(
                    src_format,
                    parameters.virtual_screen_format_parameters.width,
                    parameters.virtual_screen_format_parameters.height,
                    configuration.pixel_format,
                    parameters.width,
                    parameters.height,
                    ffmpeg::software::scaling::flag::Flags::POINT,
                )
                .map_err(FfmpegEncoderError::ScalerInitError)?,
            )
        };

        info!(
            "Initialized encoder: {}",
            encoder.codec().unwrap().video().unwrap().description()
        );

        let state = FfmpegEncoderState {
            encoder,
            scaler,
            given_params: parameters.clone(),
            frame_index: 0,
            encoder_fmt: configuration.pixel_format,
            // 16 KB initial buffer size for output
            out_buf: Vec::with_capacity(1024 * 16),
        };

        Ok(state)
    }
}

impl DevDispEncoder for FfmpegEncoder {

    type CodecData = FfmpegEncoderConfiguration;

    type Error = FfmpegEncoderError;

    fn get_supported_configurations(
        &mut self,
        parameters: &ScreenContentParameters,
    ) -> PinnedLocalFuture<'_, Result<Vec<CodecOptionInternal<FfmpegEncoderConfiguration>>, FfmpegEncoderError>> {

        // TODO: Try encoders in the provider, not here on every connection!
        // TODO: Make this async happen in a non-blocking way!

        let supported_configurations: Vec<_> = FfmpegEncoderBruteForceIterator::new(self.configuration.encoder_configurations.clone())
            .filter_map(|config| match setup_ffmpeg_encoder(parameters, &config) {
                Ok(encoder) => {
                    debug!(
                        "Encoder configuration {} supported",
                        config.codec_name
                    );
                    Some((encoder, config, parameters))
                },
                Err(e) => {
                    debug!(
                        "Encoder configuration {} not supported: {}",
                        config.codec_name, e
                    );
                    None
                },
            })
            .filter_map(|(encoder, config, input_parameters)| {
                let codec_params = get_codec_params(&config, &encoder, input_parameters);

                match codec_params {
                    Some(params) => Some(CodecOptionInternal{
                        display_name: config.codec_name.to_string(),
                        codec: params,
                        encoded_resolution: (input_parameters.width, input_parameters.height),
                        data: config,
                    }),
                    None => {
                        debug!(
                            "Failed to get codec parameters for configuration {}",
                            config.codec_name
                        );
                        return None;
                    }
                }
            })
            .collect();

        async move { Ok(supported_configurations) }.boxed_local()
    }

    fn set_codec<'s ,'p>(
        &'s mut self,
        parameters: &'p ScreenContentParameters,
        preferred_encoders: Option<Vec<&'p CodecOptionInternal<FfmpegEncoderConfiguration>>>,
        offered_encoders: Vec<&'p CodecOptionInternal<FfmpegEncoderConfiguration>>,
    ) -> PinnedLocalFuture<'s, Result<&'p CodecOptionInternal<FfmpegEncoderConfiguration>, Self::Error>> where 'p: 's {
        async move {
            ffmpeg::init().map_err(FfmpegEncoderError::FfmpegInitError)?;

            let mut encoders: Box<dyn Iterator<Item = &CodecOptionInternal<FfmpegEncoderConfiguration>>> = match preferred_encoders {
                // If the client did not respond with any preference, try all offered encoders.
                None => {
                    info!("No preferred encoders specified, will try all configured ffmpeg encoders.");
                    Box::new(offered_encoders.into_iter())
                }
                // Otherwise, use the client's preferred encoders in the specified order.
                Some(ref prefs) => {
                    info!(
                        "Trying preferred encoders in order: {:?}",
                        prefs
                            .iter()
                            .map(|e| e.display_name.clone())
                            .collect::<Vec<_>>()
                    );
                    
                    Box::new(prefs.iter().map(|e| *e))
                }
            };

            while let Some(configuration) = encoders.next() {
                debug!(
                    "Trying encoder configuration: {} with options {:#?} and pixel format {:#?}",
                    configuration.data.codec_name,
                    configuration.data.encoder_options,
                    configuration.data.pixel_format
                );

                match self.try_init(parameters, &configuration.data) {
                    Ok(state) => {

                        let has_scaler_str = match &state.scaler {
                            Some(s) => {
                                let input_format = s.input().format;
                                let output_format = s.output().format;
                                format!("with scaler ({:?} -> {:?})", input_format, output_format)
                            },
                            None => "without scaler".to_string(),
                        };

                        debug!(
                            "Successfully initialized encoder: {} {}",
                            configuration.data.codec_name,
                            has_scaler_str
                        );

                        self.state = Some(state);

                        return Ok(configuration);
                    }
                    Err(e) => {
                        debug!(
                            "Failed to initialize encoder \"{}\": {}",
                            configuration.data.codec_name, e
                        );
                    }
                }
            }

            let tried_encoders = preferred_encoders.clone().map(|p| p.into_iter().cloned().collect::<Vec<_>>()).unwrap_or_else(|| Vec::new());

            Err(FfmpegEncoderError::CodecNegotiationFailure(tried_encoders))
        }
        .boxed_local()
    }

    fn encode<'s, 'a>(
        &'s mut self,
        raw_data: &'a [u8],
    ) -> PinnedLocalFuture<'s, Result<&'s [u8], Self::Error>>
    where
        'a: 's,
    {
        async move {
            let state = self.state.as_mut().ok_or(FfmpegEncoderError::EncoderNotInitialized)?;

            // Perform encoding on the raw data
            // Return the encoded data

            let start = Instant::now();

            // Frame representing input data before scaling
            let mut input_frame = Video::new(
                ffmpeg_format_from_internal_format(&state.given_params.virtual_screen_format_parameters.format),
                state.given_params.virtual_screen_format_parameters.width,
                state.given_params.virtual_screen_format_parameters.height,
            );
            let alloc_input_frame = start.elapsed();

            let height = state.given_params.virtual_screen_format_parameters.height as usize;
            let src_stride = state.given_params.virtual_screen_format_parameters.stride as usize;
            let dst_stride = input_frame.stride(0);
            let data = input_frame.data_mut(0);

            let expected_data = src_stride * height;
            if raw_data.len() < expected_data {
                return Err(FfmpegEncoderError::InputBufferTooSmall {
                    expected: expected_data,
                    got: raw_data.len(),
                });
            }

            let copy_start = Instant::now();
            for i in 0..height {
                let src_start = i * src_stride;
                let src_end = src_start + src_stride;
                let dst_start = i * dst_stride;
                let dst_end = dst_start + src_stride;
                data[dst_start..dst_end].copy_from_slice(&raw_data[src_start..src_end]);
            }
            let copy_time = copy_start.elapsed();

            // The output frame after scaling.
            let mut scale_time = Duration::from_secs(0);
            let formatted_frame = if let Some(scaler) = state.scaler.as_mut() {
                let mut formatted_frame = Video::new(
                    state.encoder_fmt,
                    state.given_params.width,
                    state.given_params.height,
                );
                // Scale the input frame to the encoder's input format
                let scale_start = Instant::now();
                scaler
                    .run(&input_frame, &mut formatted_frame)
                    .map_err(FfmpegEncoderError::ScaleError)?;

                formatted_frame.set_pts(Some(state.frame_index as i64));
                state.frame_index += 1;
                scale_time = scale_start.elapsed();
                formatted_frame

            } else {
                input_frame
            };
            

            // Send for encoding
            let encode_start = Instant::now();
            state
                .encoder
                .send_frame(&formatted_frame)
                .map_err(FfmpegEncoderError::EncoderSendFailed)?;

            state.out_buf.clear();
            let mut packet = ffmpeg::Packet::empty();
            let mut consumed_len = 0;
            // TODO: Stream this data!
            while let Ok(_) = state.encoder.receive_packet(&mut packet) {
                match packet.data() {
                    Some(data) => {
                        consumed_len += data.len();
                        state.out_buf.extend_from_slice(data)
                    }
                    None => (),
                }
            }

            let encode_time = encode_start.elapsed();
            trace!(
                "Alloc input time: {}ms   Copy time: {}ms   Scale time: {}ms   Encode time: {}ms (round trip)",
                alloc_input_frame.as_millis(),
                copy_time.as_millis(),
                scale_time.as_millis(),
                encode_time.as_millis()
            );

            // Only return the used portion of the buffer
            let ret = &state.out_buf[..consumed_len];

            Ok(ret)
        }
        .boxed_local()
    }
}

#[derive(Debug, Clone)]
pub struct FfmpegEncoderProvider {
    configuration: FfmpegConfiguration,
}

impl FfmpegEncoderProvider {
    pub fn new(configuration: FfmpegConfiguration) -> Self {
        FfmpegEncoderProvider { configuration }
    }
}

impl EncoderProvider for FfmpegEncoderProvider {
    type EncoderType = FfmpegEncoder;

    fn create_encoder(&self) -> PinnedFuture<'_, Result<Self::EncoderType, String>> {
        async move {
            Ok(FfmpegEncoder::new(self.configuration.clone()))
        }
        .boxed()
    }
}
