use dev_disp_core::daemon::api::{DeviceCollectionStatus, DiscoveryId, DisplayHostId};

/// Events to communicate to the frontend
#[derive(Debug, Clone)]
pub enum BackendEvent {
    Connected(String),
    Disconnected,
    DeviceListUpdated(DeviceCollectionStatus),
}

#[derive(Debug, Clone)]
pub enum BackendCommand {
    Disconnect,

    /// Attempt to connect to the specified device.
    InitializeDevice(DisplayHostId, DiscoveryId),
    /// Attempt to stop using the specified device.
    DisconnectDevice(DisplayHostId, DiscoveryId),

    StreamDevices,
}
