use std::{collections::HashMap, fmt::Display};

use ffmpeg_next::{
    codec::encoder::video::Encoder as VideoEncoder,
    ffi::{AVPixelFormat, FF_LEVEL_UNKNOWN, FF_PROFILE_UNKNOWN},
    format::Pixel,
};
use log::warn;

// The defined encoder families.
#[derive(Debug, Clone)]
pub enum FfmpegEncoderFamily {
    Hevc,
    H264,
    Vp09,
    Vp8,
    Av1,
}

impl Display for FfmpegEncoderFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FfmpegEncoderFamily::Hevc => write!(f, "hevc"),
            FfmpegEncoderFamily::H264 => write!(f, "h264"),
            FfmpegEncoderFamily::Vp09 => write!(f, "vp09"),
            FfmpegEncoderFamily::Vp8 => write!(f, "vp8"),
            FfmpegEncoderFamily::Av1 => write!(f, "av1"),
        }
    }
}

impl FfmpegEncoderFamily {
    pub fn to_web_codec_id(&self) -> &'static str {
        match self {
            FfmpegEncoderFamily::Hevc => "hvc1",
            FfmpegEncoderFamily::H264 => "avc1",
            FfmpegEncoderFamily::Vp09 => "vp09",
            FfmpegEncoderFamily::Vp8 => "vp08",
            FfmpegEncoderFamily::Av1 => "av01",
        }
    }
}

/// A set of FFmpeg encoder configurations to try for a particular encoder.
///
/// You can deduce encoders and options by running
/// `ffmpeg -encoders` and `ffmpeg -h encoder=ENCODER_NAME`.
#[derive(Debug, Clone)]
pub struct FfmpegEncoderConfigurationSet {
    pub encoder_name: String,
    pub encoder_family: String,
    pub encoder_option_sets: Vec<HashMap<&'static str, &'static str>>,
    pub pixel_formats: Vec<Pixel>,
    encoder_option_set_index: usize,
    pixel_format_index: usize,
}

