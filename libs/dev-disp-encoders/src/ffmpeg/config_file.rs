use std::path::{Path, PathBuf};

use dev_disp_core::core::ConfigurationFile;
use futures::FutureExt;
use serde::{Deserialize, Serialize};

use crate::ffmpeg::configurations::{FfmpegEncoderConfigurationSet, get_encoders};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegConfiguration {
    pub encoder_configurations: Vec<FfmpegEncoderConfigurationSet>,
}

impl Default for FfmpegConfiguration {
    fn default() -> Self {
        FfmpegConfiguration {
            encoder_configurations: get_encoders().into_inner(),
        }
    }
}

impl ConfigurationFile for FfmpegConfiguration {
    fn display_name() -> String {
        "FFmpeg Encoder Configuration".to_string()
    }

    fn get_default_path(
        project_path: &Path,
    ) -> Result<PathBuf, dev_disp_core::core::ConfigurationFilePathError> {
        let mut path_buf = project_path.to_path_buf();
        path_buf.push("ffmpeg_configuration.json");
        Ok(path_buf)
    }

    fn serialize(
        &self,
    ) -> dev_disp_core::util::PinnedLocalFuture<'_, Result<Vec<u8>, Box<dyn std::error::Error>>>
    {
        async move {
            let data = serde_json::to_vec_pretty(&self)?;
            Ok(data)
        }
        .boxed_local()
    }

    fn deserialize(
        mut source: Vec<u8>,
    ) -> dev_disp_core::util::PinnedLocalFuture<'static, Result<Self, Box<dyn std::error::Error>>>
    {
        async move {
            let slice = source.as_mut_slice();
            json_strip_comments::strip_slice(slice)?;
            let config = serde_json::from_slice::<FfmpegConfiguration>(slice)?;
            Ok(config)
        }
        .boxed_local()
    }
}
