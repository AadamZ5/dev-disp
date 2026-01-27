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

pub type DiscoveryId = String;

/// App keeps track of the current available devices, and in-use devices.
struct App<S>
where
    S: ScreenProvider,
{
    screen_provider: S,
    available_devices: Arc<RwLock<HashMap<DiscoveryId, DeviceRef>>>,
    in_use_devices: Arc<RwLock<HashMap<DiscoveryId, DeviceRef>>>,
}

impl<S> App<S>
where
    S: ScreenProvider,
{
    pub fn new(screen_provider: S) -> Self {
        Self {
            screen_provider,
            available_devices: Arc::new(RwLock::new(HashMap::new())),
            in_use_devices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn setup_discovery<D, C, T>(
        &self,
        discovery: D,
        discovery_id: DiscoveryId,
    ) -> PinnedFuture<'static, ()>
    where
        D: StreamingDeviceDiscovery<DeviceCandidate = C>,
        C: ConnectableDevice<Transport = T>,
        T: ScreenTransport + 'static,
    {
        let provider = self.screen_provider.clone();
        let discovery = discovery.into_stream();
        let available_devices = self.available_devices.clone();

        // Discover devices, and enter them into the available devices list.
        async move {
            while let Some(devices) = discovery.next().await {
                for device in devices {
                    let info = device.get_info();
                    let device_ref = DeviceRef {
                        name: info.name,
                        interface_key: discovery_id.clone(),
                        interface_display: discovery.get_display_name(),
                        id: info.id,
                        serial: None,
                    };

                    available_devices
                        .write()
                        .await
                        .insert(device_ref.id.clone(), device_ref);
                }
            }
        }
        .boxed()
    }

    pub async fn setup_discovery_polling<D>(
        &self,
        discovery: D,
        discovery_id: DiscoveryId,
        poll_interval: std::time::Duration,
    ) where
        D: dev_disp_core::host::DeviceDiscovery,
    {
        let streaming_discovery =
            dev_disp_core::host::PollingDeviceDiscovery::new(discovery, poll_interval, |d| {
                tokio::time::sleep(d).boxed()
            });

        self.setup_discovery(streaming_discovery, discovery_id)
    }
}
