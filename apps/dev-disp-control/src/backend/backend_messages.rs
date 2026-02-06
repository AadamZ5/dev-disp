use dev_disp_core::daemon::api::{DeviceCollectionStatus, DiscoveryId, DisplayHostId};

#[derive(Debug, Clone)]
pub enum Event {
    Connected(String),
    Disconnected,
    DeviceListUpdated(DeviceCollectionStatus),
}

/// Commands that can be sent to the backend task to perform actions.
///
/// A `Connect` command is missing here to avoid polluting all event enums
/// with a generic parameter, since the connection parameter can be generic
/// to the backend implementation. Instead, the connection command is sent via
/// a separate channel on the [BackendRef].
#[derive(Debug, Clone)]
pub enum Command {
    Disconnect,
    StreamDevices,
    ConnectDevice(DisplayHostId, DiscoveryId),
    DisconnectDevice(DisplayHostId, DiscoveryId),
}
