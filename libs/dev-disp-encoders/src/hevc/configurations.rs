use std::collections::HashMap;

use ffmpeg_next::{codec::encoder::video::Encoder as VideoEncoder, format::Pixel};
use log::warn;

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
        FfmpegEncoderConfigurationSet::new(
            "libvpx-vp9",
            "vp09",
            vec![HashMap::from([
                ("deadline", "realtime"),
                ("quality", "realtime"),
            ])],
            vec![Pixel::YUV420P],
        ),
        FfmpegEncoderConfigurationSet::new(
            "libvpx",
            "vp8",
            vec![HashMap::from([
                ("deadline", "realtime"),
                ("quality", "realtime"),
                ("vp8flags", "altref"),
            ])],
            vec![Pixel::YUVA420P, Pixel::YUV420P],
        ),
        FfmpegEncoderConfigurationSet::new(
            "libaom-av1",
            "av1",
            vec![HashMap::from([("usage", "realtime"), ("cpu-used", "4")])],
            vec![Pixel::YUV420P],
        ),
        // I don't think this encoder exists
        FfmpegEncoderConfigurationSet::new(
            "hevc",
            "hevc",
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
        //     "hevc",
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
            "hevc",
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
        FfmpegEncoderConfigurationSet::new("hevc_vaapi", "hevc", vec![], vec![Pixel::VAAPI]),
        // Vulkan-based encoder
        FfmpegEncoderConfigurationSet::new(
            "hevc_vulkan",
            "hevc",
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
            "hevc",
            vec![HashMap::from([
                ("preset", "ultrafast"),
                ("tune", "zerolatency"),
            ])],
            vec![Pixel::YUV420P],
        ),
        // Don't think this exists
        FfmpegEncoderConfigurationSet::new(
            "h265",
            "hevc",
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        // Don't think this exists
        FfmpegEncoderConfigurationSet::new(
            "x265",
            "hevc",
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
    ])
}

pub fn get_relevant_codec_parameters(
    encoder_family: &str,
    encoder: &VideoEncoder,
) -> HashMap<String, String> {
    let name = encoder.codec().map(|c| c.name().to_owned());

    match encoder_family {
        "hevc" => HashMap::from([
            ("preset".to_string(), "".to_string()),
            (
                "tune".to_string(),
                "Encoder tuning (e.g., zerolatency)".to_string(),
            ),
            ("bitrate".to_string(), "Target bitrate in kbps".to_string()),
            ("maxrate".to_string(), "Maximum bitrate in kbps".to_string()),
            ("bufsize".to_string(), "Buffer size in kbps".to_string()),
            ("g".to_string(), "GOP size (keyframe interval)".to_string()),
            (
                "rc-lookahead".to_string(),
                "Number of frames for lookahead".to_string(),
            ),
        ]),
        _ => {
            warn!(
                "Unknown codec family '{}', no relevant parameters available",
                encoder_family
            );
            HashMap::new()
        }
    }
}
