use dev_disp_core::util::{PinnedFuture, PinnedStream};

pub type DiscoveryId = String;
pub type DisplayHostId = String;

#[derive(Debug, Clone)]
pub struct DeviceRef {
    pub name: String,
    pub interface_key: String,
    pub interface_display: String,
    pub id: String,
}

pub struct DeviceCollectionStatus {
    pub connectable_devices: Vec<DeviceRef>,
    pub in_use_devices: Vec<DeviceRef>,
}

pub trait DevDispApiFacade {
    fn get_device_status(&self) -> PinnedFuture<'static, DeviceCollectionStatus>;
    fn stream_device_status(&self) -> PinnedStream<'static, DeviceCollectionStatus>;

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
}
