use dev_disp_core::{
    client::{ScreenTransport, TransportError},
    util::PinnedFuture,
};
use futures_util::FutureExt;

pub struct WsTransport {}

impl ScreenTransport for WsTransport {
    fn initialize(&mut self) -> PinnedFuture<'_, Result<(), TransportError>> {
        todo!()
    }

    fn notify_loading_screen(&self) -> PinnedFuture<'_, Result<(), TransportError>> {
        async { Err(TransportError::NotImplemented) }.boxed()
    }

    fn get_display_config(
        &mut self,
    ) -> PinnedFuture<'_, Result<dev_disp_core::host::DisplayParameters, TransportError>> {
        todo!()
    }

    fn send_screen_data<'a>(
        &mut self,
        data: &'a [u8],
    ) -> PinnedFuture<'_, Result<(), TransportError>> {
        todo!()
    }
}
