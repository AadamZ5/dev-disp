use std::fmt::Display;

use crate::client::{DisplayHostInfo, ScreenTransport};

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

    pub async fn get_display_config(&self) -> DisplayHostInfo {
        self.transport.get_display_config().await
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    pub fn send_screen_data<'a>(&mut self, data: &'a [u8]) -> Result<(), T::Error> {
        self.transport.send_screen_data(data)
    }

    pub async fn close(&mut self) -> Result<(), T::Error> {
        self.transport.close().await
    }
}
