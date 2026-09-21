use std::{
    fmt::{Debug, Display},
    future,
    time::Duration,
};

use futures_util::FutureExt;
use log::debug;
use thiserror::Error;

use crate::{
    coding::encoder::EncoderContentParameters,
    host::DisplayParameters,
    util::{PinnedFuture, PinnedLocalFuture},
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

#[derive(Debug, Error)]
pub enum TransportSendError {
    #[error("Failed to encode data")]
    EncodeError(Box<dyn std::error::Error + Send + Sync>),
    #[error("Failed to send data")]
    SendError(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Clone)]
pub enum TransportSendMetrics {
    /// No metrics recorded
    None,
    /// Encoding and sending metrics were recorded
    EncodeAndSend {
        encoded_bytes: usize,
        sent_bytes: usize,
        encode_time: Duration,
        send_time: Duration,
    },
    /// Sending metrics were recorded
    Send {
        sent_bytes: usize,
        send_time: Duration,
    },
}

impl Display for TransportSendMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportSendMetrics::None => write!(f, "n/a"),
            TransportSendMetrics::EncodeAndSend {
                encoded_bytes,
                sent_bytes,
                encode_time,
                send_time,
            } => {
                write!(
                    f,
                    "encoded_bytes={}, sent_bytes={}, encode_time={:?}, send_time={:?}",
                    encoded_bytes, sent_bytes, encode_time, send_time
                )
            }
            TransportSendMetrics::Send {
                sent_bytes,
                send_time,
            } => {
                write!(f, "sent_bytes={}, send_time={:?}", sent_bytes, send_time)
            }
        }
    }
}

/// The contract for something that can negotiate parameters and send screen data to a client.
/// This usually exists in the place that produces the screen data, or where the virtual screen
/// is being managed.
///
/// Opposite of [ScreenReceiverTransport]
pub trait ScreenTransport {
    /// Initialization setup for the transport. Called first to allow the
    /// transport to perform any preliminary setup. You can use this to get screen size,
    /// perform integrity challenge, or some other preliminary tasks.
    ///
    /// Encoder setup is handled later in [Self::setup_encoding_config], try not to do that here.
    /// The controller logic notifies the control application of what phase of initialization the
    /// connection is currently in.
    fn initialize(&mut self) -> PinnedLocalFuture<'_, Result<(), TransportError>>;

    /// Notifies the transport that the virtual screen is currently being created.
    fn notify_loading_screen(&self) -> PinnedLocalFuture<'_, Result<(), TransportError>> {
        async { Err(TransportError::NotImplemented) }.boxed()
    }

    /// Retrieves the current screen host display configuration from the transport.
    /// The returned value should represent the usable display paramters of the
    /// client that will be showing our display data.
    ///
    /// Used before creating the virtual screen, so we know what parameters to
    /// create the virtual screen with
    fn get_display_config(
        &mut self,
    ) -> PinnedLocalFuture<'_, Result<DisplayParameters, TransportError>>;

    /// Used when the business logic deems that the loop is ending, and the connection will be closed.
    /// Purely a hook to allow the transport to notify the client of a graceful shutdown, and optionally
    /// return the client back to the "available" pool.
    /// TODO: Allow to pass owned `self` here, not an `&mut self`
    fn close(&mut self) -> PinnedLocalFuture<'_, Result<(), TransportError>> {
        future::ready(Ok(())).boxed()
    }

    /// Optional function that runs in the background while the transport is active,
    /// started before initialization. Cannot hold onto self reference.
    fn background<'s, 'a>(&'s mut self) -> PinnedLocalFuture<'a, Result<(), TransportError>> {
        debug!("Default transport background impl");
        future::ready(Ok(())).boxed()
    }

    /// Used when the business logic wants to set up the encoding configuration for the transport.
    ///
    /// Perform codec negotiation with the screen host device based on the virtual screen's
    /// parameters.
    fn setup_encoding_config<'s, 'p>(
        &'s mut self,
        source_parameters: &'p EncoderContentParameters,
    ) -> PinnedLocalFuture<'s, Result<(), TransportError>>
    where
        'p: 's; // The parameters generated will be alive for as long as the transport itself is alive.

    /// The point where the transport optionally encodes and delivers the data to the client.
    ///
    /// TODO: Consider changing the future to be non-boxed if possible for performance
    fn send_screen_data<'s, 'a>(
        &'s mut self,
        raw_data: &'a [u8],
    ) -> PinnedLocalFuture<'s, Result<TransportSendMetrics, TransportSendError>>
    where
        'a: 's;
}

/// The contract for something that can receive screen data from the screen producer,
/// and participate in negotiations for parameters and such. Usually this exists on
/// the receiving end of the screen data pipeline, or the place that is actually
/// going to display the screen data.
///
/// Opposite of [ScreenTransport]
pub trait ScreenReceiverTransport {
    // TODO: omg implement me
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
    fn initialize(&mut self) -> PinnedLocalFuture<'_, Result<(), TransportError>> {
        self.inner.initialize()
    }

    fn get_display_config(
        &mut self,
    ) -> PinnedLocalFuture<'_, Result<DisplayParameters, TransportError>> {
        self.inner.get_display_config()
    }

    fn background<'s, 'a>(&'s mut self) -> PinnedLocalFuture<'a, Result<(), TransportError>> {
        self.inner.background()
    }

    fn notify_loading_screen(&self) -> PinnedLocalFuture<'_, Result<(), TransportError>> {
        self.inner.notify_loading_screen()
    }

    fn setup_encoding_config<'s, 'p>(
        &'s mut self,
        parameters: &'p EncoderContentParameters,
    ) -> PinnedLocalFuture<'s, Result<(), TransportError>>
    where
        'p: 's,
    {
        self.inner.setup_encoding_config(parameters)
    }

    fn send_screen_data<'s, 'a>(
        &'s mut self,
        raw_data: &'a [u8],
    ) -> PinnedLocalFuture<'s, Result<TransportSendMetrics, TransportSendError>>
    where
        'a: 's,
    {
        self.inner.send_screen_data(raw_data)
    }

    fn close(&mut self) -> PinnedLocalFuture<'_, Result<(), TransportError>> {
        self.inner.close()
    }
}

impl From<Box<dyn ScreenTransport>> for SomeScreenTransport {
    fn from(value: Box<dyn ScreenTransport>) -> Self {
        Self::new_boxed(value)
    }
}
