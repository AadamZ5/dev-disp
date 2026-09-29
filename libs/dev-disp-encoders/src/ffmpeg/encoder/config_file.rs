use std::path::{Path, PathBuf};

use dev_disp_core::{
    core::{ConfigurationFile, ConfigurationFilePathError},
    util::PinnedLocalFuture,
};
use futures::FutureExt;
use serde::{Deserialize, Serialize};

use crate::ffmpeg::encoder::configurations::{FfmpegEncoderConfigurationSet, get_encoders};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegEncoderConfiguration {
    pub encoder_configurations: Vec<FfmpegEncoderConfigurationSet>,
}

impl Default for FfmpegEncoderConfiguration {
    fn default() -> Self {
        FfmpegEncoderConfiguration {
            encoder_configurations: get_encoders().into_inner(),
        }
    }
}

impl ConfigurationFile for FfmpegEncoderConfiguration {
    fn display_name() -> String {
        "FFmpeg Encoder Configuration".to_string()
    }

    fn get_default_path(project_path: &Path) -> Result<PathBuf, ConfigurationFilePathError> {
        Ok(project_path.join("ffmpeg_configuration.json"))
    }

    fn serialize(&self) -> PinnedLocalFuture<'_, Result<Vec<u8>, Box<dyn std::error::Error>>> {
        async move {
            let data = serde_json::to_vec_pretty(&self)?;
            Ok(data)
        }
        .boxed_local()
    }

    fn deserialize(
        mut source: Vec<u8>,
    ) -> PinnedLocalFuture<'static, Result<Self, Box<dyn std::error::Error>>> {
        async move {
            let slice = source.as_mut_slice();
            json_strip_comments::strip_slice(slice)?;
            let config = serde_json::from_slice::<FfmpegEncoderConfiguration>(slice)?;
            Ok(config)
        }
        .boxed_local()
    }
}