impl FfmpegEncoderConfigurationSet {
    pub fn new<T>(
        encoder_name: T,
        encoder_family: T,
        encoder_option_sets: Vec<HashMap<&'static str, &'static str>>,
        pixel_formats: Vec<Pixel>,
    ) -> Self
    where
        T: Into<String>,
    {
        FfmpegEncoderConfigurationSet {
            encoder_name: encoder_name.into(),
            encoder_family: encoder_family.into(),
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
            encoder_name: self.encoder_name.clone(),
            encoder_family: self.encoder_family.clone(),
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
    pub encoder_name: String,
    pub encoder_family: String,
    pub encoder_options: HashMap<&'static str, &'static str>,
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
            "hvc1",
            vec![HashMap::from([
                ("preset", "ultrafast"),
                ("tune", "zerolatency"),
            ])],
            vec![Pixel::YUV420P],
        ),
        // Nvidia NVENC
        // Note if the driver is active but the GPU isn't connected,
        // it may take a long time to try to initialize and fail.
        // FfmpegEncoderConfigurationSet::new(
        //     "hevc_nvenc",
        //     "hvc1",
        //     vec![HashMap::from([("preset", "p1"), ("tune", "ull")])],
        //     vec![
        //         Pixel::YUV420P,
        //         Pixel::YUV444P,
        //         Pixel::RGBA,
        //         Pixel::YUV444P16LE,
        //         Pixel::NV12,
        //         Pixel::P010LE,
        //         Pixel::P016LE,
        //         Pixel::CUDA,
        //     ],
        // ),
        // Intel Quick Sync Video
        FfmpegEncoderConfigurationSet::new(
            "hevc_qsv",
            "hvc1",
            vec![HashMap::from([
                ("preset", "veryfast"),
                ("scenario", "displayremoting"),
            ])],
            vec![
                Pixel::YUYV422,
                Pixel::NV12,
                Pixel::P010LE,
                Pixel::P012LE,
                Pixel::QSV,
                Pixel::BGRA,
                Pixel::VUYX,
            ],
        ),
        // AMD AMF
        FfmpegEncoderConfigurationSet::new("hevc_vaapi", "hvc1", vec![], vec![Pixel::VAAPI]),
        // Vulkan-based encoder
        FfmpegEncoderConfigurationSet::new(
            "hevc_vulkan",
            "hvc1",
            vec![HashMap::from([
                ("usage", "stream"),
                ("tune", "ull"),
                ("content", "desktop"),
            ])],
            vec![Pixel::VULKAN],
        ),
        // CPU-based software encoders
        FfmpegEncoderConfigurationSet::new(
            "libx265",
            "hvc1",
            vec![HashMap::from([
                ("preset", "ultrafast"),
                ("tune", "zerolatency"),
            ])],
            vec![Pixel::YUV420P],
        ),
        // Don't think this exists
        FfmpegEncoderConfigurationSet::new(
            "h265",
            "hvc1",
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        // Don't think this exists
        FfmpegEncoderConfigurationSet::new(
            "x265",
            "hvc1",
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        // Don't think this exists
        FfmpegEncoderConfigurationSet::new(
            "h264",
            "h264",
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        // Vulkan-based h264 encoder
        FfmpegEncoderConfigurationSet::new(
            "h264_vulkan",
            "h264",
            vec![HashMap::from([
                ("tuning", "ll"),
                ("usage", "stream"),
                ("content", "desktop"),
            ])],
            vec![Pixel::VULKAN],
        ),
        // CPU-based software h264 encoder
        FfmpegEncoderConfigurationSet::new(
            "libx264",
            "h264",
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        FfmpegEncoderConfigurationSet::new(
            "libvpx-vp9",
            "vp09",
            // Tuned with realtime screen encoding by following
            // https://developers.google.com/media/vp9/live-encoding
            vec![HashMap::from([
                ("deadline", "realtime"),
                ("quality", "realtime"),
                ("speed", "8"),
                ("tile-columns", "3"),
                ("frame-parallel", "1"),
                ("threads", "8"),
                ("static-thresh", "0"),
                ("max-intra-rate", "300"),
                ("lag-in-frames", "0"),
                ("qmin", "4"),
                ("qmax", "50"),
                ("row-mt", "1"),
                ("error-resilient", "1"),
            ])],
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
            "vp8",
            vec![HashMap::from([
                ("deadline", "realtime"),
                ("quality", "realtime"),
                ("vp8flags", "altref"),
                ("lag-in-frames", "0"),
                ("cpu-used", "5"),
            ])],
            vec![Pixel::YUVA420P, Pixel::YUV420P],
        ),
        FfmpegEncoderConfigurationSet::new(
            "libaom-av1",
            "av1",
            vec![HashMap::from([("usage", "realtime"), ("cpu-used", "4")])],
            vec![Pixel::YUV420P],
        ),
    ])
}

pub fn get_relevant_codec_parameters(
    encoder_preset: &FfmpegEncoderConfiguration,
    encoder: &VideoEncoder,
) -> HashMap<String, String> {
    match encoder_preset.encoder_family.as_str() {
        "vp09" => unsafe {
            let ptr = encoder.as_ptr();

            let profile = (*ptr).profile;
            let profile = if profile == FF_PROFILE_UNKNOWN {
                0
            } else {
                profile
            };

            let level = (*ptr).level;
            let level = if level == FF_LEVEL_UNKNOWN { 10 } else { level };

            let pix_fmt = (*ptr).pix_fmt;

            let (bit_depth, chroma_subsampling) = match pix_fmt {
                AVPixelFormat::AV_PIX_FMT_YUV420P => (8, 1),
                AVPixelFormat::AV_PIX_FMT_YUV422P => (8, 2),
                AVPixelFormat::AV_PIX_FMT_YUV444P => (8, 3),
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

            HashMap::from([
                ("profile".to_string(), profile.to_string()),
                ("level".to_string(), level.to_string()),
                ("bitDepth".to_string(), bit_depth.to_string()),
                (
                    "chromaSubsampling".to_string(),
                    chroma_subsampling.to_string(),
                ),
            ])
        },
        "vp8" => HashMap::new(),
        "hvc1" => {
            unsafe {
                let ptr = encoder.as_ptr();

                let profile = (*ptr).profile;
                let profile = if profile == FF_PROFILE_UNKNOWN {
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
                let level = if level == FF_LEVEL_UNKNOWN { 93 } else { level };

                // TODO: Find out how to get this value properly.
                let tier_letter = "L";

                let constraits = 0xB0;

                HashMap::from([
                    ("profile".to_string(), profile.to_string()),
                    ("compatibility".to_string(), format!("{:02X}", compat)),
                    ("level".to_string(), level.to_string()),
                    ("tier".to_string(), tier_letter.to_string()),
                    ("constraints".to_string(), format!("{:02X}", constraits)),
                ])
            }
        }
        _ => {
            warn!(
                "No parameter logic defined for encoder family {}",
                encoder_preset.encoder_family
            );
            HashMap::new()
        }
    }
}
