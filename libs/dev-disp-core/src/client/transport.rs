use std::{
    fmt::{Debug, Display},
    future,
};

use futures_util::FutureExt;
use log::debug;
use thiserror::Error;

use crate::{
    host::{DisplayParameters, EncoderContentParameters, EncoderPossibleConfiguration},
    util::PinnedFuture,
};

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
    /// Initialization setup for the transport. Called first to allow the
    /// transport to perform any preliminary setup. You can use this to get screen size,
    /// perform integrity challenge, or some other preliminary tasks.
    ///
    /// Encoder setup is handled later in [Self::setup_encoding_config], try not to do that here.
    /// The controller logic notifies the control application of what phase of initialization the
    /// connection is currently in.
    fn initialize(&mut self) -> PinnedFuture<'_, Result<(), TransportError>>;

    /// Notifies the transport that the virtual screen is currently being created.
    fn notify_loading_screen(&self) -> PinnedFuture<'_, Result<(), TransportError>> {
        async { Err(TransportError::NotImplemented) }.boxed()
    }

    /// Retrieves the current screen host display configuration from the transport.
    /// The returned value should represent the usable display paramters of the
    /// client that will be showing our display data.
    ///
    /// Used before creating the virtual screen, so we know what parameters to
    /// create the virtual screen with
    fn get_display_config(&mut self)
    -> PinnedFuture<'_, Result<DisplayParameters, TransportError>>;

    /// Used when the business logic deems that the loop is ending, and the connection will be closed.
    /// Purely a hook to allow the transport to notify the client of a graceful shutdown, and optionally
    /// return the client back to the "available" pool.
    /// TODO: Allow to pass owned `self` here, not an `&mut self`
    fn close(&mut self) -> PinnedFuture<'_, Result<(), TransportError>> {
        future::ready(Ok(())).boxed()
    }

    /// Optional function that runs in the background while the transport is active,
    /// started before initialization. Cannot hold onto self reference.
    fn background<'s, 'a>(&'s mut self) -> PinnedFuture<'a, Result<(), TransportError>> {
        debug!("Default transport background impl");
        future::ready(Ok(())).boxed()
    }

    /// Used when the business logic wants to set up the encoding configuration for the transport.
    ///
    /// Perform codec negotiation with the screen host device based on the virtual screen's
    /// parameters.
    fn setup_encoding_config(
        &mut self,
        source_parameters: &EncoderContentParameters,
    ) -> PinnedFuture<'_, Result<(), TransportError>>;

    /// The encoding step for this transport. Defined separately to allow for performance tracing.
    ///
    /// TODO: Consider changing the future to be non-boxed if possible for performance
    fn encode<'s, 'a>(
        &'s mut self,
        raw_data: &'a [u8],
    ) -> PinnedFuture<'s, Result<&'a [u8], TransportError>>
    where
        'a: 's;

    /// The transmission step for this transport, responsible for sending the encoded screen data to the client.
    ///
    /// TODO: Consider changing the future to be non-boxed if possible for performance
    fn send_screen_data<'s, 'a>(
        &'s mut self,
        encoded_data: &'a [u8],
    ) -> PinnedFuture<'s, Result<(), TransportError>>
    where
        'a: 's;
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

    fn background<'s, 'a>(&'s mut self) -> PinnedFuture<'a, Result<(), TransportError>> {
        self.inner.background()
    }

    fn notify_loading_screen(&self) -> PinnedFuture<'_, Result<(), TransportError>> {
        self.inner.notify_loading_screen()
    }

    fn setup_encoding_config(
        &mut self,
        parameters: EncoderContentParameters,
    ) -> PinnedFuture<'_, Result<(), TransportError>> {
        self.inner.setup_encoding_config(parameters)
    }

    fn encode<'s, 'a>(
        &'s mut self,
        raw_data: &'a [u8],
    ) -> PinnedFuture<'s, Result<&'a [u8], TransportError>>
    where
        'a: 's,
    {
        self.inner.encode(raw_data)
    }

    fn send_screen_data<'s, 'a>(
        &'s mut self,
        data: &'a [u8],
    ) -> PinnedFuture<'s, Result<(), TransportError>>
    where
        'a: 's,
    {
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
