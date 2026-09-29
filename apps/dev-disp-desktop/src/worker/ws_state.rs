use dev_disp_core::client::run_receiver_with_adapter;
use dev_disp_transports::websocket::receiver::WsReceiver;
use tokio::task::JoinHandle;

pub enum WsContextCommand {}

/// The context used to communicate with the adapter service.
pub struct WsContext {
    work_handle: JoinHandle<()>,
}
