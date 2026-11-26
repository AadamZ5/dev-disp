use dev_disp_core::core::ConfigurationFile;
use futures::FutureExt;
use serde::{Deserialize, Serialize};

use crate::{
    ffmpeg::decoder::configuration::FfmpegDecoderCodecConfiguration, toolkit::codecs::CodecFamily,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FfmpegDecoderConfiguration {
    /// The configured codec choices for each codec family.
    ///
    /// Ex, the codec family `H264` might have multiple decoder configurations available,
    /// and you can configure all of them in priority order to try for that family.
    ///
    /// Alternatively, you can configure all GPU-based decoders first, falling back to
    /// the software-based decoders if no suitable GPU-based decoder works.
    pub codec_choices: Vec<FfmpegDecoderCodecConfiguration>,
}

impl Default for FfmpegDecoderConfiguration {
    fn default() -> Self {
        Self {
            codec_choices: vec![FfmpegDecoderCodecConfiguration {
                codec_family: CodecFamily::Vp09,
                codec_name: "libvpx-vp9".to_string(),
                decoder_options: Default::default(),
            }],
        }
    }
}

impl ConfigurationFile for FfmpegDecoderConfiguration {
    fn display_name() -> String {
        "FFmpeg Decoder Configuration".to_string()
    }

    fn get_default_path(
        project_config: &std::path::Path,
    ) -> Result<std::path::PathBuf, dev_disp_core::core::ConfigurationFilePathError> {
        Ok(project_config.join("ffmpeg_decoder_config.json"))
    }

    fn serialize(
        &self,
    ) -> dev_disp_core::util::PinnedLocalFuture<'_, Result<Vec<u8>, Box<dyn std::error::Error>>>
    {
        async move { Ok(serde_json::to_vec(&self)?) }.boxed_local()
    }

    fn deserialize(
        mut source: Vec<u8>,
    ) -> dev_disp_core::util::PinnedLocalFuture<'static, Result<Self, Box<dyn std::error::Error>>>
    {
        // Strip JSON comments
        async move {
            let slice = source.as_mut_slice();
            json_strip_comments::strip_slice(slice)?;
            let config = serde_json::from_slice::<FfmpegDecoderConfiguration>(slice)?;
            Ok(config)
        }
        .boxed_local()
    }
}
