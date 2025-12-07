use std::{collections::HashMap, fmt::Display};

use ffmpeg_next::{
    codec::encoder::video::Encoder as VideoEncoder,
    ffi::{AVPixelFormat, FF_LEVEL_UNKNOWN, FF_PROFILE_UNKNOWN},
    format::Pixel,
};
use log::warn;
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
            FfmpegEncoderFamily::Vp8 => "vp8",
            FfmpegEncoderFamily::Av1 => "av01",
        }
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
    pub encoder_family: String,
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
        encoder_family: T,
        encoder_option_sets: Vec<HashMap<String, String>>,
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

struct StringMapBuilder {
    inner: HashMap<String, String>,
}

impl StringMapBuilder {
    fn new() -> Self {
        StringMapBuilder {
            inner: HashMap::new(),
        }
    }

    fn insert<T1, T2>(mut self, key: T1, value: T2) -> Self
    where
        T1: Into<String>,
        T2: Into<String>,
    {
        self.inner.insert(key.into(), value.into());
        self
    }

    fn build(self) -> HashMap<String, String> {
        self.inner
    }
}

pub fn get_encoders() -> FfmpegEncoderBruteForceIterator {
    // These are provided in order of preference, top to bottom left to right.
    FfmpegEncoderBruteForceIterator::new(vec![
        // I don't think this encoder exists
        FfmpegEncoderConfigurationSet::new(
            "hevc",
            "hvc1",
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
            "hvc1",
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
            "hvc1",
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
        FfmpegEncoderConfigurationSet::new("hevc_vaapi", "hvc1", vec![], vec![Pixel::VAAPI]),
        // Vulkan-based encoder
        FfmpegEncoderConfigurationSet::new(
            "hevc_vulkan",
            "hvc1",
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
            "hvc1",
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
            "h264",
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        FfmpegEncoderConfigurationSet::new(
            "libx264",
            "h264",
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        FfmpegEncoderConfigurationSet::new(
            "vp9_qsv",
            "vp09",
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
            "vp09",
            vec![HashMap::default()],
            vec![Pixel::VAAPI],
        ),
        FfmpegEncoderConfigurationSet::new(
            "libvpx-vp9",
            "vp09",
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
            "vp8",
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
            "av1",
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

            StringMapBuilder::new()
                .insert("profile", profile.to_string())
                .insert("level", level.to_string())
                .insert("bitDepth", bit_depth.to_string())
                // (
                //     "chromaSubsampling",
                //     chroma_subsampling,
                // ),
                .build()
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

                let constraints = 0xB0;

                StringMapBuilder::new()
                    .insert("profile", profile.to_string())
                    .insert("compatibility", format!("{:02X}", compat))
                    .insert("level", level.to_string())
                    .insert("tier", tier_letter)
                    .insert("constraints", format!("{:02X}", constraints))
                    .build()
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
