use dev_disp_core::{
    client::ScreenReceiverAdapter, client::ScreenTransportReceiver, util::PinnedLocalFuture,
};
use dev_disp_transports::websocket::transport::WsReceiverPrepState;
use futures::FutureExt;
use thiserror::Error;

pub struct JsAdapter {}

#[derive(Debug, Error)]
pub enum JsAdapterError {}

impl<'a> ScreenReceiverAdapter<WsReceiverPrepState<'a>> for JsAdapter {
    type Error = JsAdapterError;

    fn initialize(&mut self) -> PinnedLocalFuture<'_, Result<(), Self::Error>> {
        todo!()
    }

    fn on_loading_screen(&mut self) -> PinnedLocalFuture<'_, Result<(), Self::Error>> {
        todo!()
    }

    fn provide_display_config(
        &mut self,
    ) -> PinnedLocalFuture<'_, Result<dev_disp_core::host::DisplayParameters, Self::Error>> {
        todo!()
    }

    fn close(&mut self) -> PinnedLocalFuture<'_, Result<(), Self::Error>> {
        todo!()
    }

    fn prepare_receive_screen_data(
        &mut self,
        parameters: &dev_disp_core::host::ScreenContentParameters,
        transport_data: WsReceiverPrepState<'a>,
    ) -> PinnedLocalFuture<'_, Result<(), Self::Error>> {
        todo!()
    }

    fn recieve_screen_data(
        &mut self,
        data: &[u8],
    ) -> PinnedLocalFuture<'_, Result<(), Self::Error>> {
        todo!()
    }
}
