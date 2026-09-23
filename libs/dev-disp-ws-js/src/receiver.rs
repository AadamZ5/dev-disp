use dev_disp_core::{client::ScreenTransportReceiver, util::PinnedLocalFuture};
use futures::FutureExt;
use thiserror::Error;

pub struct JsAdapter {}

pub struct JsWsReceiver {}

pub struct JsTransportData {}

#[derive(Debug, Error)]
pub enum JsWsReceiverError {
    #[error("Initialization failed")]
    InitializationFailed,
    #[error("Listen failed")]
    ListenFailed,
}

impl ScreenTransportReceiver for JsWsReceiver {
    type Error = JsWsReceiverError;

    type Adapter = JsAdapter;

    fn initialize(&mut self) -> PinnedLocalFuture<'_, Result<(), Self::Error>> {
        async { Ok(()) }.boxed_local()
    }

    fn listen(&mut self) -> PinnedLocalFuture<'_, Result<(), Self::Error>> {
        todo!()
    }
}
