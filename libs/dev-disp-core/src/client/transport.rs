use std::{
    fmt::{Debug, Display},
    future,
};

use futures_util::FutureExt;
use thiserror::Error;

use crate::{host::DisplayParameters, util::PinnedFuture};

#[derive(Debug, Error)]
pub enum TransportError {
    NoConnection,
    Timeout,
    Other(Box<dyn std::error::Error + Send + Sync>),
    Unknown,
    NotImplemented,
    SerializationError,
}

impl Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::NoConnection => write!(f, "No connection"),
            TransportError::Timeout => write!(f, "Timeout"),
            TransportError::Other(e) => write!(f, "Other error: {}", e),
            TransportError::Unknown => write!(f, "Unknown error"),
            TransportError::NotImplemented => write!(f, "Not Implemented"),
            TransportError::SerializationError => write!(f, "Serialization Error"),
        }
    }
}

/// The transport needs to be a sink that sends the screen data to the
/// client via whatever means possible.
pub trait ScreenTransport {
    fn initialize(&mut self) -> PinnedFuture<'_, Result<(), TransportError>>;

    fn notify_loading_screen(&self) -> PinnedFuture<'_, Result<(), TransportError>> {
        async { Err(TransportError::NotImplemented) }.boxed()
    }

    fn get_display_config(&mut self)
    -> PinnedFuture<'_, Result<DisplayParameters, TransportError>>;

    fn close(&mut self) -> PinnedFuture<'_, Result<(), TransportError>> {
        future::ready(Ok(())).boxed()
    }

    /// Optional function that runs in the background while the transport is active,
    /// after initialization.
    fn background(&self) -> PinnedFuture<'_, Result<(), TransportError>> {
        future::ready(Ok(())).boxed()
    }

    fn send_screen_data<'a>(
        &mut self,
        data: &'a [u8],
    ) -> PinnedFuture<'_, Result<(), TransportError>>;
}

pub struct SomeScreenTransport {
    inner: Box<dyn ScreenTransport>,
}

impl SomeScreenTransport {
    pub fn new<T>(inner: T) -> Self
    where
        T: ScreenTransport + 'static,
    {
        Self {
            inner: Box::new(inner),
        }
    }

    pub fn new_boxed(inner: Box<dyn ScreenTransport>) -> Self {
        Self { inner }
    }
}

impl ScreenTransport for SomeScreenTransport {
    fn initialize(&mut self) -> PinnedFuture<'_, Result<(), TransportError>> {
        self.inner.initialize()
    }

    fn get_display_config(
        &mut self,
    ) -> PinnedFuture<'_, Result<DisplayParameters, TransportError>> {
        self.inner.get_display_config()
    }

    fn notify_loading_screen(&self) -> PinnedFuture<'_, Result<(), TransportError>> {
        self.inner.notify_loading_screen()
    }

    fn send_screen_data<'s, 'a>(
        &mut self,
        data: &'a [u8],
    ) -> PinnedFuture<'_, Result<(), TransportError>> {
        self.inner.send_screen_data(data)
    }

    fn close(&mut self) -> PinnedFuture<'_, Result<(), TransportError>> {
        self.inner.close()
    }
}

impl From<Box<dyn ScreenTransport>> for SomeScreenTransport {
    fn from(value: Box<dyn ScreenTransport>) -> Self {
        Self::new_boxed(value)
    }
}
