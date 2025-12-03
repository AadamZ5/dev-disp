use std::{collections::HashMap, fmt::Debug, time::Instant};

use dev_disp_core::{
    host::{
        Encoder as DevDispEncoder, EncoderParameters, EncoderPossibleConfiguration,
        EncoderProvider, VirtualScreenPixelFormat,
    },
    util::PinnedLocalFuture,
};
use ffmpeg_next::{
    self as ffmpeg, Dictionary, codec::encoder::video::Encoder as VideoEncoder, format::Pixel,
    frame::Video, software::scaling::Context as ScalingContext,
};
use futures::FutureExt;
use log::{debug, info};

use crate::{
    hevc::configurations::{
        FfmpegEncoderConfiguration, get_encoders, get_relevant_codec_parameters,
    },
    util::ffmpeg_format_from_internal_format,
};

struct HevcEncoderState {
    encoder: VideoEncoder,
    scaler: ScalingContext,
    encoder_fmt: Pixel,
    given_params: EncoderParameters,
    frame_index: u64,
    out_buf: Vec<u8>,
}

impl Debug for HevcEncoderState {
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

// TODO: Rename me! I am no longer just an HEVC encoder, but a generic FFmpeg-based encoder!
#[derive(Debug, Default)]
pub struct HevcEncoder {
    state: Option<HevcEncoderState>,
}

pub fn get_encoder(
    parameters: &EncoderParameters,
    configuration: &FfmpegEncoderConfiguration,
) -> Result<VideoEncoder, String> {
    let mut codec = ffmpeg::encoder::find_by_name(&configuration.encoder_name)
        .ok_or_else(|| format!("Encoder '{}' not found", configuration.encoder_name))?;

    debug!("Initializing HEVC encoder: {}", codec.name(),);

    let mut context = ffmpeg::codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()
        .map_err(|e| format!("Failed to create video codec context: {}", e))?;

    context.set_height(parameters.height);
    context.set_width(parameters.width);
    context.set_format(configuration.pixel_format);
    context.set_time_base((1, parameters.fps as i32));

    let options = Dictionary::from_iter(configuration.encoder_options.clone().into_iter());
    context
        .open_with(options)
        .map_err(|e| format!("Failed to open encoder: {}", e))
}

impl HevcEncoder {
    fn try_init(
        &mut self,
        parameters: EncoderParameters,
        configuration: FfmpegEncoderConfiguration,
    ) -> Result<HevcEncoderState, String> {
        let encoder = get_encoder(&parameters, &configuration)?;

        let scaler = ScalingContext::get(
            ffmpeg_format_from_internal_format(&parameters.encoder_input_parameters.format),
            parameters.encoder_input_parameters.width,
            parameters.encoder_input_parameters.height,
            configuration.pixel_format,
            parameters.width,
            parameters.height,
            ffmpeg::software::scaling::flag::Flags::FAST_BILINEAR,
        )
        .map_err(|e| format!("Failed to create scaler: {}", e))?;

        info!(
            "Initialized encoder: {}",
            encoder.codec().unwrap().video().unwrap().description()
        );

        let state = HevcEncoderState {
            encoder,
            scaler,
            given_params: parameters,
            frame_index: 0,
            encoder_fmt: configuration.pixel_format,
            // 16 KB initial buffer size for HEVC output
            out_buf: Vec::with_capacity(1024 * 16),
        };

        Ok(state)
    }
}

impl DevDispEncoder for HevcEncoder {
    fn get_supported_configurations(
        &mut self,
        parameters: &EncoderParameters,
    ) -> Result<Vec<EncoderPossibleConfiguration>, String> {
        let supported_configurations: Vec<_> = get_encoders()
            .filter_map(|config| match get_encoder(parameters, &config) {
                Ok(encoder) => Some((encoder, config, parameters)),
                Err(_) => None,
            })
            .map(|(encoder, config, _)| {
                let codec_params = get_relevant_codec_parameters(&config, &encoder);

                EncoderPossibleConfiguration {
                    encoder_name: config.encoder_name,
                    encoder_family: config.encoder_family,
                    encoded_resolution: (parameters.width, parameters.height),
                    parameters: codec_params,
                }
            })
            .collect();

        Ok(supported_configurations)
    }

