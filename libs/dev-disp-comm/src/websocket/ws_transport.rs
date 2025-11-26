use async_tungstenite::WebSocketStream;
use dev_disp_core::{
    client::{ScreenTransport, TransportError},
    util::PinnedFuture,
};
use futures::{AsyncRead, AsyncWrite};
use futures_util::FutureExt;

pub struct WsTransport<S> {
    websocket: async_tungstenite::WebSocketStream<S>,
}

impl<S> WsTransport<S> {
    pub fn new(websocket: WebSocketStream<S>) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        Self { websocket }
    }
}

impl<S> ScreenTransport for WsTransport<S> {
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
