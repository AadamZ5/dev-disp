use std::{
    fmt::{Debug, Display},
    pin::Pin,
};

use crate::{
    client::{ScreenTransport, SomeScreenTransport, TransportError},
    coding::encoder::EncoderContentParameters,
    host::DisplayParameters,
    util::{PinnedFuture, PinnedLocalFuture},
};

/// The display host is the device that is hosting the screen, not
/// the device producing the screen data. The display host consumes
/// the produced screen data.
#[derive(Debug)]
pub struct DisplayHost<T> {
    client_id: i32,
    name: String,
    transport: T,
}

impl<T> Display for DisplayHost<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.client_id)
    }
}

impl<T> DisplayHost<T>
where
    T: ScreenTransport,
{
    pub fn new(client_id: i32, name: String, transport: T) -> Self {
        Self {
            client_id,
            name,
            transport,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_client_id(&self) -> i32 {
        self.client_id
    }

    pub fn background_task<'s, 'a>(&'s mut self) -> PinnedFuture<'a, Result<(), TransportError>>
    where
        'a: 's,
    {
        self.transport.background()
    }

    pub async fn initialize(&mut self) -> Result<(), TransportError> {
        self.transport.initialize().await
    }

    /// Notifies the screen host device that the virtual screen is currently loading.
    /// See [ScreenTransport::notify_loading_screen] for more details.
    ///
    /// Some providers like EVDI take awhile to create a new virtual screen. This lets the
    /// device know that that stuff is in progress.
    pub async fn notify_loading_screen(&mut self) -> Result<(), TransportError> {
        self.transport.notify_loading_screen().await
    }

    /// See [ScreenTransport::get_display_config] for more details.
    pub async fn get_display_config(&mut self) -> Result<DisplayParameters, TransportError> {
        self.transport.get_display_config().await
    }

    /// Performs codec negotiation with the screen host device.
    /// See [ScreenTransport::setup_encoding_config] or your specific implementation
    /// of that trait for more details.
    pub async fn setup_encoding_config(
        &mut self,
        source_parameters: &EncoderContentParameters,
    ) -> Result<(), TransportError> {
        self.transport
            .setup_encoding_config(source_parameters)
            .await
    }

    /// See [ScreenTransport::encode] for more details.
    ///
    /// TODO: Consider changing the future to be non-boxed if possible for performance
    pub fn encode<'s, 'a>(
        &'s mut self,
        raw_data: &'a [u8],
    ) -> PinnedLocalFuture<'s, Result<&'a [u8], TransportError>>
    where
        'a: 's,
    {
        self.transport.encode(raw_data)
    }

    /// See [ScreenTransport::send_screen_data] for more details.
    ///
    /// TODO: Consider changing the future to be non-boxed if possible for performance
    pub fn send_screen_data<'s, 'a>(
        &'s mut self,
        data: &'a [u8],
    ) -> PinnedLocalFuture<'s, Result<(), TransportError>>
    where
        'a: 's,
    {
        self.transport.send_screen_data(data)
    }

    /// See [ScreenTransport::close] for more details.
    pub async fn close(&mut self) -> Result<(), TransportError> {
        self.transport.close().await
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    /// Return a new version of this DisplayHost with the transport
    /// wrapped in a `SomeScreenTransport`, to unify the transport type.
    ///
    /// Note that this introduces boxing and dynamic dispatch overhead.
    pub fn as_generic_transport(self) -> DisplayHost<SomeScreenTransport>
    where
        T: 'static,
    {
        DisplayHost {
            client_id: self.client_id,
            name: self.name.clone(),
            transport: SomeScreenTransport::new(self.transport),
        }
    }
}
