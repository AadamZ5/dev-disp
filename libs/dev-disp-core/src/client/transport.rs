use std::{
    fmt::{Debug, Display},
    future,
};

use thiserror::Error;

type PinnedFuture<T> = std::pin::Pin<Box<dyn Future<Output = T> + Send>>;

/// Information about the screen provided by the client.
#[derive(Debug, Clone)]
pub struct DisplayHostInfo {
    pub width_px: u32,
    pub height_px: u32,
    edid: Vec<u8>,
}

impl DisplayHostInfo {
    pub fn new(width_px: u32, height_px: u32, edid: Vec<u8>) -> Self {
        Self {
            width_px,
            height_px,
            edid,
        }
    }

    pub fn get_edid(&self) -> &[u8] {
        &self.edid
    }
}

#[derive(Debug, Error)]
pub enum TransportError {
    NoConnection,
    Timeout,
    Other(Box<dyn std::error::Error + Send + Sync>),
    Unknown,
}

impl Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::NoConnection => write!(f, "No connection"),
            TransportError::Timeout => write!(f, "Timeout"),
            TransportError::Other(e) => write!(f, "Other error: {}", e),
            TransportError::Unknown => write!(f, "Unknown error"),
        }
    }
}

/// The transport needs to be a sink that sends the screen data to the
/// client via whatever means possible.
pub trait ScreenTransport {
    fn get_display_config(&self) -> PinnedFuture<DisplayHostInfo>;

    fn close(&mut self) -> PinnedFuture<Result<(), TransportError>> {
        Box::pin(future::ready(Ok(())))
    }

    fn send_screen_data<'a>(&mut self, data: &'a [u8]) -> Result<(), TransportError>;
}

pub struct SomeScreenTransport {
    inner: Box<dyn ScreenTransport>,
}

impl SomeScreenTransport {
    pub fn new(inner: Box<dyn ScreenTransport>) -> Self {
        Self { inner }
    }
}

impl ScreenTransport for SomeScreenTransport {
    fn get_display_config(&self) -> PinnedFuture<DisplayHostInfo> {
        self.inner.get_display_config()
    }

    fn send_screen_data<'a>(&mut self, data: &'a [u8]) -> Result<(), TransportError> {
        self.inner.send_screen_data(data)
    }

    fn close(&mut self) -> PinnedFuture<Result<(), TransportError>> {
        self.inner.close()
    }
}

impl From<Box<dyn ScreenTransport>> for SomeScreenTransport {
    fn from(value: Box<dyn ScreenTransport>) -> Self {
        Self::new(value)
    }
}
