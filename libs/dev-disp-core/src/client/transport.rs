use std::{
    fmt::{Debug, Display},
    future,
    time::Duration,
};

use futures_util::FutureExt;
use log::debug;
use thiserror::Error;

use crate::{
    host::{DisplayParameters, ScreenContentParameters},
    util::PinnedLocalFuture,
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
    EncodeError(Box<dyn std::error::Error + Send>),
    #[error("Failed to send data")]
    SendError(Box<dyn std::error::Error + Send>),
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
                    "encoded_bytes={}, sent_bytes={}, encode_time={}ms, send_time={}ms",
                    encoded_bytes,
                    sent_bytes,
                    encode_time.as_millis(),
                    send_time.as_millis()
                )
            }
            TransportSendMetrics::Send {
                sent_bytes,
                send_time,
            } => {
                write!(
                    f,
                    "sent_bytes={}, send_time={}ms",
                    sent_bytes,
                    send_time.as_millis()
                )
            }
        }
    }
}

/// The contract for something that can negotiate parameters and send screen data to a client.
/// This usually exists in the place that produces the screen data, or where the virtual screen
/// is being managed.
///
/// Opposite of [ScreenTransportReceiver]
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

    /// Called once after the virtual screen has been created, and we will soon begin sending
    /// screen data through the transport.
    ///
    /// You can perform codec negotiation with the screen host device based on the virtual screen's
    /// parameters here.
    fn prepare_send_screen_data<'s, 'p>(
        &'s mut self,
        source_parameters: &'p ScreenContentParameters,
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
///
/// Type parameter `D` represents the transport-specific data that will be provided to the adapter
/// during the preparation phase.
pub trait ScreenTransportReceiver<D> {
    type Error: std::error::Error;

    /// Initialize listening facilities
    fn initialize<'s>(&'s mut self) -> PinnedLocalFuture<'s, Result<(), Self::Error>>;

    /// The listen loop for receiving screen data and handling incoming messages from the [ScreenTransport].
    fn listen<'s, T>(&'s mut self, adapter: T) -> PinnedLocalFuture<'s, Result<(), Self::Error>>
    where
        T: ScreenReceiverAdapter<D> + 's,
        T::Error: std::error::Error;
}

/// The thing that adapts the [ScreenTransportReceiver] to some higher-level platform-specific application logic.
///
/// Type `D` represents the transport-specific data that will be provided to the adapter during the preparation phase,
/// before screen data is sent.
pub trait ScreenReceiverAdapter<D> {
    type Error: std::error::Error + 'static;

    /// Initialize any resources before receiving.
    fn initialize(&mut self) -> PinnedLocalFuture<'_, Result<(), Self::Error>>;

    /// Handle the notification that the virtual screen is being prepared
    fn on_loading_screen(&mut self) -> PinnedLocalFuture<'_, Result<(), Self::Error>>;

    /// Provides the display parameters that this receiver can host.
    fn provide_display_config(
        &mut self,
    ) -> PinnedLocalFuture<'_, Result<DisplayParameters, Self::Error>>;

    /// Called when the receiver is being gracefully closed and should release any resources.
    fn close(&mut self) -> PinnedLocalFuture<'_, Result<(), Self::Error>>;

    /// Optional background task used by the receiver controller.
    fn background<'s, 'a>(&'s mut self) -> PinnedLocalFuture<'a, Result<(), Self::Error>> {
        async { Ok(()) }.boxed_local()
    }

    fn prepare_receive_screen_data<'s>(
        &'s mut self,
        parameters: &'s ScreenContentParameters,
        transport_data: D,
    ) -> PinnedLocalFuture<'s, Result<(), Self::Error>>;

    /// The point where the screen data is received and optionally decoded before being displayed.
    fn recieve_screen_data<'s>(
        &'s mut self,
        data: &'s [u8],
    ) -> PinnedLocalFuture<'s, Result<(), Self::Error>>;
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

    fn prepare_send_screen_data<'s, 'p>(
        &'s mut self,
        parameters: &'p ScreenContentParameters,
    ) -> PinnedLocalFuture<'s, Result<(), TransportError>>
    where
        'p: 's,
    {
        self.inner.prepare_send_screen_data(parameters)
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

pub async fn run_listener_with_adapter<L, A, D>(
    mut listener: L,
    adapter: A,
) -> Result<(), <L as ScreenTransportReceiver<D>>::Error>
where
    L: ScreenTransportReceiver<D>,
    A: ScreenReceiverAdapter<D>,
{
    listener.listen(adapter).await
}