    fn init(
        &mut self,
        parameters: EncoderParameters,
        preferred_encoders: Option<Vec<EncoderPossibleConfiguration>>,
    ) -> PinnedLocalFuture<'_, Result<EncoderPossibleConfiguration, String>> {
        async move {
            ffmpeg::init().map_err(|e| format!("Failed to initialize ffmpeg: {}", e))?;

            let mut encoders: Box<dyn Iterator<Item = FfmpegEncoderConfiguration>>;

            match preferred_encoders {
                None => {
                    info!("No preferred encoders specified, will try all available HEVC encoders.");
                    encoders = Box::new(get_encoders());
                }
                Some(ref prefs) => {
                    info!(
                        "Trying preferred encoders in order: {:?}",
                        prefs
                            .iter()
                            .map(|e| e.encoder_name.clone())
                            .collect::<Vec<_>>()
                    );
                    encoders = Box::new(get_encoders().filter(move |config| {
                        prefs.iter().any(|preferred| {
                            preferred.encoder_name == config.encoder_name
                                && preferred.encoder_family == config.encoder_family
                        })
                    }));
                }
            }

            while let Some(configuration) = encoders.next() {
                debug!(
                    "Trying encoder configuration: {} with options {:#?} and pixel format {:#?}",
                    configuration.encoder_name,
                    configuration.encoder_options,
                    configuration.pixel_format
                );

                match self.try_init(parameters.clone(), configuration.clone()) {
                    Ok(state) => {
                        debug!(
                            "Successfully initialized encoder: {}",
                            configuration.encoder_name
                        );

                        let codec_params =
                            get_relevant_codec_parameters(&configuration, &state.encoder);

                        let configuration = EncoderPossibleConfiguration {
                            encoder_name: configuration.encoder_name,
                            encoder_family: configuration.encoder_family,
                            encoded_resolution: (parameters.width, parameters.height),
                            parameters: codec_params,
                        };

                        self.state = Some(state);

                        return Ok(configuration);
                    }
                    Err(e) => {
                        debug!(
                            "Failed to initialize encoder \"{}\": {}",
                            configuration.encoder_name, e
                        );
                    }
                }
            }

            Err("Failed to find an HEVC codec to use!".to_string())
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
            let state = self.state.as_mut().ok_or("Encoder not initialized")?;

            // Perform HEVC encoding on the raw data
            // Return the encoded data

            let start = Instant::now();

            // Frame representing input data before scaling
            let mut input_frame = Video::new(
                ffmpeg_format_from_internal_format(&state.given_params.encoder_input_parameters.format),
                state.given_params.encoder_input_parameters.width,
                state.given_params.encoder_input_parameters.height,
            );
            let alloc_input_frame = start.elapsed();

            let height = state.given_params.encoder_input_parameters.height as usize;
            let src_stride = state.given_params.encoder_input_parameters.stride as usize;
            let dst_stride = input_frame.stride(0);
            let data = input_frame.data_mut(0);

            let expected_data = src_stride * height;
            if raw_data.len() < expected_data {
                return Err(format!(
                    "Input buffer too small. Expected {}, got {}",
                    expected_data,
                    raw_data.len()
                ));
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
            let mut yuv_frame = Video::new(
                state.encoder_fmt,
                state.given_params.width,
                state.given_params.height,
            );

            // Scale the input frame to the encoder's input format
            let scale_start = Instant::now();
            state
                .scaler
                .run(&input_frame, &mut yuv_frame)
                .map_err(|e| format!("Failed to scale frame: {}", e))?;

            yuv_frame.set_pts(Some(state.frame_index as i64));
            state.frame_index += 1;
            let scale_time = scale_start.elapsed();

            // Send for encoding
            let encode_start = Instant::now();
            state
                .encoder
                .send_frame(&yuv_frame)
                .map_err(|e| format!("Failed to send frame to encoder: {}", e))?;

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
            debug!(
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

pub struct HevcEncoderProvider;

impl EncoderProvider for HevcEncoderProvider {
    type EncoderType = HevcEncoder;

    fn create_encoder(&self) -> Result<Self::EncoderType, String> {
        Ok(HevcEncoder::default())
    }
}
