use std::future;

use futures_util::Sink;

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

/// The transport needs to be a sink that sends the screen data to the
/// client via whatever means possible.
pub trait ScreenTransport: Sink<&'static [u8]> {
    fn get_display_config(&self) -> impl Future<Output = DisplayHostInfo>;

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> {
        future::ready(Ok(()))
    }

    fn send_screen_data<'a>(&mut self, data: &'a [u8]) -> Result<(), Self::Error>;
}
