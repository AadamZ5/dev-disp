use std::collections::HashMap;

use ffmpeg_next::format::Pixel;

#[derive(Debug, Clone)]
pub struct FfmpegEncoderConfigurationSet {
    pub encoder_name: String,
    pub encoder_option_sets: Vec<HashMap<&'static str, &'static str>>,
    pub pixel_formats: Vec<Pixel>,
    encoder_option_set_index: usize,
    pixel_format_index: usize,
}

impl FfmpegEncoderConfigurationSet {
    pub fn new(
        encoder_name: String,
        encoder_option_sets: Vec<HashMap<&'static str, &'static str>>,
        pixel_formats: Vec<Pixel>,
    ) -> Self {
        FfmpegEncoderConfigurationSet {
            encoder_name,
            encoder_option_sets,
            pixel_formats,
            encoder_option_set_index: 0,
            pixel_format_index: 0,
        }
    }

    fn get_iter_state(&self) -> (usize, usize) {
        (self.encoder_option_set_index, self.pixel_format_index)
    }

    fn set_iter_state(&mut self, option_set_index: usize, pixel_format_index: usize) {
        self.encoder_option_set_index = option_set_index;
        self.pixel_format_index = pixel_format_index;
    }
}

impl Iterator for FfmpegEncoderConfigurationSet {
    type Item = FfmpegEncoderConfiguration;

    fn next(&mut self) -> Option<Self::Item> {
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

        let config = FfmpegEncoderConfiguration {
            encoder_name: self.encoder_name.clone(),
            encoder_options: self.encoder_option_sets[self.encoder_option_set_index].clone(),
            pixel_format: self.pixel_formats[self.pixel_format_index],
        };

        self.pixel_format_index += 1;

        Some(config)
    }
}

#[derive(Debug, Clone)]
pub struct FfmpegEncoderConfiguration {
    pub encoder_name: String,
    pub encoder_options: HashMap<&'static str, &'static str>,
    pub pixel_format: Pixel,
}

#[derive(Debug, Clone)]
pub struct FfmpegEncoderBruteForceIterator {
    configurations: Vec<FfmpegEncoderConfigurationSet>,
    current_index: usize,
}

impl FfmpegEncoderBruteForceIterator {
    pub fn new() -> Self {
        FfmpegEncoderBruteForceIterator {
            configurations: Vec::new(),
            current_index: 0,
        }
    }

    pub fn new_from_iter<T>(configurations: T) -> Self
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

    pub fn into_inner(self) -> Vec<FfmpegEncoderConfigurationSet> {
        self.configurations
    }
}

impl Iterator for FfmpegEncoderBruteForceIterator {
    type Item = FfmpegEncoderConfiguration;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= self.configurations.len() {
            None
        } else {
            let config = &mut self.configurations[self.current_index];

            match config.next() {
                Some(cfg) => Some(cfg),
                None => {
                    self.current_index += 1;
                    return self.next();
                }
            }
        }
    }
}

pub fn get_encoders() -> FfmpegEncoderBruteForceIterator {
    FfmpegEncoderBruteForceIterator::new_from_iter(vec![
        FfmpegEncoderConfigurationSet::new(
            "hevc".to_string(),
            vec![HashMap::from([
                ("preset", "ultrafast"),
                ("tune", "zerolatency"),
            ])],
            vec![Pixel::YUV420P],
        ),
        FfmpegEncoderConfigurationSet::new(
            "libx265".to_string(),
            vec![HashMap::from([
                ("preset", "ultrafast"),
                ("tune", "zerolatency"),
            ])],
            vec![Pixel::YUV420P],
        ),
        FfmpegEncoderConfigurationSet::new(
            "h265".to_string(),
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        FfmpegEncoderConfigurationSet::new(
            "x265".to_string(),
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        FfmpegEncoderConfigurationSet::new(
            "h264".to_string(),
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
        FfmpegEncoderConfigurationSet::new(
            "h264_vulkan".to_string(),
            vec![HashMap::from([
                ("tuning", "ll"),
                ("usage", "stream"),
                ("content", "desktop"),
            ])],
            vec![Pixel::VULKAN],
        ),
        FfmpegEncoderConfigurationSet::new(
            "libx264".to_string(),
            vec![HashMap::new()],
            vec![Pixel::YUV420P],
        ),
    ])
}
