use std::collections::HashMap;

use dev_disp_core::coding::{
    codecs::{Av1Parameters, Codec, CodecFamily, H264Parameters, HevcParameters, Vp09Parameters},
    encoder::EncoderContentParameters,
};
use ffmpeg_next::{
    codec::encoder::video::Encoder as VideoEncoder,
    ffi::{AV_LEVEL_UNKNOWN, AVPixelFormat, FF_PROFILE_UNKNOWN},
    format::Pixel,
};
use log::{debug, warn};
use rust_util::string_map_builder::StringMapBuilder;
use serde::{Deserialize, Serialize};

mod pixel_serialization {
    use std::str::FromStr;

    use ffmpeg_next::format::Pixel;
    use log::warn;
    use serde::{self, Deserializer, Serializer};

    pub fn serialize<S>(pixels: &Vec<Pixel>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let pixel_strings: Vec<String> = pixels
            .iter()
            .map(|p| format!("{:?}", p).to_lowercase())
            .collect();
        serde::Serialize::serialize(&pixel_strings, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Pixel>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let pixel_strings: Vec<String> = serde::Deserialize::deserialize(deserializer)?;

        let pixels = pixel_strings
            .into_iter()
            .filter_map(|s| match Pixel::from_str(&s) {
                Ok(pix) => Some(pix),
                Err(_) => {
                    warn!("Unknown pixel format string during deserialization: {}", s);
                    None
                }
            })
            .collect();

        Ok(pixels)
    }
}

/// Combines lists of FFmpeg encoder options and pixel formats
/// to try for a particular encoder.
///
/// The `FfmpegEncoderConfigurationSet` can be used as an iterator and will
/// iterate over all pixel formats for each set of encoder options. Once
/// the pixel formats are exhausted for a given option set, it will move
/// on to the next option set and retry all pixel formats again.
///
/// You can deduce encoders and options by running
/// `ffmpeg -encoders` and `ffmpeg -h encoder=ENCODER_NAME`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegEncoderConfigurationSet {
    /// The encoder's FFmpeg name, e.g. "hevc_nvenc".
    pub encoder_name: String,
    /// The encoder family, e.g. "hvc1".
    pub encoder_family: CodecFamily,
    /// A list of options to try this encoder with. More desired
    /// combinations should be placed first.
    pub encoder_option_sets: Vec<HashMap<String, String>>,
    /// A list of pixel formats to try this encoder with. More desired
    /// formats should be placed first.
    #[serde(with = "pixel_serialization")]
    pub pixel_formats: Vec<Pixel>,

    #[serde(skip)]
    encoder_option_set_index: usize,
    #[serde(skip)]
    pixel_format_index: usize,
}

impl FfmpegEncoderConfigurationSet {
    pub fn new<T>(
        encoder_name: T,
        encoder_family: CodecFamily,
        encoder_option_sets: Vec<HashMap<String, String>>,
        pixel_formats: Vec<Pixel>,
    ) -> Self
    where
        T: Into<String>,
    {
        FfmpegEncoderConfigurationSet {
            encoder_name: encoder_name.into(),
            encoder_family: encoder_family,
            encoder_option_sets,
            pixel_formats,
            encoder_option_set_index: 0,
            pixel_format_index: 0,
        }
    }
}

impl Iterator for FfmpegEncoderConfigurationSet {
    type Item = FfmpegEncoderConfiguration;

    fn next(&mut self) -> Option<Self::Item> {
        // Iterate over each combination of encoder options and pixel formats

        if self.encoder_option_set_index >= self.encoder_option_sets.len() {
            return None;
        }

        if self.pixel_format_index >= self.pixel_formats.len() {
            self.pixel_format_index = 0;
            self.encoder_option_set_index += 1;

            if self.encoder_option_set_index >= self.encoder_option_sets.len() {
                return None;
            }
        }

        let options = if self.encoder_option_sets.is_empty() {
            HashMap::new()
        } else {
            self.encoder_option_sets[self.encoder_option_set_index].clone()
        };

        let config = FfmpegEncoderConfiguration {
            codec_name: self.encoder_name.clone(),
            codec_family: self.encoder_family.clone(),
            encoder_options: options,
            pixel_format: self.pixel_formats[self.pixel_format_index],
        };

        self.pixel_format_index += 1;

        Some(config)
    }
}

/// A particular FFmpeg encoder configuration.
#[derive(Debug, Clone)]
pub struct FfmpegEncoderConfiguration {
    /// An exact FFmpeg encoder name, e.g., "libx264".
    pub codec_name: String,
    pub codec_family: CodecFamily,
    /// Encoder options for ffmpeg, these are not the internal codec parameters.
    pub encoder_options: HashMap<String, String>,
    pub pixel_format: Pixel,
}

