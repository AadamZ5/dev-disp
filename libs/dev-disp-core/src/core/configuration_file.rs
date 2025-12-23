use crate::util::PinnedLocalFuture;
use futures::{FutureExt, Stream};
use rust_util::computed_cell::{ComputedCell, ComputedResult};
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

pub struct ConfigurationFileConnection<T>
where
    T: ConfigurationFile + Clone,
{
    inner: ComputedCell<
        T,
        Box<dyn Fn() -> PinnedLocalFuture<'static, T>>,
        PinnedLocalFuture<'static, T>,
        (),
    >,
}

/// A connection that represents a configuration file that can be
/// reloaded when invalidated.
///
/// TODO: Should we just refactor this to be a stream with latest value?
impl<T> ConfigurationFileConnection<T>
where
    T: ConfigurationFile + Clone,
{
    pub fn new<F, Fut, I>(load_fn: F, invalidate_notifications: I) -> Self
    where
        F: Fn() -> Fut + 'static,
        Fut: Future<Output = Result<T, Box<dyn std::error::Error>>> + 'static,
        I: Stream<Item = ()> + Unpin + 'static,
    {
        let compute_fn = move || {
            let fut = load_fn();
            async move {
                match fut.await {
                    Ok(config) => config,
                    Err(e) => {
                        log::error!(
                            "Failed to load configuration file {}: {}. Using default.",
                            T::display_name(),
                            e
                        );
                        T::default()
                    }
                }
            }
            .boxed_local()
        };
        Self {
            inner: ComputedCell::new(Box::new(compute_fn), invalidate_notifications),
        }
    }

    pub async fn get_configuration(&mut self) -> &ComputedResult<T> {
        self.inner.get().await
    }
}
