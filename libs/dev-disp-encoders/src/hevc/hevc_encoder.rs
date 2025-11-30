use std::time::Instant;

use dev_disp_core::{
    host::{
        Encoder as DevDispEncoder, EncoderParameters, EncoderProvider, VirtualScreenPixelFormat,
    },
    util::PinnedLocalFuture,
};
use ffmpeg_next::{
    self as ffmpeg, Dictionary, codec::encoder::video::Encoder as VideoEncoder, format::Pixel,
    frame::Video, software::scaling::Context as ScalingContext,
};
use futures::FutureExt;
use log::debug;

use crate::{
    hevc::configurations::{FfmpegEncoderConfiguration, get_encoders},
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

pub struct HevcEncoder {
    state: Option<HevcEncoderState>,
}

impl HevcEncoder {
    pub fn new() -> Self {
        HevcEncoder { state: None }
    }

    fn try_init(
        &mut self,
        parameters: EncoderParameters,
        configuration: FfmpegEncoderConfiguration,
    ) -> Result<HevcEncoderState, String> {
        let codec = ffmpeg::encoder::find_by_name(&configuration.encoder_name)
            .ok_or_else(|| format!("Encoder '{}' not found", configuration.encoder_name))?;

        debug!("Using HEVC encoder: {}", codec.name());

        let mut context = ffmpeg::codec::context::Context::new_with_codec(codec)
            .encoder()
            .video()
            .map_err(|e| format!("Failed to create video codec context: {}", e))?;

        context.set_height(parameters.height);
        context.set_width(parameters.width);
        context.set_format(configuration.pixel_format);
        context.set_time_base((1, parameters.fps as i32));

        let options = Dictionary::from_iter(configuration.encoder_options.into_iter());
        let encoder = context
            .open_with(options)
            .map_err(|e| format!("Failed to open encoder: {}", e))?;

        let scaler = ScalingContext::get(
            ffmpeg_format_from_internal_format(&parameters.input_parameters.format),
            parameters.input_parameters.width,
            parameters.input_parameters.height,
            configuration.pixel_format,
            parameters.width,
            parameters.height,
            ffmpeg::software::scaling::flag::Flags::BILINEAR,
        )
        .map_err(|e| format!("Failed to create scaler: {}", e))?;

        let state = HevcEncoderState {
            encoder,
            scaler,
            given_params: parameters,
            frame_index: 0,
            encoder_fmt: configuration.pixel_format,
            // TODO: Compute buffer-size based on parameters!
            out_buf: Vec::with_capacity(1024 * 1024),
        };

        Ok(state)
    }
}

impl DevDispEncoder for HevcEncoder {
    fn init(&mut self, parameters: EncoderParameters) -> PinnedLocalFuture<'_, Result<(), String>> {
        async move {
            ffmpeg::init().map_err(|e| format!("Failed to initialize ffmpeg: {}", e))?;

            let mut encoders = get_encoders();

            while let Some(configuration) = encoders.next() {
                debug!(
                    "Trying encoder configuration: {} with options {:?} and pixel format {:?}",
                    configuration.encoder_name,
                    configuration.encoder_options,
                    configuration.pixel_format
                );

                match self.try_init(parameters.clone(), configuration.clone()) {
                    Ok(state) => {
                        debug!(
                            "Successfully initialized encoder: {} with options {:?} and pixel format {:?}",
                            configuration.encoder_name,
                            configuration.encoder_options,
                            configuration.pixel_format
                        );
                        self.state = Some(state);
                        return Ok(());
                    }
                    Err(e) => {
                        debug!(
                            "Failed to initialize encoder: {} with options {:?} and pixel format {:?}: {}",
                            configuration.encoder_name,
                            configuration.encoder_options,
                            configuration.pixel_format,
                            e
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

            let mut input_frame = Video::new(
                ffmpeg_format_from_internal_format(&state.given_params.input_parameters.format),
                state.given_params.input_parameters.width,
                state.given_params.input_parameters.height,
            );

            let alloc_input_frame = start.elapsed();

            let bpp = match state.given_params.input_parameters.format {
                VirtualScreenPixelFormat::Rgb888 | VirtualScreenPixelFormat::Bgr888 => 3,
                _ => 4,
            };

            let width = state.given_params.input_parameters.width as usize;
            let height = state.given_params.input_parameters.height as usize;
            let src_stride = width * bpp;
            let dst_stride = input_frame.stride(0);
            let data = input_frame.data_mut(0);

            if raw_data.len() < height * src_stride {
                return Err(format!(
                    "Input buffer too small. Expected {}, got {}",
                    height * src_stride,
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
            let copy_time = Instant::now() - copy_start;

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

impl HevcEncoderProvider {
    pub fn new() -> Self {
        HevcEncoderProvider {}
    }
}

impl EncoderProvider for HevcEncoderProvider {
    type EncoderType = HevcEncoder;

    fn create_encoder(&self) -> Result<Self::EncoderType, String> {
        Ok(HevcEncoder::new())
    }
}