/// An iterator over multiple FFmpeg encoder configurations to try in sequence.
///
/// Given each `FfmpegEncoderConfigurationSet`, it will iterate over all possible
/// configurations before moving on to the next set.
#[derive(Debug, Clone, Default)]
pub struct FfmpegEncoderBruteForceIterator {
    configurations: Vec<FfmpegEncoderConfigurationSet>,
    current_index: usize,
}

impl FfmpegEncoderBruteForceIterator {
    pub fn new<T>(configurations: T) -> Self
    where
        T: IntoIterator<Item = FfmpegEncoderConfigurationSet>,
    {
        FfmpegEncoderBruteForceIterator {
            configurations: configurations.into_iter().collect(),
            current_index: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.configurations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.configurations.is_empty()
    }

    pub fn into_inner(self) -> Vec<FfmpegEncoderConfigurationSet> {
        self.configurations
    }

    pub fn into_iter_encoder_names(self) -> impl Iterator<Item = String> {
        self.configurations.into_iter().map(|set| set.encoder_name)
    }
}

impl Iterator for FfmpegEncoderBruteForceIterator {
    type Item = FfmpegEncoderConfiguration;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current_index >= self.configurations.len() {
                return None;
            } else {
                let config_set = &mut self.configurations[self.current_index];
                let config = config_set.next();

                if config.is_some() {
                    return config;
                } else {
                    self.current_index += 1;
                    continue;
                }
            }
        }
    }
}

