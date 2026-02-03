use crate::util::{PinnedFuture, PinnedStream};

pub type DiscoveryId = String;
pub type DisplayHostId = String;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DisplayHostStatus {
    Available,
    Connecting,
    InUse,
    Unreachable,
    Disconnecting,
    Error(String),
    #[default]
    Unknown,
}

#[derive(Debug, Clone)]
pub struct DisplayHostRef {
    pub name: String,
    pub discovery_id: DiscoveryId,
    pub id: DisplayHostId,
    pub status: DisplayHostStatus,
}

#[derive(Debug, Clone)]
pub struct DiscoveryRef {
    pub id: DiscoveryId,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeviceCollectionStatus {
    pub connectable_devices: Vec<DisplayHostRef>,
    pub in_use_devices: Vec<DisplayHostRef>,
}

/// Represents the API for controlling and managing the dev disp application
pub trait DevDispApi {
    fn get_devices(
        &self,
    ) -> PinnedFuture<
        'static,
        Result<DeviceCollectionStatus, Box<dyn std::error::Error + Send + Sync>>,
    >;
    fn stream_devices(&self) -> PinnedStream<'static, DeviceCollectionStatus>;

    /// TODO: Better error handling
    fn initialize_device(
        &self,
        discovery_id: DiscoveryId,
        device_id: DisplayHostId,
    ) -> PinnedFuture<'static, Result<(), String>>;

    fn disconnect_device(
        &self,
        discovery_id: DiscoveryId,
        device_id: DisplayHostId,
    ) -> PinnedFuture<'static, Result<(), String>>;

    fn get_discovery_methods(
        &self,
    ) -> PinnedFuture<'static, Result<Vec<DiscoveryRef>, Box<dyn std::error::Error + Send + Sync>>>;

    // TODO: Do we need a stream for discovery methods changes?
}
