/// TODO: Implement this file?
use dev_disp_core::core::DevDispMessageFromSource;
use futures::channel::mpsc;

use crate::websocket::messages::WsMessageProtocolInit;

struct WsTransportListenCtx<'a> {
    tx_request_pre_init: mpsc::Sender<()>,
    tx_request_device_info: mpsc::Sender<()>,
    tx_request_protocol_init: mpsc::Sender<WsMessageProtocolInit>,
    tx_core: mpsc::Sender<DevDispMessageFromSource<'a>>,
}

pub struct WsTransportClientFunctions {
    handle_request_pre_init: Box<dyn Fn() + Send>,
    handle_request_device_info: Box<dyn Fn() + Send>,
    handle_request_protocol_init: Box<dyn Fn(WsMessageProtocolInit) + Send>,
    handle_core: Box<dyn Fn(DevDispMessageFromSource) + Send>,
}

pub struct WsTransportClient {
    functions: WsTransportClientFunctions,
}

impl WsTransportClient {
    pub fn new(functions: WsTransportClientFunctions) -> Self {
        Self { functions }
    }
}
