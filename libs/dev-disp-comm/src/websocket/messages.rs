use dev_disp_core::core::{DevDispMessageFromClient, DevDispMessageFromSource};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WsMessageProtocolInit {
    // TODO: Security!
    init_key: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(bound(deserialize = "'de: 'a"))]
pub enum WsMessageFromSource<'a> {
    RequestProtocolInit(WsMessageProtocolInit),
    Core(DevDispMessageFromSource<'a>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WsMessageFromClient {
    ResponseProtocolInit(WsMessageProtocolInit),
    Core(DevDispMessageFromClient),
}
