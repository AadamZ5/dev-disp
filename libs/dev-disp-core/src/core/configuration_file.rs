use crate::util::PinnedLocalFuture;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigurationFilePathError {
    /// The default configuration file path is not available.
    #[error("Default configuration file path is not available")]
    NotAvailable,
    /// Other default path error
    #[error("Other default path error: {0}")]
    Other(Box<dyn std::error::Error>),
}

/// A trait for some thing that represents a configuration file.
pub trait ConfigurationFile: Default {
    /// Returns a display name to describe the configuration file type.
    fn display_name() -> String;

    /// A function that instructs the core how to retrieve the default path for this
    /// configuration file. The provided argument is the base project configuration directory,
    /// but does not have to be used.
    fn get_default_path(project_config: &Path) -> Result<PathBuf, ConfigurationFilePathError>;

    /// A function that serializes the configuration file with whatever
    /// format is appropriate.
    fn serialize(&self) -> PinnedLocalFuture<'_, Result<Vec<u8>, Box<dyn std::error::Error>>>;

    /// A function that deserializes the configuration file from whatever
    /// format is appropriate.
    fn deserialize(
        source: Vec<u8>,
    ) -> PinnedLocalFuture<'static, Result<Self, Box<dyn std::error::Error>>>;
}

pub fn get_default_config_path_for<T>() -> Result<PathBuf, ConfigurationFilePathError>
where
    T: ConfigurationFile,
{
    let project_config = dirs::config_local_dir()
        .map(|path| path.join("dev-disp"))
        .ok_or(ConfigurationFilePathError::NotAvailable)?;
    T::get_default_path(project_config.as_path())
}