pub fn get_encoders() -> FfmpegEncoderBruteForceIterator {
    // These are provided in order of preference, top to bottom left to right.
    FfmpegEncoderBruteForceIterator::new(vec![
        // I don't think this encoder exists
        FfmpegEncoderConfigurationSet::new(
            "hevc",
            CodecFamily::Hevc,
            vec![
                StringMapBuilder::new()
                    .insert("preset", "ultrafast")
                    .insert("tune", "zerolatency")
                    .build(),
            ],
            vec![Pixel::YUV420P],
        ),
        // Nvidia NVENC
        // Note if the driver is active but the GPU isn't connected,
        // it may take a long time to try to initialize and fail.
        FfmpegEncoderConfigurationSet::new(
            "hevc_nvenc",
            CodecFamily::Hevc,
            vec![
                StringMapBuilder::new()
                    .insert("preset", "llhq")
                    .insert("tune", "ull")
                    // ("profile", "main"),
                    .insert("delay", "0")
                    .insert("rc", "vbr_hq")
                    .insert("rc-lookahead", "0")
                    .insert("tier", "high")
                    .insert("multipass", "0")
                    .insert("cq", "20")
                    .insert("spatial-aq", "0")
                    .insert("temporal-aq", "0")
                    .insert("zerolatency", "1")
                    .build(),
            ],
            vec![
                // Putting RGB-like formats first so that any pixel conversion/scaling
                // can be done by the GPU instead of by ffmpeg software scaler.
                Pixel::RGBA,
                Pixel::BGRA,
                Pixel::YUV420P,
                Pixel::YUV444P,
                Pixel::YUV444P16LE,
                Pixel::NV12,
                Pixel::P010LE,
                Pixel::P016LE,
            ],
        ),
        // Intel Quick Sync Video
        FfmpegEncoderConfigurationSet::new(
            "hevc_qsv",
            CodecFamily::Hevc,
            vec![
                StringMapBuilder::new()
                    .insert("preset", "veryfast")
                    .insert("scenario", "displayremoting")
                    .build(),
            ],
            vec![
                Pixel::RGBA,
                Pixel::BGRA,
                Pixel::YUYV422,
                Pixel::NV12,
                Pixel::P010LE,
                Pixel::P012LE,
                Pixel::QSV,
                Pixel::VUYX,
            ],
        ),
        // AMD AMF
        FfmpegEncoderConfigurationSet::new(
            "hevc_vaapi",
            CodecFamily::Hevc,
            vec![],
            vec![Pixel::VAAPI],
        ),
        // Vulkan-based encoder
        FfmpegEncoderConfigurationSet::new(
            "hevc_vulkan",
            CodecFamily::Hevc,
            vec![
                StringMapBuilder::new()
                    .insert("usage", "stream")
                    .insert("tune", "ull")
                    .insert("content", "desktop")
                    .build(),
            ],
            vec![Pixel::VULKAN],
        ),
        // CPU-based software encoders
        FfmpegEncoderConfigurationSet::new(
            "libx265",
            CodecFamily::Hevc,
            vec![
                StringMapBuilder::new()
                    .insert("preset", "ultrafast")
                    .insert("tune", "zerolatency")
                    .build(),
            ],
            vec![Pixel::YUV420P],
        ),
        // Don't think this exists
        FfmpegEncoderConfigurationSet::new(
            "h265",
            CodecFamily::Hevc,
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        // Don't think this exists
        FfmpegEncoderConfigurationSet::new(
            "x265",
            CodecFamily::Hevc,
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        // Don't think this exists
        FfmpegEncoderConfigurationSet::new(
            "h264",
            CodecFamily::H264,
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        // Vulkan-based h264 encoder
        FfmpegEncoderConfigurationSet::new(
            "h264_vulkan",
            CodecFamily::H264,
            vec![
                StringMapBuilder::new()
                    .insert("tuning", "ll")
                    .insert("usage", "stream")
                    .insert("content", "desktop")
                    .build(),
            ],
            vec![Pixel::VULKAN],
        ),
        // CPU-based software h264 encoder
        FfmpegEncoderConfigurationSet::new(
            "libx264",
            CodecFamily::H264,
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        FfmpegEncoderConfigurationSet::new(
            "libx264",
            CodecFamily::H264,
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        FfmpegEncoderConfigurationSet::new(
            "vp9_qsv",
            CodecFamily::Vp09,
            vec![HashMap::new()],
            vec![
                Pixel::NV12,
                Pixel::P010LE,
                Pixel::VUYX,
                Pixel::QSV,
                Pixel::XV30LE,
            ],
        ),
        FfmpegEncoderConfigurationSet::new(
            "vp9_vaapi",
            CodecFamily::Vp09,
            vec![HashMap::default()],
            vec![Pixel::VAAPI],
        ),
        FfmpegEncoderConfigurationSet::new(
            "libvpx-vp9",
            CodecFamily::Vp09,
            // Tuned with realtime screen encoding by following
            // https://developers.google.com/media/vp9/live-encoding
            vec![
                StringMapBuilder::new()
                    .insert("deadline", "realtime")
                    .insert("quality", "realtime")
                    .insert("speed", "8")
                    .insert("tile-columns", "3")
                    .insert("frame-parallel", "1")
                    .insert("threads", "8")
                    .insert("static-thresh", "0")
                    .insert("max-intra-rate", "300")
                    .insert("lag-in-frames", "0")
                    .insert("qmin", "4")
                    .insert("qmax", "50")
                    .insert("row-mt", "1")
                    .insert("error-resilient", "1")
                    .build(),
            ],
            vec![
                Pixel::YUV420P,
                Pixel::YUV422P,
                Pixel::YUV440P,
                Pixel::YUV444P,
                // Seems like alpha channels encode slower
                Pixel::YUVA420P,
            ],
        ),
        FfmpegEncoderConfigurationSet::new(
            "libvpx",
            CodecFamily::Vp8,
            vec![
                StringMapBuilder::new()
                    .insert("deadline", "realtime")
                    .insert("quality", "realtime")
                    .insert("vp8flags", "altref")
                    .insert("lag-in-frames", "0")
                    .insert("cpu-used", "5")
                    .build(),
            ],
            vec![Pixel::YUV420P, Pixel::YUVA420P],
        ),
        FfmpegEncoderConfigurationSet::new(
            "libaom-av1",
            CodecFamily::Av1,
            vec![
                StringMapBuilder::new()
                    .insert("cpu-used", "8")
                    .insert("threads", "8")
                    .insert("tile-columns", "3")
                    .insert("row-mt", "1")
                    .insert("end-usage", "cbr")
                    .insert("lag-in-frames", "0")
                    .build(),
            ],
            vec![Pixel::YUV420P],
        ),
    ])
}

// TODO: This is pretty much getting codec parameters that the web codecs expect! Is there
// TODO: a more elegant way to structure this or define the contracts? Should our encoders
// TODO: crate define the supported codecs and parameters?
pub fn get_codec_params(
    encoder_preset: &FfmpegEncoderConfiguration,
    encoder: &VideoEncoder,
    input_parameters: &EncoderContentParameters,
) -> Option<Codec> {
    match encoder_preset.codec_family {
        CodecFamily::Raw => None,
        CodecFamily::Vp09 => unsafe {
            let ptr = encoder.as_ptr();

            let pix_fmt = (*ptr).pix_fmt;

            let (bit_depth, chroma_subsampling) = match pix_fmt {
                AVPixelFormat::AV_PIX_FMT_YUV420P => (8, 1),
                AVPixelFormat::AV_PIX_FMT_YUV422P => (8, 2),
                AVPixelFormat::AV_PIX_FMT_YUV444P => (8, 3),
                AVPixelFormat::AV_PIX_FMT_YUV440P => (8, 0),
                AVPixelFormat::AV_PIX_FMT_YUVA420P => (8, 1),
                AVPixelFormat::AV_PIX_FMT_YUV420P10LE => (10, 1),
                AVPixelFormat::AV_PIX_FMT_YUV422P10LE => (10, 2),
                AVPixelFormat::AV_PIX_FMT_YUV444P10LE => (10, 3),
                AVPixelFormat::AV_PIX_FMT_YUVA420P10LE => (10, 1),
                AVPixelFormat::AV_PIX_FMT_YUV420P12LE => (12, 1),
                AVPixelFormat::AV_PIX_FMT_YUV422P12LE => (12, 2),
                AVPixelFormat::AV_PIX_FMT_YUV444P12LE => (12, 3),
                _ => {
                    warn!("Unexpected pixel format {:?} for vp09 encoder", pix_fmt);
                    (8, 0)
                }
            };

            let profile = (*ptr).profile;
            let profile = if profile == FF_PROFILE_UNKNOWN {
                let inferred = match (bit_depth, chroma_subsampling) {
                    (8, 1) => 0,
                    (8, _) => 1,
                    (_, 1) => 2,
                    (_, _) => 3,
                };

                debug!(
                    "FF_PROFILE_UNKNOWN ({}): Inferring VP9 profile {} based on bit depth and chroma subsampling",
                    profile, inferred
                );
                inferred
            } else {
                profile
            };

            let level = (*ptr).level;
            let level = if level == AV_LEVEL_UNKNOWN { 10 } else { level };

            Some(Codec::Vp09(Vp09Parameters {
                bit_depth,
                profile: profile as u8,
                level: level as u8,
            }))
        },
        CodecFamily::Vp8 => Some(Codec::Vp8),
        CodecFamily::Hevc => unsafe {
            let ptr = encoder.as_ptr();

            let profile = (*ptr).profile;
            let profile = if profile == FF_PROFILE_UNKNOWN {
                warn!(
                    "FF_PROFILE_UNKNOWN ({}): Assuming default HEVC profile 1",
                    profile
                );
                1
            } else {
                profile
            };

            // Don't really know what this does, but people seem to
            // hard-code this value for HEVC.
            let compat = 0x06;

            let level = (*ptr).level;
            // Divide this int by 30 to get the level decimal number.
            // Ex, 90 / 30 = 3.0
            let level = if level == AV_LEVEL_UNKNOWN { 93 } else { level };

            // TODO: Find out how to get this value properly.
            let tier_letter = 'L';

            let constraints = 0xB0;

            Some(Codec::Hevc(HevcParameters {
                profile: profile as u8,
                level: level as u8,
                compatibility: compat,
                tier: tier_letter,
                constraints,
            }))
        },

        CodecFamily::Av1 => unsafe {
            let ptr = encoder.as_ptr();

            let profile = (*ptr).profile;
            let profile = if profile == FF_PROFILE_UNKNOWN {
                warn!(
                    "FF_PROFILE_UNKNOWN ({}): Assuming default AVC profile 66 (Baseline)",
                    profile
                );
                66
            } else {
                profile
            };

            let level = (*ptr).level;
            let level = if level == AV_LEVEL_UNKNOWN { 30 } else { level };

            warn!("AVC encoder profile constraints flags not yet implemented!");
            Some(Codec::Av1(Av1Parameters {
                profile: profile as u8,
                level: level as u8,
                // TODO: Add proper constraint flags
                constraint_flags: 0x00,
            }))
        },

        CodecFamily::H264 => unsafe {
            let ptr = encoder.as_ptr();

            let profile = (*ptr).profile;
            let profile = if profile == FF_PROFILE_UNKNOWN {
                warn!(
                    "FF_PROFILE_UNKNOWN ({}): Assuming default H264 profile 66 (Baseline)",
                    profile
                );
                66
            } else {
                profile
            };

            let level = (*ptr).level;
            let level = if level == AV_LEVEL_UNKNOWN { 30 } else { level };

            warn!("H264 encoder profile constraints flags not yet implemented!");

            Some(Codec::H264(H264Parameters {
                profile: profile as u8,
                level: level as u8,
                // TODO: Add proper constraint flags
                constraint_flags: 0x00,
            }))
        },
    }
}
