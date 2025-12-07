use std::path::Path;

use log::{debug, warn};

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationFileError {
    #[error("I/O error: {0}")]
    IoError(std::io::Error),
    #[error("Serialization error: {0}")]
    SerializationError(Box<dyn std::error::Error>),
}

pub async fn read_configuration_for<T>(path: &Path) -> Result<T, ConfigurationFileError>
where
    T: dev_disp_core::core::ConfigurationFile,
{
    debug!(
        "Reading configuration file for {} from {:?}",
        T::display_name(),
        path
    );
    let content = tokio::fs::read(path)
        .await
        .map_err(ConfigurationFileError::IoError)?;
    let config = T::deserialize(content)
        .await
        .map_err(ConfigurationFileError::SerializationError)?;
    debug!(
        "Read configuration file for {} from {:?}",
        T::display_name(),
        path
    );
    Ok(config)
}

pub async fn write_configuration_for<T>(
    path: &Path,
    config: &T,
) -> Result<(), ConfigurationFileError>
where
    T: dev_disp_core::core::ConfigurationFile,
{
    let parent_folder =
        path.parent()
            .ok_or(ConfigurationFileError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to get parent folder for configuration file path",
            )))?;
    tokio::fs::create_dir_all(parent_folder)
        .await
        .map_err(ConfigurationFileError::IoError)?;
    debug!(
        "Writing configuration file for {} to {:?}",
        T::display_name(),
        path
    );
    let data = config
        .serialize()
        .await
        .map_err(ConfigurationFileError::SerializationError)?;
    tokio::fs::write(path, data)
        .await
        .map_err(ConfigurationFileError::IoError)?;
    Ok(())
}

pub async fn read_configuration_or_write_default_for<T>(
    path: &Path,
) -> Result<T, ConfigurationFileError>
where
    T: dev_disp_core::core::ConfigurationFile + Default,
{
    match read_configuration_for::<T>(path).await {
        Ok(config) => Ok(config),
        Err(err) => {
            let default_config = T::default();

            match err {
                ConfigurationFileError::IoError(ref io_err)
                    if io_err.kind() == std::io::ErrorKind::NotFound =>
                {
                    debug!(
                        "Configuration file for {} not found at {:?}, writing default configuration",
                        T::display_name(),
                        path
                    );
                    match write_configuration_for(path, &default_config).await {
                        Ok(_) => Ok(default_config),
                        Err(e) => {
                            warn!(
                                "Failed to write default configuration file for {} at {:?}: {:?}",
                                T::display_name(),
                                path,
                                e
                            );
                            Err(e)
                        }
                    }
                }
                _ => {
                    warn!(
                        "Failed to read configuration file for {} at {:?}: {:?}",
                        T::display_name(),
                        path,
                        err
                    );
                    Err(err)
                }
            }
        }
    }
}
