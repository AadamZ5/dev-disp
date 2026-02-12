use dev_disp_core::daemon::api::{DeviceCollectionStatus, DiscoveryId, DisplayHostId};

/// Events that come from the backend task.
#[derive(Debug, Clone)]
pub enum Event {
    /// The backend has connected to the endpoint.
    Connected(String),
    /// The backend has disconnected from the endpoint.
    /// This can happen either because the frontend requested
    /// a disconnect, or because the connection failed somehow.
    Disconnected,
    /// The device list has been updated with the contained data.
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
    /// Ask the backend to disconnect from the current API.
    Disconnect,
    /// Ask the backend to start streaming device updates.
    StreamDevices,
    /// Attempt to initialize the specified device.
    InitializeDevice(DisplayHostId, DiscoveryId),
    /// Attempt to stop using the specified device.
    DisconnectDevice(DisplayHostId, DiscoveryId),
}
