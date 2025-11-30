pub use dev_disp_core::{
    core::{DevDispMessageFromClient, DevDispMessageFromSource},
    host::DisplayParameters,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WsMessageProtocolInit {
    // TODO: Security!
    pub init_key: String,
}

/// This is device information shown to the user when selecting a device.
/// In other words, this is "pre" display parameters, intended to give the
/// user an idea of what the device is before the initiate a full display session.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WsMessageDeviceInfo {
    pub name: String,
    pub resolution: (u32, u32),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(bound(deserialize = "'de: 'a"))]
pub enum WsMessageFromSource<'a> {
    /// Used to ask new connection if it is in the right place
    RequestPreInit,

    /// Used to ask connection for its device information
    RequestDeviceInformation,

    /// Used to request that the device is really ready to receive screen data
    RequestProtocolInit(WsMessageProtocolInit),

    /// Used to forward a core logic message to the client
    Core(DevDispMessageFromSource<'a>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WsMessageFromClient {
    /// Used to tell the server "I intend to be selectable"
    ResponsePreInit,

    /// Used to give the server basic info used for display
    ResponseDeviceInformation(WsMessageDeviceInfo),

    /// Used to assure the server we are ready to display stuff
    ResponseProtocolInit(WsMessageProtocolInit),

    /// Used to give a core-logic message to the server
    Core(DevDispMessageFromClient),
}
