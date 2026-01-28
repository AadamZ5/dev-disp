use std::{collections::HashMap, sync::Arc};

use dev_disp_core::{
    client::ScreenTransport,
    host::{ConnectableDevice, ScreenProvider, StreamingDeviceDiscovery},
    util::{PinnedFuture, PinnedLocalFuture, PinnedLocalStream},
};
use futures_util::FutureExt;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct DeviceRef {
    pub name: String,
    pub interface_key: String,
    pub interface_display: String,
    pub id: String,
    pub serial: Option<String>,
}

pub struct DeviceCollectionStatus {
    pub connectable_devices: Vec<DeviceRef>,
    pub in_use_devices: Vec<DeviceRef>,
}

trait DevDispApiFacade {
    fn get_device_status(&self) -> PinnedLocalFuture<'_, DeviceCollectionStatus>;
    fn stream_device_status(&self) -> PinnedLocalStream<'_, DeviceCollectionStatus>;
}
